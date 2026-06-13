//! Auth here is delegated to the wrapper dashboard. The agent-secret middleware
//! (applied at the router level) already gates every `/api/...` request, and the
//! dashboard proxy forwards the authenticated user's identity via `x-user-id` /
//! `x-user-email` headers. These extractors just surface that identity and load
//! the target KB — there is NO ownership/role checking (knowledgebases are global
//! and shared in this build).

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use std::convert::Infallible;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::knowledgebase::Knowledgebase;
use crate::state::AppState;

/// Identity forwarded by the wrapper. Both fields are optional — a request may
/// arrive without them (e.g. direct dev calls); callers treat that as anonymous.
#[derive(Debug, Clone, Default)]
pub struct Identity {
    pub user_id: Option<Uuid>,
    pub email: Option<String>,
}

impl<S: Send + Sync> FromRequestParts<S> for Identity {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Infallible> {
        let user_id = parts
            .headers
            .get("x-user-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| Uuid::parse_str(s).ok());
        let email = parts
            .headers
            .get("x-user-email")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        Ok(Identity { user_id, email })
    }
}

/// Loads the KB named by `kb_id` in the path plus the caller's identity. Global
/// scope — any authenticated caller may access any KB.
pub struct KbCtx {
    pub kb: Knowledgebase,
    pub identity: Identity,
}

impl FromRequestParts<AppState> for KbCtx {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let kb_id = extract_kb_id(parts)?
            .ok_or_else(|| AppError::BadRequest("kb_id not found in path".into()))?;
        let kb = db::knowledgebases::get_by_id(&state.db, kb_id)
            .await?
            .ok_or_else(|| AppError::NotFound("knowledgebase not found".into()))?;
        let Ok(identity) = Identity::from_request_parts(parts, state).await;
        Ok(KbCtx { kb, identity })
    }
}

/// Extract the `{kb_id}` from a `/api/kb/{kb_id}/...` path.
fn extract_kb_id(parts: &Parts) -> Result<Option<Uuid>, AppError> {
    let path = parts.uri.path();
    let segments: Vec<&str> = path.split('/').collect();
    for (i, seg) in segments.iter().enumerate() {
        if *seg == "kb" {
            if let Some(id_str) = segments.get(i + 1) {
                return Uuid::parse_str(id_str)
                    .map(Some)
                    .map_err(|_| AppError::BadRequest("invalid kb_id".into()));
            }
        }
    }
    Ok(None)
}
