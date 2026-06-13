mod auth;
mod config;
mod controllers;
mod db;
mod error;
#[cfg(test)]
mod eval;
mod models;
mod services;
mod state;
mod utils;

use std::sync::Arc;
use std::time::Instant;

use agent_protocol::{
    AgentApiConfig, AgentManifest, AgentSecret, AgentUiConfig, HealthResponse,
};
use axum::routing::{delete, get, post, put};
use axum::{middleware, Extension, Json, Router};
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

use config::Config;
use services::rag_cache::RagCache;
use state::AppState;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let mut config = Config::from_env();
    config.resolve_secrets().await;

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .expect("failed to connect to database");
    info!("connected to database");

    // S3 (optional — upload/indexing disabled if unset)
    let bucket = match config.s3_config() {
        Some(s3_config) => match services::s3::create_bucket(&s3_config) {
            Ok(b) => {
                info!("S3 bucket connected");
                Some(Arc::new(b))
            }
            Err(e) => {
                tracing::error!("S3 init failed: {e} — document upload disabled");
                None
            }
        },
        None => None,
    };

    let rag_cache = RagCache::new(db.clone(), config.openai_api_key.clone());

    let state = AppState {
        db: db.clone(),
        bucket,
        config: Arc::new(config.clone()),
        rag_cache: Arc::new(rag_cache),
        start_time: Instant::now(),
    };

    // Background indexer — needs both S3 (for doc bytes) and an OpenAI key.
    if state.bucket.is_some() && !config.openai_api_key.is_empty() {
        let indexer_state = state.clone();
        tokio::spawn(async move {
            services::indexer::run_indexer_loop(indexer_state).await;
        });
    } else {
        info!("indexer not started (requires S3 + OPENAI_API_KEY)");
    }

    // Cache eviction
    {
        let evict_state = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                evict_state.rag_cache.evict_stale().await;
            }
        });
    }

    // Chat worker pool + stale-job cleanup
    services::chat_worker::spawn_workers(state.clone());
    {
        let cleanup_state = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                services::chat_worker::cleanup_stale_jobs(&cleanup_state).await;
            }
        });
    }

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Protected API routes (gated by the wrapper's agent secret)
    let api_routes = Router::new()
        .route("/api/kbs", get(controllers::knowledgebases::list).post(controllers::knowledgebases::create))
        .route("/api/kb/{kb_id}", get(controllers::knowledgebases::get).delete(controllers::knowledgebases::delete))
        .route("/api/kb/{kb_id}/settings", get(controllers::settings::get_settings).put(controllers::settings::update_settings))
        // Folders
        .route("/api/kb/{kb_id}/folders", post(controllers::folders::create).get(controllers::folders::list))
        .route("/api/kb/{kb_id}/folders/{id}/rename", put(controllers::folders::rename))
        .route("/api/kb/{kb_id}/folders/{id}/move", put(controllers::folders::move_folder))
        .route("/api/kb/{kb_id}/folders/{id}/category", put(controllers::folders::update_category))
        .route("/api/kb/{kb_id}/folders/{id}", delete(controllers::folders::delete))
        // Documents
        .route("/api/kb/{kb_id}/documents", post(controllers::documents::upload).get(controllers::documents::list))
        .route("/api/kb/{kb_id}/documents/{id}", get(controllers::documents::get).delete(controllers::documents::delete))
        .route("/api/kb/{kb_id}/documents/{id}/move", put(controllers::folders::move_document))
        .route("/api/kb/{kb_id}/documents/{id}/reindex", post(controllers::documents::reindex))
        .route("/api/kb/{kb_id}/documents/{id}/content", get(controllers::documents::content))
        .route("/api/kb/{kb_id}/documents/{id}/pages", get(controllers::documents::pages))
        // RAG
        .route("/api/kb/{kb_id}/query", post(controllers::query::query))
        .route("/api/kb/{kb_id}/retrieve", post(controllers::retrieve::retrieve))
        // Chat sessions (polling — no SSE through the dashboard proxy)
        .route("/api/kb/{kb_id}/sessions", get(controllers::chat_sessions::list_sessions).post(controllers::chat_sessions::create_session))
        .route("/api/kb/{kb_id}/sessions/{sid}", get(controllers::chat_sessions::get_session).delete(controllers::chat_sessions::delete_session))
        .route("/api/kb/{kb_id}/sessions/{sid}/messages", post(controllers::chat_sessions::send_message))
        // Wiki
        .route("/api/kb/{kb_id}/wiki", get(controllers::wiki::list_pages))
        .route("/api/kb/{kb_id}/wiki/{slug}", get(controllers::wiki::get_page))
        .layer(middleware::from_fn(agent_protocol::require_agent_secret));

    // Public routes (health + manifest for discovery)
    let mut app = Router::new()
        .route("/.well-known/agent.json", get(manifest))
        .route("/health", get(health))
        .merge(api_routes)
        .layer(Extension(AgentSecret(config.agent_secret.clone())))
        .layer(cors)
        .with_state(state.clone());

    // Serve built frontend UI
    let static_dir = std::path::Path::new(&config.static_dir);
    if static_dir.exists() {
        info!("Serving knowledgebase UI from {}", config.static_dir);
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
    info!("Knowledgebase agent listening on {}", config.listen_addr);
    axum::serve(listener, app).await.expect("Server failed");
}

async fn manifest() -> Json<AgentManifest> {
    Json(AgentManifest {
        id: "knowledgebase".to_string(),
        name: "Knowledgebase".to_string(),
        version: "0.2.0".to_string(),
        description: "RAG knowledgebase — upload documents and chat with an agent over them".to_string(),
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
        // Surfaced in the dashboard as "missing secrets" until configured. The
        // agent runs without them, but document upload/indexing + RAG chat stay
        // disabled until they're set (here or via Platform Secrets).
        required_secrets: vec![
            "OPENAI_API_KEY".to_string(),
            "S3_ACCESS_KEY".to_string(),
            "S3_SECRET_KEY".to_string(),
            "S3_BUCKET".to_string(),
        ],
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
