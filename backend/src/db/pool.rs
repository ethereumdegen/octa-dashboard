use deadpool_postgres::{Config, Pool, Runtime};
use tokio_postgres::NoTls;

pub fn create_pool(database_url: &str) -> Pool {
    let mut cfg = Config::new();
    cfg.url = Some(database_url.to_string());
    cfg.create_pool(Some(Runtime::Tokio1), NoTls)
        .expect("Failed to create database pool")
}
