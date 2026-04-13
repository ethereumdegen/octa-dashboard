use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use serde::{Deserialize, Serialize};

/// Standard Agent Protocol (SAP) manifest returned by `GET /.well-known/agent.json`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<AgentUiConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<AgentApiConfig>,
    pub health_endpoint: String,
    pub capabilities: Vec<String>,
    /// Secret keys this agent needs from the platform (e.g. ["OPENAI_API_KEY", "PINECONE_API_KEY"]).
    /// The dashboard will warn admins if any listed key is not configured.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_secrets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUiConfig {
    pub entry_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// JS bundle filename relative to entry_path (e.g. "poolsec.js")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_js: Option<String>,
    /// CSS bundle filename relative to entry_path (e.g. "poolsec.css")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_css: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentApiConfig {
    pub base_path: String,
}

/// Standard health response from `GET /health`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: u64,
}

// ── Agent Secret Auth ────────────────────────────────────────────────

/// Extension marker holding the expected agent secret.
#[derive(Clone)]
pub struct AgentSecret(pub String);

/// Axum middleware that validates `Authorization: Bearer <AGENT_SECRET>` on
/// every request. If `AGENT_SECRET` is empty/unset, all requests pass through
/// (dev mode). Health and manifest endpoints should be placed *outside* this
/// layer so Railway health checks still work.
pub async fn require_agent_secret(
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected = req
        .extensions()
        .get::<AgentSecret>()
        .map(|s| s.0.clone())
        .unwrap_or_default();

    // If no secret configured, allow all (dev mode) but warn
    if expected.is_empty() {
        tracing::warn!("AGENT_SECRET is not set — agent endpoints are unauthenticated");
        return Ok(next.run(req).await);
    }

    let provided = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");

    // Constant-time comparison to prevent timing attacks
    use sha2::{Sha256, Digest};
    let hash_provided = Sha256::digest(provided.as_bytes());
    let hash_expected = Sha256::digest(expected.as_bytes());
    if hash_provided == hash_expected {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
