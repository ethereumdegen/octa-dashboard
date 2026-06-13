use agent_protocol::AgentManifest;
use sqlx::PgPool;
use tracing::{info, warn};

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

pub async fn discover_agents(pool: &PgPool, agent_urls: &[String]) {
    let secret = std::env::var("AGENT_SECRET").unwrap_or_default();

    let http = reqwest::Client::new();

    for url in agent_urls {
        let manifest_url = format!("{url}/.well-known/agent.json");
        let mut req = http.get(&manifest_url);
        if !secret.is_empty() {
            req = req.header("Authorization", format!("Bearer {secret}"));
        }

        match req.send().await {
            Ok(resp) => match resp.json::<AgentManifest>().await {
                Ok(manifest) => {
                    let manifest_json = serde_json::to_value(&manifest).unwrap_or_default();
                    let _ = sqlx::query(
                        "INSERT INTO agents (id, name, url, manifest, status, registered_at)
                         VALUES ($1, $2, $3, $4, 'healthy', now())
                         ON CONFLICT (id) DO UPDATE SET
                            name = EXCLUDED.name,
                            url = EXCLUDED.url,
                            manifest = EXCLUDED.manifest,
                            status = 'healthy'",
                    )
                    .bind(manifest.id.as_str())
                    .bind(manifest.name.as_str())
                    .bind(url.as_str())
                    .bind(&manifest_json)
                    .execute(pool)
                    .await;
                    info!("Registered agent: {} ({})", manifest.name, manifest.id);

                    // Auto-create/update microservice record from manifest data.
                    // Only create for agents that have a UI (they need a sidebar entry).
                    if manifest.ui.is_some() {
                        let slug = &manifest.id;
                        let nav_path = format!("/app/{slug}");
                        let icon = manifest
                            .icon
                            .as_deref()
                            .map(|i| capitalize_first(i))
                            .unwrap_or_else(|| "Box".to_string());

                        let _ = sqlx::query(
                            "INSERT INTO microservices (id, name, description, icon, slug, nav_path, enabled, source_url, installed_at)
                             VALUES ($1, $2, $3, $4, $5, $6, false, $7, now())
                             ON CONFLICT (id) DO UPDATE SET
                                name = EXCLUDED.name,
                                description = EXCLUDED.description,
                                icon = EXCLUDED.icon,
                                slug = EXCLUDED.slug,
                                nav_path = EXCLUDED.nav_path,
                                source_url = EXCLUDED.source_url",
                        )
                        .bind(manifest.id.as_str())
                        .bind(manifest.name.as_str())
                        .bind(manifest.description.as_str())
                        .bind(icon.as_str())
                        .bind(slug.as_str())
                        .bind(nav_path.as_str())
                        .bind(url.as_str())
                        .execute(pool)
                        .await;
                        info!("Upserted microservice from manifest: {} (slug={})", manifest.name, slug);
                    }
                }
                Err(e) => warn!("Failed to parse manifest from {url}: {e}"),
            },
            Err(e) => warn!("Failed to fetch manifest from {url}: {e}"),
        }
    }
}
