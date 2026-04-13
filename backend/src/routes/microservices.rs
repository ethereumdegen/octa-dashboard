use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use crate::db::models::Microservice;
use crate::error::AppError;
use crate::AppState;

pub async fn list_microservices(
    State(state): State<AppState>,
) -> Result<Json<Vec<Microservice>>, AppError> {
    let client = state.pool.get().await?;
    let rows = client
        .query(
            "SELECT id, name, description, icon, slug, nav_path, enabled, source_url, installed_at
             FROM microservices ORDER BY name",
            &[],
        )
        .await?;

    let services: Vec<Microservice> = rows
        .iter()
        .map(|r| Microservice {
            id: r.get("id"),
            name: r.get("name"),
            description: r.get("description"),
            icon: r.get("icon"),
            slug: r.get("slug"),
            nav_path: r.get("nav_path"),
            enabled: r.get("enabled"),
            source_url: r.get("source_url"),
            installed_at: r.get("installed_at"),
        })
        .collect();

    Ok(Json(services))
}

#[derive(Deserialize)]
pub struct ToggleBody {
    pub enabled: bool,
}

pub async fn toggle_microservice(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ToggleBody>,
) -> Result<Json<Microservice>, AppError> {
    let client = state.pool.get().await?;
    let row = client
        .query_opt(
            "UPDATE microservices SET enabled = $1 WHERE id = $2
             RETURNING id, name, description, icon, slug, nav_path, enabled, source_url, installed_at",
            &[&body.enabled, &id],
        )
        .await?
        .ok_or_else(|| AppError::NotFound("Microservice not found".into()))?;

    Ok(Json(Microservice {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        icon: row.get("icon"),
        slug: row.get("slug"),
        nav_path: row.get("nav_path"),
        enabled: row.get("enabled"),
        source_url: row.get("source_url"),
        installed_at: row.get("installed_at"),
    }))
}

/// Enriched services list: joins microservices with agent health data.
/// Also computes `missing_secrets` for each service by comparing the manifest's
/// `required_secrets` against the keys in `platform_secrets`.
pub async fn list_services(
    State(state): State<AppState>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let client = state.pool.get().await?;

    // Fetch configured secret keys in one query
    let secret_rows = client
        .query("SELECT key FROM platform_secrets", &[])
        .await?;
    let configured_keys: std::collections::HashSet<String> = secret_rows
        .iter()
        .map(|r| r.get::<_, String>("key"))
        .collect();

    let rows = client
        .query(
            "SELECT m.id, m.name, m.description, m.icon, m.slug, m.nav_path,
                    m.enabled, m.source_url, m.installed_at,
                    a.status as agent_status, a.last_health_check, a.url as agent_url,
                    a.manifest
             FROM microservices m
             LEFT JOIN agents a ON a.id = m.id
             ORDER BY m.name",
            &[],
        )
        .await?;

    let services: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            let manifest = r.get::<_, Option<serde_json::Value>>("manifest");

            // Extract required_secrets from manifest
            let required_secrets: Vec<String> = manifest
                .as_ref()
                .and_then(|m| m.get("required_secrets"))
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();

            let missing_secrets: Vec<&String> = required_secrets
                .iter()
                .filter(|k| !configured_keys.contains(k.as_str()))
                .collect();

            serde_json::json!({
                "id": r.get::<_, String>("id"),
                "name": r.get::<_, String>("name"),
                "description": r.get::<_, String>("description"),
                "icon": r.get::<_, String>("icon"),
                "slug": r.get::<_, String>("slug"),
                "nav_path": r.get::<_, String>("nav_path"),
                "enabled": r.get::<_, bool>("enabled"),
                "source_url": r.get::<_, Option<String>>("source_url"),
                "installed_at": r.get::<_, chrono::DateTime<chrono::Utc>>("installed_at").to_rfc3339(),
                "agent_status": r.get::<_, Option<String>>("agent_status"),
                "agent_url": r.get::<_, Option<String>>("agent_url"),
                "last_health_check": r.get::<_, Option<chrono::DateTime<chrono::Utc>>>("last_health_check").map(|d| d.to_rfc3339()),
                "manifest": manifest,
                "required_secrets": required_secrets,
                "missing_secrets": missing_secrets,
            })
        })
        .collect();

    Ok(Json(services))
}
