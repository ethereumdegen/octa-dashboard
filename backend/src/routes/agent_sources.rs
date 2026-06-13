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
pub struct AddSourceRequest {
    pub url: String,
    pub label: Option<String>,
}

pub async fn list_sources(
    State(state): State<AppState>,
    _claims: Claims,
) -> impl IntoResponse {
    match sqlx::query(
        "SELECT id, url, label, created_at FROM agent_sources ORDER BY created_at",
    )
    .fetch_all(&state.pool)
    .await
    {
        Ok(rows) => {
            let sources: Vec<serde_json::Value> = rows
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.get::<Uuid, _>("id"),
                        "url": r.get::<String, _>("url"),
                        "label": r.get::<String, _>("label"),
                        "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
                    })
                })
                .collect();
            Json(serde_json::json!(sources)).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn add_source(
    State(state): State<AppState>,
    _claims: Claims,
    Json(body): Json<AddSourceRequest>,
) -> impl IntoResponse {
    let url = body.url.trim().trim_end_matches('/').to_string();
    if url.is_empty() {
        return (StatusCode::BAD_REQUEST, "URL is required".to_string()).into_response();
    }

    let label = body.label.unwrap_or_default();

    match sqlx::query(
        "INSERT INTO agent_sources (url, label) VALUES ($1, $2)
         ON CONFLICT (url) DO UPDATE SET label = EXCLUDED.label
         RETURNING id, url, label, created_at",
    )
    .bind(url.as_str())
    .bind(label.as_str())
    .fetch_one(&state.pool)
    .await
    {
        Ok(row) => {
            let source = serde_json::json!({
                "id": row.get::<Uuid, _>("id"),
                "url": row.get::<String, _>("url"),
                "label": row.get::<String, _>("label"),
                "created_at": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
            });

            // Trigger discovery for the newly added URL
            let pool = state.pool.clone();
            let discover_url = url.clone();
            tokio::spawn(async move {
                crate::agents::registry::discover_agents(&pool, &[discover_url]).await;
            });

            (StatusCode::CREATED, Json(source)).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn remove_source(
    State(state): State<AppState>,
    _claims: Claims,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match sqlx::query("DELETE FROM agent_sources WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
    {
        Ok(r) if r.rows_affected() == 0 => StatusCode::NOT_FOUND.into_response(),
        Ok(_) => Json(serde_json::json!({"deleted": true})).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Read agent source URLs from the database.
pub async fn get_source_urls(pool: &sqlx::PgPool) -> Vec<String> {
    match sqlx::query("SELECT url FROM agent_sources ORDER BY created_at")
        .fetch_all(pool)
        .await
    {
        Ok(rows) => rows.iter().map(|r| r.get::<String, _>("url")).collect(),
        Err(_) => vec![],
    }
}
