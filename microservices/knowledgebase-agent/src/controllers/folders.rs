use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::auth::KbCtx;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::folder::{
    CreateFolderRequest, MoveDocumentRequest, MoveFolderRequest, RenameFolderRequest,
    UpdateCategoryRequest,
};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub parent_id: Option<Uuid>,
}

/// POST /api/kb/:kb_id/folders
pub async fn create(
    ctx: KbCtx,
    State(state): State<AppState>,
    Json(req): Json<CreateFolderRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    let name = req.name.trim().to_string();
    if name.is_empty() || name.len() > 200 {
        return Err(AppError::BadRequest("folder name must be 1-200 characters".into()));
    }
    if let Some(ref cat) = req.category {
        if cat.len() > 100 {
            return Err(AppError::BadRequest("category must be at most 100 characters".into()));
        }
    }

    if let Some(pid) = req.parent_id {
        let parent = db::folders::get_by_id(&state.db, pid)
            .await?
            .ok_or_else(|| AppError::NotFound("parent folder not found".into()))?;
        if parent.kb_id != ctx.kb.id {
            return Err(AppError::NotFound("parent folder not found in this KB".into()));
        }
    }

    let folder = db::folders::insert(
        &state.db,
        ctx.kb.id,
        req.parent_id,
        &name,
        req.category.as_deref(),
        ctx.identity.user_id,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(json!(folder))))
}

/// GET /api/kb/:kb_id/folders?parent_id=
pub async fn list(
    ctx: KbCtx,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    let folders = db::folders::list_children(&state.db, ctx.kb.id, query.parent_id).await?;
    let documents = db::documents::list_for_folder(&state.db, ctx.kb.id, query.parent_id).await?;
    let breadcrumb = if let Some(pid) = query.parent_id {
        db::folders::breadcrumb(&state.db, pid).await?
    } else {
        vec![]
    };

    Ok(Json(json!({
        "folders": folders,
        "documents": documents,
        "breadcrumb": breadcrumb,
    })))
}

/// PUT /api/kb/:kb_id/folders/:id/rename
pub async fn rename(
    ctx: KbCtx,
    State(state): State<AppState>,
    Path((_kb_id, id)): Path<(Uuid, Uuid)>,
    Json(req): Json<RenameFolderRequest>,
) -> AppResult<Json<Value>> {
    verify_folder_kb(&state, ctx.kb.id, id).await?;
    let name = req.name.trim().to_string();
    if name.is_empty() || name.len() > 200 {
        return Err(AppError::BadRequest("folder name must be 1-200 characters".into()));
    }
    let updated = db::folders::rename(&state.db, id, &name).await?;
    Ok(Json(json!(updated)))
}

/// PUT /api/kb/:kb_id/folders/:id/move
pub async fn move_folder(
    ctx: KbCtx,
    State(state): State<AppState>,
    Path((_kb_id, id)): Path<(Uuid, Uuid)>,
    Json(req): Json<MoveFolderRequest>,
) -> AppResult<Json<Value>> {
    verify_folder_kb(&state, ctx.kb.id, id).await?;
    if let Some(target) = req.parent_id {
        if db::folders::is_descendant(&state.db, id, target).await? {
            return Err(AppError::BadRequest(
                "cannot move folder into its own descendant".into(),
            ));
        }
    }
    let updated = db::folders::move_folder(&state.db, id, req.parent_id).await?;
    Ok(Json(json!(updated)))
}

/// PUT /api/kb/:kb_id/folders/:id/category
pub async fn update_category(
    ctx: KbCtx,
    State(state): State<AppState>,
    Path((_kb_id, id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateCategoryRequest>,
) -> AppResult<Json<Value>> {
    verify_folder_kb(&state, ctx.kb.id, id).await?;
    let updated = db::folders::update_category(&state.db, id, req.category.as_deref()).await?;
    Ok(Json(json!(updated)))
}

/// DELETE /api/kb/:kb_id/folders/:id
pub async fn delete(
    ctx: KbCtx,
    State(state): State<AppState>,
    Path((_kb_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    verify_folder_kb(&state, ctx.kb.id, id).await?;
    db::folders::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// PUT /api/kb/:kb_id/documents/:id/move
pub async fn move_document(
    ctx: KbCtx,
    State(state): State<AppState>,
    Path((_kb_id, id)): Path<(Uuid, Uuid)>,
    Json(req): Json<MoveDocumentRequest>,
) -> AppResult<Json<Value>> {
    let doc = db::documents::get_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("document not found".into()))?;
    if doc.kb_id != ctx.kb.id {
        return Err(AppError::NotFound("document not found in this KB".into()));
    }
    if let Some(fid) = req.folder_id {
        let folder = db::folders::get_by_id(&state.db, fid)
            .await?
            .ok_or_else(|| AppError::NotFound("target folder not found".into()))?;
        if folder.kb_id != ctx.kb.id {
            return Err(AppError::NotFound("target folder not found in this KB".into()));
        }
    }
    db::documents::move_to_folder(&state.db, id, req.folder_id).await?;
    let doc = db::documents::get_by_id(&state.db, id).await?.unwrap();
    Ok(Json(json!(doc)))
}

async fn verify_folder_kb(state: &AppState, kb_id: Uuid, folder_id: Uuid) -> AppResult<()> {
    let folder = db::folders::get_by_id(&state.db, folder_id)
        .await?
        .ok_or_else(|| AppError::NotFound("folder not found".into()))?;
    if folder.kb_id != kb_id {
        return Err(AppError::NotFound("folder not found in this KB".into()));
    }
    Ok(())
}
