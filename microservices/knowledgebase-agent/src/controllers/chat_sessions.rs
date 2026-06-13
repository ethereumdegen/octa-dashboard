use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::KbCtx;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::chat_session::ChatSession;
use crate::state::AppState;

const MAX_MESSAGE_LENGTH: usize = 32_000;

/// In global/shared mode a session is reachable by anyone hitting its KB; we only
/// verify the session actually belongs to the KB in the path.
fn verify_session_kb(session: &ChatSession, ctx: &KbCtx) -> AppResult<()> {
    if session.kb_id != ctx.kb.id {
        return Err(AppError::NotFound("session not found".into()));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct CreateSession {
    pub title: Option<String>,
}

/// POST /api/kb/:kb_id/sessions
pub async fn create_session(
    ctx: KbCtx,
    State(state): State<AppState>,
    Json(req): Json<CreateSession>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    if let Some(ref t) = req.title {
        if t.len() > 200 {
            return Err(AppError::BadRequest("title must be at most 200 characters".into()));
        }
    }
    let title = req.title.as_deref().unwrap_or("New Chat");
    let session =
        db::chat_sessions::create_session(&state.db, ctx.kb.id, ctx.identity.user_id, title).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!(session))))
}

/// GET /api/kb/:kb_id/sessions
pub async fn list_sessions(
    ctx: KbCtx,
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let sessions = db::chat_sessions::list_sessions(&state.db, ctx.kb.id).await?;
    Ok(Json(serde_json::json!(sessions)))
}

/// GET /api/kb/:kb_id/sessions/:sid — session + messages (polled by the UI)
pub async fn get_session(
    ctx: KbCtx,
    State(state): State<AppState>,
    Path((_kb_id, sid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let session = db::chat_sessions::get_session(&state.db, sid)
        .await?
        .ok_or_else(|| AppError::NotFound("session not found".into()))?;
    verify_session_kb(&session, &ctx)?;

    let messages = db::chat_sessions::get_messages(&state.db, sid).await?;
    Ok(Json(serde_json::json!({
        "session": session,
        "messages": messages,
    })))
}

/// DELETE /api/kb/:kb_id/sessions/:sid
pub async fn delete_session(
    ctx: KbCtx,
    State(state): State<AppState>,
    Path((_kb_id, sid)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let session = db::chat_sessions::get_session(&state.db, sid)
        .await?
        .ok_or_else(|| AppError::NotFound("session not found".into()))?;
    verify_session_kb(&session, &ctx)?;

    db::chat_sessions::delete_session(&state.db, sid).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct SendMessage {
    pub content: String,
}

/// POST /api/kb/:kb_id/sessions/:sid/messages
/// Saves the user message and enqueues a background job; returns immediately.
/// The UI polls `get_session` until the assistant reply appears.
pub async fn send_message(
    ctx: KbCtx,
    State(state): State<AppState>,
    Path((_kb_id, sid)): Path<(Uuid, Uuid)>,
    Json(req): Json<SendMessage>,
) -> AppResult<Json<serde_json::Value>> {
    let session = db::chat_sessions::get_session(&state.db, sid)
        .await?
        .ok_or_else(|| AppError::NotFound("session not found".into()))?;
    verify_session_kb(&session, &ctx)?;

    if req.content.trim().is_empty() || req.content.len() > MAX_MESSAGE_LENGTH {
        return Err(AppError::BadRequest(format!(
            "message must be 1-{MAX_MESSAGE_LENGTH} characters"
        )));
    }

    // Set session title from the first message if still default
    if session.title == "New Chat" {
        let title = crate::utils::truncate_at_char(&req.content, 50);
        let _ = db::chat_sessions::update_title(&state.db, sid, &title).await;
    }

    let user_msg = db::chat_sessions::add_message(
        &state.db,
        sid,
        crate::models::chat_session::ChatRole::User,
        &req.content,
        None,
    )
    .await?;

    db::chat_jobs::create(&state.db, sid, ctx.kb.id, ctx.identity.user_id, &req.content).await?;

    Ok(Json(serde_json::json!(user_msg)))
}
