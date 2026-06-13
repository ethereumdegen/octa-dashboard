use axum::extract::{Multipart, Path, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::auth::KbCtx;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::services::s3;
use crate::state::AppState;

/// POST /api/kb/:kb_id/documents — drag-and-drop upload
pub async fn upload(
    ctx: KbCtx,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<(StatusCode, Json<Value>)> {
    let bucket = state.require_bucket()?;

    let mut file_data: Option<(String, String, Vec<u8>)> = None;
    let mut folder_id: Option<Uuid> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let filename = field.file_name().unwrap_or("unknown").to_string();
            let content_type = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string();
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(e.to_string()))?;
            file_data = Some((filename, content_type, bytes.to_vec()));
        } else if name == "folder_id" {
            let text = field
                .text()
                .await
                .map_err(|e| AppError::BadRequest(e.to_string()))?;
            if !text.is_empty() {
                folder_id = Some(
                    text.parse::<Uuid>()
                        .map_err(|_| AppError::BadRequest("invalid folder_id".into()))?,
                );
            }
        }
    }

    let (filename, content_type, bytes) =
        file_data.ok_or_else(|| AppError::BadRequest("no file field in upload".into()))?;

    let size_bytes = bytes.len() as i64;

    // Sanitize filename: strip path separators to prevent traversal
    let safe_filename: String = filename
        .replace(['/', '\\', '\0'], "_")
        .trim_start_matches('.')
        .to_string();
    let safe_filename = if safe_filename.is_empty() {
        "file".to_string()
    } else {
        safe_filename
    };

    let s3_key = format!("{}/documents/{}/{}", ctx.kb.id, Uuid::new_v4(), safe_filename);

    // Validate folder belongs to same KB
    if let Some(fid) = folder_id {
        let folder = db::folders::get_by_id(&state.db, fid)
            .await?
            .ok_or_else(|| AppError::BadRequest("folder not found".into()))?;
        if folder.kb_id != ctx.kb.id {
            return Err(AppError::BadRequest("folder does not belong to this KB".into()));
        }
    }

    s3::upload_bytes(bucket, &s3_key, &bytes, &content_type).await?;

    let doc = db::documents::insert(
        &state.db,
        ctx.kb.id,
        &filename,
        &content_type,
        &s3_key,
        size_bytes,
        ctx.identity.user_id,
        folder_id,
    )
    .await?;

    tracing::info!(doc_id = %doc.id, kb_id = %ctx.kb.id, filename = %doc.filename, "document uploaded");
    Ok((StatusCode::CREATED, Json(json!(doc))))
}

/// GET /api/kb/:kb_id/documents
pub async fn list(ctx: KbCtx, State(state): State<AppState>) -> AppResult<Json<Value>> {
    let docs = db::documents::list_for_kb(&state.db, ctx.kb.id).await?;
    Ok(Json(json!(docs)))
}

/// GET /api/kb/:kb_id/documents/:id
pub async fn get(
    ctx: KbCtx,
    State(state): State<AppState>,
    Path((_kb_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Value>> {
    let doc = load_doc(&state, ctx.kb.id, id).await?;
    Ok(Json(json!(doc)))
}

/// DELETE /api/kb/:kb_id/documents/:id
pub async fn delete(
    ctx: KbCtx,
    State(state): State<AppState>,
    Path((_kb_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let doc = load_doc(&state, ctx.kb.id, id).await?;
    db::documents::delete(&state.db, id).await?;
    if let Some(bucket) = &state.bucket {
        if let Err(e) = s3::delete_object(bucket, &doc.s3_key).await {
            tracing::warn!(doc_id = %id, error = %e, "failed to delete S3 object after DB delete");
        }
    }
    tracing::info!(doc_id = %id, kb_id = %ctx.kb.id, "document deleted");
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/kb/:kb_id/documents/:id/reindex
pub async fn reindex(
    ctx: KbCtx,
    State(state): State<AppState>,
    Path((_kb_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Value>> {
    load_doc(&state, ctx.kb.id, id).await?;
    db::documents::reset_for_reindex(&state.db, id).await?;
    state.rag_cache.invalidate(ctx.kb.id).await;
    tracing::info!(doc_id = %id, kb_id = %ctx.kb.id, "document queued for reindex");
    let doc = db::documents::get_by_id(&state.db, id).await?.unwrap();
    Ok(Json(json!(doc)))
}

/// GET /api/kb/:kb_id/documents/:id/content — proxy raw S3 file content
pub async fn content(
    ctx: KbCtx,
    State(state): State<AppState>,
    Path((_kb_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<impl IntoResponse> {
    let bucket = state.require_bucket()?;
    let doc = load_doc(&state, ctx.kb.id, id).await?;
    let bytes = s3::download_bytes(bucket, &doc.s3_key).await?;

    let disposition = format!("inline; filename=\"{}\"", doc.filename.replace('"', "_"));
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, doc.mime_type.clone()),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    ))
}

/// GET /api/kb/:kb_id/documents/:id/pages — indexed pages + root index
pub async fn pages(
    ctx: KbCtx,
    State(state): State<AppState>,
    Path((_kb_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Value>> {
    let doc = load_doc(&state, ctx.kb.id, id).await?;
    let page_list = db::page_indexes::get_pages_for_document(&state.db, id).await?;
    let doc_index = db::page_indexes::get_document_index(&state.db, id).await?;
    Ok(Json(json!({
        "document": doc,
        "pages": page_list,
        "root_index": doc_index.map(|di| di.root_index),
    })))
}

/// Load a document, verifying it belongs to the given KB.
async fn load_doc(
    state: &AppState,
    kb_id: Uuid,
    id: Uuid,
) -> AppResult<crate::models::document::Document> {
    let doc = db::documents::get_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("document {id} not found")))?;
    if doc.kb_id != kb_id {
        return Err(AppError::NotFound("document not found in this KB".into()));
    }
    Ok(doc)
}
