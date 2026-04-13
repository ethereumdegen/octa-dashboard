mod config;
mod routes;
mod storage;

use agent_protocol::{AgentApiConfig, AgentManifest, AgentSecret, AgentUiConfig, HealthResponse};
use axum::{middleware, routing::{delete, get, post, put}, Extension, Json, Router};
use deadpool_postgres::{Config as PgConfig, Pool, Runtime};
use std::time::Instant;
use tokio_postgres::NoTls;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

use config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: Pool,
    pub start_time: Instant,
}

fn create_pool(database_url: &str) -> Pool {
    let mut cfg = PgConfig::new();
    cfg.url = Some(database_url.to_string());
    cfg.create_pool(Some(Runtime::Tokio1), NoTls)
        .expect("Failed to create database pool")
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
    let start_time = Instant::now();

    let state = AppState { pool, start_time };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Protected API routes
    let api_routes = Router::new()
        .route("/api/documents", get(routes::list_documents))
        .route("/api/documents", post(routes::create_document))
        .route("/api/documents/search", get(routes::search_documents))
        .route("/api/documents/{id}", get(routes::get_document))
        .route("/api/documents/{id}", put(routes::update_document))
        .route("/api/documents/{id}", delete(routes::delete_document))
        .route("/api/export", get(routes::export_documents))
        .route("/api/import", post(routes::import_documents))
        .layer(middleware::from_fn(agent_protocol::require_agent_secret));

    // Public routes (health + manifest for discovery)
    let mut app = Router::new()
        .route("/.well-known/agent.json", get(manifest))
        .route("/health", get(health))
        .merge(api_routes)
        .layer(Extension(AgentSecret(config.agent_secret.clone())))
        .layer(cors)
        .with_state(state.clone());

    // Serve built frontend UI from static directory
    let static_dir = std::path::Path::new(&config.static_dir);
    if static_dir.exists() {
        info!("Serving knowledgebase UI from {}", config.static_dir);
        app = app.nest_service(
            "/ui",
            ServeDir::new(static_dir).fallback(ServeFile::new(static_dir.join("index.html"))),
        );
    } else {
        info!("No static directory found at '{}', UI not served", config.static_dir);
    }

    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .expect("Failed to bind");
    info!("Knowledgebase agent listening on {}", config.listen_addr);
    axum::serve(listener, app).await.expect("Server failed");
}

async fn manifest() -> Json<AgentManifest> {
    Json(AgentManifest {
        id: "knowledgebase".to_string(),
        name: "Knowledgebase".to_string(),
        version: "0.1.0".to_string(),
        description: "Obsidian-style knowledgebase with markdown documents".to_string(),
        icon: Some("book".to_string()),
        ui: Some(AgentUiConfig {
            entry_path: "/ui/".to_string(),
            width: None,
            height: None,
            bundle_js: Some("kb.js".to_string()),
            bundle_css: Some("kb.css".to_string()),
        }),
        api: Some(AgentApiConfig {
            base_path: "/api".to_string(),
        }),
        health_endpoint: "/health".to_string(),
        capabilities: vec!["ui".to_string(), "api".to_string()],
    })
}

async fn health(axum::extract::State(state): axum::extract::State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
    })
}
