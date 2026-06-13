use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// Create a connection pool. Connects lazily so this stays synchronous and the
/// process can boot even if Postgres isn't reachable yet (first query retries).
pub fn create_pool(database_url: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(10)
        .connect_lazy(database_url)
        .expect("Failed to create database pool")
}
