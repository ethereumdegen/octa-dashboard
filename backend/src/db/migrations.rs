use deadpool_postgres::Pool;
use tracing::info;

const MIGRATIONS: &[(&str, &str)] = &[
    ("001_users", include_str!("migrations/001_users.sql")),
    ("002_team_members", include_str!("migrations/002_team_members.sql")),
    ("003_agents", include_str!("migrations/003_agents.sql")),
    ("004_kb_documents", include_str!("migrations/004_kb_documents.sql")),
    ("005_analytics_events", include_str!("migrations/005_analytics_events.sql")),
    ("006_api_keys", include_str!("migrations/006_api_keys.sql")),
    ("007_kb_is_folder", include_str!("migrations/007_kb_is_folder.sql")),
    ("008_microservices", include_str!("migrations/008_microservices.sql")),
    // Migrations 009-010 removed (security-agent and pool-security-agent)
    ("011_platform_secrets", include_str!("migrations/011_platform_secrets.sql")),
    ("012_projects", include_str!("migrations/012_projects.sql")),
    ("013_api_key_suffix", include_str!("migrations/013_api_key_suffix.sql")),
    ("014_agent_sources", include_str!("migrations/014_agent_sources.sql")),
    ("015_microservices_data_driven", include_str!("migrations/015_microservices_data_driven.sql")),
    ("016_platform_secrets_description", include_str!("migrations/016_platform_secrets_description.sql")),
];

pub async fn run_migrations(pool: &Pool) -> Result<(), Box<dyn std::error::Error>> {
    let client = pool.get().await?;

    client
        .execute(
            "CREATE TABLE IF NOT EXISTS _migrations (
                name TEXT PRIMARY KEY,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
            )",
            &[],
        )
        .await?;

    for (name, sql) in MIGRATIONS {
        let applied = client
            .query_opt("SELECT 1 FROM _migrations WHERE name = $1", &[name])
            .await?;

        if applied.is_none() {
            info!("Applying migration: {name}");
            client.batch_execute(sql).await?;
            client
                .execute("INSERT INTO _migrations (name) VALUES ($1)", &[name])
                .await?;
        }
    }

    info!("Migrations complete");
    Ok(())
}
