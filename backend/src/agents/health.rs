use deadpool_postgres::Pool;
use std::time::Duration;
use tracing::{info, warn};

pub fn start_health_checker(pool: Pool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            // Read agent source URLs from DB each cycle
            let urls = crate::routes::agent_sources::get_source_urls(&pool).await;
            super::registry::discover_agents(&pool, &urls).await;
            check_all_agents(&pool).await;
        }
    });
}

async fn check_all_agents(pool: &Pool) {
    let secret = std::env::var("AGENT_SECRET").unwrap_or_default();

    let client = match pool.get().await {
        Ok(c) => c,
        Err(_) => return,
    };

    let rows = match client.query("SELECT id, url FROM agents", &[]).await {
        Ok(r) => r,
        Err(_) => return,
    };

    let http = reqwest::Client::new();

    for row in &rows {
        let id: String = row.get("id");
        let url: String = row.get("url");
        let health_url = format!("{url}/health");

        let mut req = http.get(&health_url).timeout(Duration::from_secs(5));
        if !secret.is_empty() {
            req = req.header("Authorization", format!("Bearer {secret}"));
        }

        let status = match req.send().await {
            Ok(resp) if resp.status().is_success() => "healthy",
            _ => "unhealthy",
        };

        let _ = client
            .execute(
                "UPDATE agents SET status = $1, last_health_check = now() WHERE id = $2",
                &[&status, &id],
            )
            .await;

        if status == "unhealthy" {
            warn!("Agent {id} is unhealthy");
        }
    }

    info!("Health check complete for {} agents", rows.len());
}
