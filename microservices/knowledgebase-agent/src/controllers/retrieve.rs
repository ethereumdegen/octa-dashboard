use axum::extract::State;
use axum::Json;
use serde::Deserialize;

use crate::auth::KbCtx;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct RetrieveRequest {
    pub query: String,
    pub max_pages: Option<usize>,
}

/// POST /api/kb/:kb_id/retrieve — RAG retrieval only, no LLM synthesis
pub async fn retrieve(
    ctx: KbCtx,
    State(state): State<AppState>,
    Json(req): Json<RetrieveRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if req.query.trim().is_empty() || req.query.len() > 32_000 {
        return Err(AppError::BadRequest("query must be 1-32000 characters".into()));
    }

    let max_pages = req.max_pages.unwrap_or(10).min(100);
    let documents = state
        .rag_cache
        .retrieve(ctx.kb.id, &req.query, max_pages)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "documents": documents })))
}
