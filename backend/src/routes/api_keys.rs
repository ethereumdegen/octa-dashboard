use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use rand::Rng;
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use crate::auth::session::Claims;
use crate::AppState;

const DEFAULT_PROJECT_ID: &str = "00000000-0000-0000-0000-000000000001";

#[derive(serde::Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
}

#[derive(serde::Serialize)]
pub struct CreateApiKeyResponse {
    pub id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub key_suffix: String,
    pub key: String, // Only returned on creation
    pub created_at: String,
}

pub fn generate_api_key() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.r#gen();
    format!("tk_{}", hex::encode(bytes))
}

pub fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

pub async fn create_api_key(
    State(state): State<AppState>,
    _claims: Claims,
    Json(body): Json<CreateApiKeyRequest>,
) -> impl IntoResponse {
    let project_id: Uuid = DEFAULT_PROJECT_ID.parse().unwrap();
    let key = generate_api_key();
    let key_hash = hash_key(&key);
    let key_prefix = &key[..11]; // "tk_" + first 8 hex chars
    let key_suffix = &key[key.len() - 4..]; // last 4 hex chars

    match sqlx::query(
        "INSERT INTO api_keys (project_id, name, key_prefix, key_suffix, key_hash)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, name, key_prefix, key_suffix, created_at",
    )
    .bind(project_id)
    .bind(body.name.as_str())
    .bind(key_prefix)
    .bind(key_suffix)
    .bind(key_hash.as_str())
    .fetch_one(&state.pool)
    .await
    {
        Ok(row) => {
            let resp = CreateApiKeyResponse {
                id: row.get("id"),
                name: row.get("name"),
                key_prefix: row.get::<String, _>("key_prefix"),
                key_suffix: row.get::<String, _>("key_suffix"),
                key,
                created_at: row
                    .get::<chrono::DateTime<chrono::Utc>, _>("created_at")
                    .to_rfc3339(),
            };
            (StatusCode::CREATED, Json(serde_json::json!(resp))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_api_keys(
    State(state): State<AppState>,
    _claims: Claims,
) -> impl IntoResponse {
    let project_id: Uuid = DEFAULT_PROJECT_ID.parse().unwrap();

    match sqlx::query(
        "SELECT id, name, key_prefix, key_suffix, created_at, last_used_at
         FROM api_keys
         WHERE project_id = $1 AND revoked_at IS NULL
         ORDER BY created_at DESC",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await
    {
        Ok(rows) => {
            let keys: Vec<serde_json::Value> = rows
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.get::<Uuid, _>("id"),
                        "name": r.get::<String, _>("name"),
                        "key_prefix": r.get::<String, _>("key_prefix"),
                        "key_suffix": r.get::<String, _>("key_suffix"),
                        "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
                        "last_used_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("last_used_at").map(|t| t.to_rfc3339()),
                    })
                })
                .collect();
            Json(serde_json::json!(keys)).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn revoke_api_key(
    State(state): State<AppState>,
    _claims: Claims,
    Path(key_id): Path<Uuid>,
) -> impl IntoResponse {
    let project_id: Uuid = DEFAULT_PROJECT_ID.parse().unwrap();

    match sqlx::query(
        "UPDATE api_keys SET revoked_at = now()
         WHERE id = $1 AND project_id = $2 AND revoked_at IS NULL",
    )
    .bind(key_id)
    .bind(project_id)
    .execute(&state.pool)
    .await
    {
        Ok(r) if r.rows_affected() == 0 => StatusCode::NOT_FOUND.into_response(),
        Ok(_) => Json(serde_json::json!({"revoked": true})).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
