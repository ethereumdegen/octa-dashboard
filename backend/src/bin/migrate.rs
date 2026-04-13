use dashboard_server::db;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::pool::create_pool(&database_url);

    db::migrations::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    println!("All migrations applied successfully.");
}
