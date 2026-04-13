use axum::{
    extract::{Path, State},
    Json,
};

use crate::db::models::Agent;
use crate::error::AppError;
use crate::AppState;

pub async fn list_agents(State(state): State<AppState>) -> Result<Json<Vec<Agent>>, AppError> {
    let client = state.pool.get().await?;
    let rows = client
        .query(
            "SELECT id, name, url, manifest, status, last_health_check, registered_at
             FROM agents ORDER BY name",
            &[],
        )
        .await?;

    let agents: Vec<Agent> = rows
        .iter()
        .map(|r| Agent {
            id: r.get("id"),
            name: r.get("name"),
            url: r.get("url"),
            manifest: r.get("manifest"),
            status: r.get("status"),
            last_health_check: r.get("last_health_check"),
            registered_at: r.get("registered_at"),
        })
        .collect();

    Ok(Json(agents))
}

pub async fn discover(State(state): State<AppState>) -> Result<Json<Vec<Agent>>, AppError> {
    let urls = super::agent_sources::get_source_urls(&state.pool).await;
    crate::agents::registry::discover_agents(&state.pool, &urls).await;

    // Return the updated agent list
    list_agents(State(state)).await
}

pub async fn agent_health(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let client = state.pool.get().await?;
    let row = client
        .query_opt("SELECT url FROM agents WHERE id = $1", &[&id])
        .await?
        .ok_or_else(|| AppError::NotFound("Agent not found".into()))?;

    let url: String = row.get("url");
    let health_url = format!("{url}/health");

    let mut req = reqwest::Client::new().get(&health_url);
    let agent_secret = &state.config.agent_secret;
    if !agent_secret.is_empty() {
        req = req.header("Authorization", format!("Bearer {agent_secret}"));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Health check failed: {e}")))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Invalid health response: {e}")))?;

    Ok(Json(body))
}
