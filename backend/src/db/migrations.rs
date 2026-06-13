use sqlx::PgPool;
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
    ("017_kb_rag", include_str!("migrations/017_kb_rag.sql")),
    ("018_kb_description", include_str!("migrations/018_kb_description.sql")),
    ("019_drop_legacy_kb_documents", include_str!("migrations/019_drop_legacy_kb_documents.sql")),
];

pub async fn run_migrations(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS _migrations (
            name TEXT PRIMARY KEY,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )",
    )
    .execute(pool)
    .await?;

    for &(name, sql) in MIGRATIONS {
        let applied = sqlx::query("SELECT 1 FROM _migrations WHERE name = $1")
            .bind(name)
            .fetch_optional(pool)
            .await?;

        if applied.is_none() {
            info!("Applying migration: {name}");
            // raw_sql runs multiple statements in one batch (simple query protocol),
            // which migrations with CREATE TYPE / multiple DDL statements require.
            sqlx::raw_sql(sql).execute(pool).await?;
            sqlx::query("INSERT INTO _migrations (name) VALUES ($1)")
                .bind(name)
                .execute(pool)
                .await?;
        }
    }

    info!("Migrations complete");
    Ok(())
}
