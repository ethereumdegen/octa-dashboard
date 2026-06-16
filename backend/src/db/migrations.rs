use include_dir::{include_dir, Dir};
use sqlx::PgPool;
use tracing::info;

/// All `*.sql` files under this directory are embedded at compile time and applied
/// in filename order. Drop a new `0NN_name.sql` file in and it runs automatically —
/// no registration needed. The numeric prefix (001_, 002_, …) determines order.
static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/db/migrations");

pub async fn run_migrations(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS _migrations (
            name TEXT PRIMARY KEY,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )",
    )
    .execute(pool)
    .await?;

    // Collect embedded .sql files and apply in filename order (zero-padded prefixes sort correctly).
    let mut files: Vec<_> = MIGRATIONS_DIR
        .files()
        .filter(|f| f.path().extension().and_then(|e| e.to_str()) == Some("sql"))
        .collect();
    files.sort_by_key(|f| f.path().to_path_buf());

    for file in files {
        // Migration name is the filename without extension, e.g. "020_watcher".
        let name = file
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("migration filename must be valid UTF-8");
        let sql = file
            .contents_utf8()
            .expect("migration SQL must be valid UTF-8");

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
