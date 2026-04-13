mod agents;
mod auth;
mod config;
mod db;
mod error;
mod routes;

use axum::{
    middleware,
    routing::{any, delete, get, post, put},
    Extension, Router,
};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

use config::Config;
use db::pool::create_pool;

#[derive(Clone)]
pub struct AppState {
    pub pool: deadpool_postgres::Pool,
    pub config: Config,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = Config::from_env();
    let pool = create_pool(&config.database_url);

    // Seed initial admin
    if !config.initial_admin_email.is_empty() {
        let client = pool.get().await.expect("DB connection for seeding");
        let count: i64 = client
            .query_one("SELECT COUNT(*) as count FROM team_members", &[])
            .await
            .expect("Count team_members")
            .get("count");
        if count == 0 {
            client
                .execute(
                    "INSERT INTO team_members (email, role) VALUES ($1, 'admin')",
                    &[&config.initial_admin_email],
                )
                .await
                .expect("Seed initial admin");
            info!("Seeded initial admin: {}", config.initial_admin_email);
        }
    }

    // Seed agent sources from AGENT_URLS env var (won't overwrite existing)
    if !config.agent_urls.is_empty() {
        let client = pool.get().await.expect("DB connection for agent source seeding");
        for url in &config.agent_urls {
            let _ = client
                .execute(
                    "INSERT INTO agent_sources (url) VALUES ($1) ON CONFLICT (url) DO NOTHING",
                    &[url],
                )
                .await;
        }
        info!("Seeded {} agent source(s) from AGENT_URLS", config.agent_urls.len());
    }

    // Discover agents from DB sources
    let source_urls = routes::agent_sources::get_source_urls(&pool).await;
    agents::registry::discover_agents(&pool, &source_urls).await;

    // Start health checker (reads sources from DB each cycle)
    agents::health::start_health_checker(pool.clone());

    let state = AppState {
        pool: pool.clone(),
        config: config.clone(),
    };

    let jwt_secret = config.jwt_secret.clone();

    // Public routes
    let public_routes = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route(
            "/auth/callback",
            post(routes::auth::github_callback).get(routes::auth::github_callback),
        )
        .route(
            "/internal/platform-secrets/{key}",
            get(routes::platform_secrets::get_secret_value_internal),
        );

    // Auth-required routes
    let auth_routes = Router::new()
        .route("/api/auth/me", get(routes::auth::me))
        .route("/api/auth/logout", post(routes::auth::logout))
        .route("/api/team", get(routes::team::list_team))
        .route("/api/agents", get(routes::agents::list_agents))
        .route("/api/agents/discover", post(routes::agents::discover))
        .route("/api/agents/{id}/health", get(routes::agents::agent_health))
        .route("/api/analytics/summary", get(routes::analytics::summary))
        .route(
            "/api/analytics/chart/{metric}",
            get(routes::analytics::chart),
        )
        .route(
            "/api/agents/{id}/proxy/{*path}",
            any(routes::proxy::proxy_agent),
        )
        // API key management
        .route(
            "/api/api-keys",
            get(routes::api_keys::list_api_keys).post(routes::api_keys::create_api_key),
        )
        .route(
            "/api/api-keys/{id}",
            delete(routes::api_keys::revoke_api_key),
        )
        .route(
            "/api/microservices",
            get(routes::microservices::list_microservices),
        )
        .route(
            "/api/services",
            get(routes::microservices::list_services),
        )
        .route(
            "/api/platform-secrets",
            get(routes::platform_secrets::list_secrets),
        )
        .route(
            "/api/platform-secrets/{key}/value",
            get(routes::platform_secrets::get_secret_value),
        )
        .route(
            "/api/agent-sources",
            get(routes::agent_sources::list_sources),
        )
        .layer(middleware::from_fn(auth::middleware::require_auth));

    // Admin-only routes
    let admin_routes = Router::new()
        .route("/api/team", post(routes::team::add_member))
        .route("/api/team/{id}", delete(routes::team::remove_member))
        .route(
            "/api/microservices/{id}",
            put(routes::microservices::toggle_microservice),
        )
        .route(
            "/api/platform-secrets",
            post(routes::platform_secrets::create_secret),
        )
        .route(
            "/api/platform-secrets/{key}",
            put(routes::platform_secrets::set_secret).delete(routes::platform_secrets::delete_secret),
        )
        .route(
            "/api/agent-sources",
            post(routes::agent_sources::add_source),
        )
        .route(
            "/api/agent-sources/{id}",
            delete(routes::agent_sources::remove_source),
        )
        .layer(middleware::from_fn(auth::middleware::require_admin));

    // CORS: in production the frontend is served from the same origin,
    // so we only need permissive CORS in dev (SKIP_LOGIN mode).
    let cors = if config.skip_login {
        CorsLayer::permissive()
    } else {
        CorsLayer::new()
            .allow_origin(tower_http::cors::AllowOrigin::mirror_request())
            .allow_methods([
                http::Method::GET,
                http::Method::POST,
                http::Method::PUT,
                http::Method::DELETE,
                http::Method::OPTIONS,
            ])
            .allow_headers([
                http::header::CONTENT_TYPE,
                http::header::AUTHORIZATION,
                http::header::COOKIE,
            ])
            .allow_credentials(true)
    };

    let mut app = Router::new()
        .merge(public_routes)
        .merge(auth_routes)
        .merge(admin_routes)
        .layer(Extension(jwt_secret))
        .layer(Extension(auth::middleware::AuthPool(pool)));

    if config.skip_login {
        info!("SKIP_LOGIN is enabled — all requests authenticated as dev admin");
        app = app.layer(Extension(auth::middleware::SkipLogin));
    }

    let app = app.layer(cors).with_state(state);
    let mut app = app;

    // Serve static frontend files in production
    let static_dir = std::path::Path::new("static");
    if static_dir.exists() {
        info!("Serving static frontend from ./static");
        app = app.fallback_service(
            ServeDir::new("static").fallback(ServeFile::new("static/index.html")),
        );
    }

    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .expect("Failed to bind");
    info!("Dashboard server listening on {}", config.listen_addr);
    axum::serve(listener, app).await.expect("Server failed");
}
