use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::github::GithubClient;
use crate::storage::{self, CreateRepo};
use crate::AppState;

fn err(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

fn is_valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

// ── Repos ───────────────────────────────────────────────────────────────

pub async fn list_repos(State(state): State<AppState>) -> impl IntoResponse {
    match storage::list_repos(&state.db).await {
        Ok(repos) => Json(serde_json::json!(repos)).into_response(),
        Err(e) => err(e).into_response(),
    }
}

pub async fn add_repo(
    State(state): State<AppState>,
    Json(mut body): Json<CreateRepo>,
) -> impl IntoResponse {
    if !is_valid_name(&body.owner) || !is_valid_name(&body.repo) {
        return (StatusCode::BAD_REQUEST, "invalid owner/repo".to_string()).into_response();
    }

    // Resolve the real default branch from GitHub when a token is available.
    if body.default_branch.is_none() {
        if let Some(token) = state.config.resolve_github_token().await {
            let gh = GithubClient::new(&state.http, &token);
            if let Ok(branch) = gh.default_branch(&body.owner, &body.repo).await {
                body.default_branch = Some(branch);
            }
        }
    }

    match storage::create_repo(&state.db, body).await {
        Ok(repo) => (StatusCode::CREATED, Json(serde_json::json!(repo))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

pub async fn delete_repo(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match storage::delete_repo(&state.db, id).await {
        Ok(true) => Json(serde_json::json!({ "deleted": true })).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err(e).into_response(),
    }
}

// ── Commits ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CommitsQuery {
    pub limit: Option<i64>,
}

pub async fn list_commits(
    State(state): State<AppState>,
    Query(q): Query<CommitsQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    match storage::list_commits(&state.db, limit).await {
        Ok(commits) => Json(serde_json::json!(commits)).into_response(),
        Err(e) => err(e).into_response(),
    }
}

pub async fn get_commit(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match storage::get_commit(&state.db, id).await {
        Ok(Some(commit)) => Json(serde_json::json!(commit)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err(e).into_response(),
    }
}

// ── Trigger ─────────────────────────────────────────────────────────────

/// Kick off a watch tick immediately (does not wait for the 5-minute timer).
pub async fn trigger(State(state): State<AppState>) -> impl IntoResponse {
    let pool = state.db.clone();
    let http = state.http.clone();
    let config = state.config.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::worker::run_once(&pool, &http, &config).await {
            tracing::warn!("[Watcher] manual trigger error: {e}");
        }
    });
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "status": "started" })),
    )
}
