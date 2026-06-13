use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use sqlx::Row;
use uuid::Uuid;

use crate::auth::session::Claims;
use crate::AppState;

#[derive(serde::Deserialize)]
pub struct CreateSecretRequest {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub description: String,
}

#[derive(serde::Deserialize)]
pub struct UpdateSecretRequest {
    pub value: String,
    pub description: Option<String>,
}

/// List all platform secrets (returns keys + metadata, never the values).
pub async fn list_secrets(
    State(state): State<AppState>,
    _claims: Claims,
) -> impl IntoResponse {
    let rows = match sqlx::query(
        "SELECT key, description, updated_at FROM platform_secrets ORDER BY key",
    )
    .fetch_all(&state.pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let secrets: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "key": r.get::<String, _>("key"),
                "description": r.get::<String, _>("description"),
                "is_set": true,
                "updated_at": r.get::<chrono::DateTime<chrono::Utc>, _>("updated_at").to_rfc3339(),
            })
        })
        .collect();

    Json(serde_json::json!(secrets)).into_response()
}

/// Create a new platform secret (key + value + optional description).
pub async fn create_secret(
    State(state): State<AppState>,
    claims: Claims,
    Json(body): Json<CreateSecretRequest>,
) -> impl IntoResponse {
    let key = body.key.trim().to_uppercase();
    if key.is_empty() {
        return (StatusCode::BAD_REQUEST, "Key cannot be empty".to_string()).into_response();
    }
    if body.value.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "Value cannot be empty".to_string()).into_response();
    }

    let user_id: Option<Uuid> = claims.user_id();

    match sqlx::query(
        "INSERT INTO platform_secrets (key, value, description, updated_by, updated_at)
         VALUES ($1, $2, $3, $4, now())
         ON CONFLICT (key) DO UPDATE SET value = $2, description = $3, updated_by = $4, updated_at = now()",
    )
    .bind(key.as_str())
    .bind(body.value.trim())
    .bind(body.description.trim())
    .bind(user_id)
    .execute(&state.pool)
    .await
    {
        Ok(_) => Json(serde_json::json!({"saved": true, "key": key})).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Update an existing platform secret's value (and optionally description).
pub async fn set_secret(
    State(state): State<AppState>,
    claims: Claims,
    Path(key): Path<String>,
    Json(body): Json<UpdateSecretRequest>,
) -> impl IntoResponse {
    if body.value.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "Value cannot be empty".to_string()).into_response();
    }

    let user_id: Option<Uuid> = claims.user_id();

    // Update value, and optionally update description if provided
    let result = if let Some(desc) = &body.description {
        sqlx::query(
            "UPDATE platform_secrets SET value = $1, description = $2, updated_by = $3, updated_at = now() WHERE key = $4",
        )
        .bind(body.value.trim())
        .bind(desc.trim())
        .bind(user_id)
        .bind(key.as_str())
        .execute(&state.pool)
        .await
    } else {
        sqlx::query(
            "UPDATE platform_secrets SET value = $1, updated_by = $2, updated_at = now() WHERE key = $3",
        )
        .bind(body.value.trim())
        .bind(user_id)
        .bind(key.as_str())
        .execute(&state.pool)
        .await
    };

    match result {
        Ok(r) if r.rows_affected() == 0 => {
            // Key doesn't exist yet — create it (upsert)
            match sqlx::query(
                "INSERT INTO platform_secrets (key, value, description, updated_by, updated_at)
                 VALUES ($1, $2, $3, $4, now())
                 ON CONFLICT (key) DO UPDATE SET value = $2, updated_by = $4, updated_at = now()",
            )
            .bind(key.as_str())
            .bind(body.value.trim())
            .bind(body.description.as_deref().unwrap_or(""))
            .bind(user_id)
            .execute(&state.pool)
            .await
            {
                Ok(_) => Json(serde_json::json!({"saved": true})).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        Ok(_) => Json(serde_json::json!({"saved": true})).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Delete a platform secret.
pub async fn delete_secret(
    State(state): State<AppState>,
    _claims: Claims,
    Path(key): Path<String>,
) -> impl IntoResponse {
    match sqlx::query("DELETE FROM platform_secrets WHERE key = $1")
        .bind(key.as_str())
        .execute(&state.pool)
        .await
    {
        Ok(r) if r.rows_affected() == 0 => StatusCode::NOT_FOUND.into_response(),
        Ok(_) => Json(serde_json::json!({"deleted": true})).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Internal endpoint: agents fetch a secret value by key.
/// Protected by require_auth (agents use API keys to authenticate).
pub async fn get_secret_value(
    State(state): State<AppState>,
    _claims: Claims,
    Path(key): Path<String>,
) -> impl IntoResponse {
    match sqlx::query("SELECT value FROM platform_secrets WHERE key = $1")
        .bind(key.as_str())
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(row)) => {
            let value: String = row.get("value");
            Json(serde_json::json!({"key": key, "value": value})).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Internal endpoint for agents: fetch a secret using AGENT_SECRET auth.
pub async fn get_secret_value_internal(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(key): Path<String>,
) -> impl IntoResponse {
    let agent_secret = &state.config.agent_secret;
    if agent_secret.is_empty() {
        return (StatusCode::FORBIDDEN, "AGENT_SECRET not configured").into_response();
    }
    let provided = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    // Constant-time comparison to prevent timing attacks
    use sha2::{Digest, Sha256};
    let hash_provided = Sha256::digest(provided.as_bytes());
    let hash_expected = Sha256::digest(agent_secret.as_bytes());
    if hash_provided != hash_expected {
        return (StatusCode::UNAUTHORIZED, "Invalid agent secret").into_response();
    }

    match sqlx::query("SELECT value FROM platform_secrets WHERE key = $1")
        .bind(key.as_str())
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(row)) => {
            let value: String = row.get("value");
            Json(serde_json::json!({"key": key, "value": value})).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
