use axum::extract::State;
use axum::Json;
use serde::Deserialize;

use crate::auth::KbCtx;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// GET /api/kb/:kb_id/settings
pub async fn get_settings(ctx: KbCtx) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!(ctx.kb)))
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettings {
    pub name: Option<String>,
    pub description: Option<String>,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
    pub accent_color: Option<String>,
}

/// PUT /api/kb/:kb_id/settings
pub async fn update_settings(
    ctx: KbCtx,
    State(state): State<AppState>,
    Json(req): Json<UpdateSettings>,
) -> AppResult<Json<serde_json::Value>> {
    if let Some(ref name) = req.name {
        if name.trim().is_empty() || name.len() > 100 {
            return Err(AppError::BadRequest("name must be 1-100 characters".into()));
        }
    }
    if let Some(ref desc) = req.description {
        if desc.len() > 500 {
            return Err(AppError::BadRequest("description must be at most 500 characters".into()));
        }
    }
    if let Some(ref color) = req.accent_color {
        if color.len() > 20 {
            return Err(AppError::BadRequest("accent_color must be at most 20 characters".into()));
        }
    }

    let kb = &ctx.kb;
    let updated = db::knowledgebases::update(
        &state.db,
        kb.id,
        req.name.as_deref().unwrap_or(&kb.name),
        req.description.as_deref().unwrap_or(&kb.description),
        req.system_prompt.as_deref().unwrap_or(&kb.system_prompt),
        req.model.as_deref().unwrap_or(&kb.model),
        req.accent_color.as_deref().unwrap_or(&kb.accent_color),
    )
    .await?;

    // KB config (model/system_prompt) changed — drop any cached agent.
    state.rag_cache.invalidate(kb.id).await;

    Ok(Json(serde_json::json!(updated)))
}
