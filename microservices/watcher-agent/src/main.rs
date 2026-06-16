mod config;
mod github;
mod routes;
mod storage;
mod worker;

use std::time::Instant;

use agent_protocol::{AgentApiConfig, AgentManifest, AgentSecret, AgentUiConfig, HealthResponse};
use axum::{
    middleware,
    routing::{delete, get, post},
    Extension, Json, Router,
};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

use config::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub http: reqwest::Client,
    pub config: Config,
    pub start_time: Instant,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = Config::from_env();

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await
        .expect("failed to connect to database");
    info!("connected to database");

    let http = reqwest::Client::new();
    let state = AppState {
        db: db.clone(),
        http: http.clone(),
        config: config.clone(),
        start_time: Instant::now(),
    };

    // Background workers: watch (5 min) + LLM summaries (30 s).
    tokio::spawn(worker::start_watch_loop(
        db.clone(),
        http.clone(),
        config.clone(),
    ));
    tokio::spawn(worker::start_summary_loop(db, http, config.clone()));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_routes = Router::new()
        .route("/api/repos", get(routes::list_repos))
        .route("/api/repos", post(routes::add_repo))
        .route("/api/repos/{id}", delete(routes::delete_repo))
        .route("/api/commits", get(routes::list_commits))
        .route("/api/commits/{id}", get(routes::get_commit))
        .route("/api/trigger", post(routes::trigger))
        .layer(middleware::from_fn(agent_protocol::require_agent_secret));

    let mut app = Router::new()
        .route("/.well-known/agent.json", get(manifest))
        .route("/health", get(health))
        .merge(api_routes)
        .layer(Extension(AgentSecret(config.agent_secret.clone())))
        .layer(cors)
        .with_state(state);

    let static_dir = std::path::Path::new(&config.static_dir);
    if static_dir.exists() {
        info!("Serving watcher UI from {}", config.static_dir);
        app = app.nest_service(
            "/ui",
            ServeDir::new(static_dir).fallback(ServeFile::new(static_dir.join("index.html"))),
        );
    } else {
        info!("No static directory at '{}', UI not served", config.static_dir);
    }

    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .expect("Failed to bind");
    info!("Watcher agent listening on {}", config.listen_addr);
    axum::serve(listener, app).await.expect("Server failed");
}

async fn manifest() -> Json<AgentManifest> {
    Json(AgentManifest {
        id: "watcher".to_string(),
        name: "Watcher".to_string(),
        version: "0.1.0".to_string(),
        // Keep this keyword-free; the icon "W" is rendered as a letter glyph by
        // the dashboard sidebar (it is not a Lucide icon name).
        description: "Tracks recent GitHub commits across linked repositories".to_string(),
        icon: Some("W".to_string()),
        ui: Some(AgentUiConfig {
            entry_path: "/ui/".to_string(),
            width: None,
            height: None,
            bundle_js: Some("watcher.js".to_string()),
            bundle_css: Some("watcher.css".to_string()),
        }),
        api: Some(AgentApiConfig {
            base_path: "/api".to_string(),
        }),
        health_endpoint: "/health".to_string(),
        capabilities: vec!["ui".to_string(), "api".to_string()],
        // Surfaced in the dashboard as "missing secrets" until configured.
        required_secrets: vec!["GITHUB_TOKEN".to_string(), "OPENAI_API_KEY".to_string()],
    })
}

async fn health(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
    })
}
