use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::auth::{Identity, KbCtx};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::services::s3;
use crate::state::AppState;

fn validate_slug(slug: &str) -> AppResult<()> {
    if slug.is_empty() || slug.len() > 64 {
        return Err(AppError::BadRequest("slug must be 1-64 characters".into()));
    }
    if !slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err(AppError::BadRequest("slug must be lowercase alphanumeric with dashes".into()));
    }
    if slug.starts_with('-') || slug.ends_with('-') {
        return Err(AppError::BadRequest("slug cannot start or end with dash".into()));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct CreateKb {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
}

/// GET /api/kbs — list all knowledgebases (global/shared)
pub async fn list(State(state): State<AppState>) -> AppResult<Json<serde_json::Value>> {
    let kbs = db::knowledgebases::list_all(&state.db).await?;
    Ok(Json(serde_json::json!(kbs)))
}

/// POST /api/kbs — create a new knowledgebase
pub async fn create(
    identity: Identity,
    State(state): State<AppState>,
    Json(req): Json<CreateKb>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    if req.name.trim().is_empty() || req.name.len() > 100 {
        return Err(AppError::BadRequest("name must be 1-100 characters".into()));
    }
    validate_slug(&req.slug)?;
    if let Some(ref desc) = req.description {
        if desc.len() > 500 {
            return Err(AppError::BadRequest("description must be at most 500 characters".into()));
        }
    }

    let kb = db::knowledgebases::create(
        &state.db,
        identity.user_id,
        &req.name,
        &req.slug,
        req.description.as_deref().unwrap_or(""),
        &state.config.openai_model,
    )
    .await
    .map_err(|e| {
        if let AppError::Database(ref db_err) = e {
            let msg = db_err.to_string();
            if msg.contains("duplicate key") || msg.contains("unique constraint") {
                return AppError::BadRequest("slug already exists".into());
            }
        }
        e
    })?;

    Ok((StatusCode::CREATED, Json(serde_json::json!(kb))))
}

/// GET /api/kb/:kb_id — get KB details
pub async fn get(ctx: KbCtx) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!(ctx.kb)))
}

/// DELETE /api/kb/:kb_id — delete KB + all its documents from S3
pub async fn delete(ctx: KbCtx, State(state): State<AppState>) -> AppResult<StatusCode> {
    let docs = db::documents::list_for_kb(&state.db, ctx.kb.id).await?;
    if let Some(bucket) = &state.bucket {
        for doc in &docs {
            if let Err(e) = s3::delete_object(bucket, &doc.s3_key).await {
                tracing::warn!(doc_id = %doc.id, error = %e, "failed to delete S3 object during KB deletion");
            }
        }
    }

    state.rag_cache.invalidate(ctx.kb.id).await;
    db::knowledgebases::delete(&state.db, ctx.kb.id).await?;

    tracing::info!(kb_id = %ctx.kb.id, docs_deleted = docs.len(), "knowledgebase deleted");
    Ok(StatusCode::NO_CONTENT)
}
