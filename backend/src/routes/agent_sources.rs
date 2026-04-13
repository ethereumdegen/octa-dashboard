use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
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
    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    match client
        .query(
            "SELECT id, url, label, created_at FROM agent_sources ORDER BY created_at",
            &[],
        )
        .await
    {
        Ok(rows) => {
            let sources: Vec<serde_json::Value> = rows
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.get::<_, Uuid>("id"),
                        "url": r.get::<_, String>("url"),
                        "label": r.get::<_, String>("label"),
                        "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>("created_at").to_rfc3339(),
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
    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    match client
        .query_one(
            "INSERT INTO agent_sources (url, label) VALUES ($1, $2)
             ON CONFLICT (url) DO UPDATE SET label = EXCLUDED.label
             RETURNING id, url, label, created_at",
            &[&url, &label],
        )
        .await
    {
        Ok(row) => {
            let source = serde_json::json!({
                "id": row.get::<_, Uuid>("id"),
                "url": row.get::<_, String>("url"),
                "label": row.get::<_, String>("label"),
                "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>("created_at").to_rfc3339(),
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
    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    match client
        .execute("DELETE FROM agent_sources WHERE id = $1", &[&id])
        .await
    {
        Ok(0) => StatusCode::NOT_FOUND.into_response(),
        Ok(_) => Json(serde_json::json!({"deleted": true})).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Read agent source URLs from the database.
pub async fn get_source_urls(pool: &deadpool_postgres::Pool) -> Vec<String> {
    let client = match pool.get().await {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    match client
        .query("SELECT url FROM agent_sources ORDER BY created_at", &[])
        .await
    {
        Ok(rows) => rows.iter().map(|r| r.get::<_, String>("url")).collect(),
        Err(_) => vec![],
    }
}
