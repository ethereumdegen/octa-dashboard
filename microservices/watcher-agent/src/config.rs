use std::env;
use tracing::{info, warn};

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub listen_addr: String,
    pub static_dir: String,
    pub agent_secret: String,
    pub dashboard_url: String,
    pub github_token: String,
    pub openai_api_key: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL required"),
            listen_addr: env::var("WATCHER_LISTEN_ADDR").unwrap_or_else(|_| {
                let port = env::var("PORT").unwrap_or_else(|_| "4004".to_string());
                format!("0.0.0.0:{port}")
            }),
            static_dir: env::var("WATCHER_STATIC_DIR").unwrap_or_else(|_| "static".to_string()),
            agent_secret: env::var("AGENT_SECRET").unwrap_or_default(),
            dashboard_url: env::var("DASHBOARD_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            github_token: env::var("GITHUB_TOKEN").unwrap_or_default(),
            openai_api_key: env::var("OPENAI_API_KEY").unwrap_or_default(),
        }
    }

    /// Resolve the GitHub PAT: prefer the dashboard's platform secret, fall back to env.
    pub async fn resolve_github_token(&self) -> Option<String> {
        self.resolve_secret("GITHUB_TOKEN", &self.github_token).await
    }

    /// Resolve the OpenAI API key (shared with the knowledgebase agent).
    pub async fn resolve_openai_key(&self) -> Option<String> {
        self.resolve_secret("OPENAI_API_KEY", &self.openai_api_key).await
    }

    async fn resolve_secret(&self, key: &str, fallback: &str) -> Option<String> {
        if !self.dashboard_url.is_empty() {
            match fetch_platform_secret(&self.dashboard_url, &self.agent_secret, key).await {
                Ok(value) if !value.is_empty() => {
                    info!("Resolved {key} from dashboard platform secrets");
                    return Some(value);
                }
                Ok(_) => {}
                Err(e) => warn!("Could not fetch {key} from dashboard: {e}"),
            }
        }
        if !fallback.is_empty() {
            return Some(fallback.to_string());
        }
        None
    }
}

async fn fetch_platform_secret(
    dashboard_url: &str,
    api_key: &str,
    secret_key: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "{}/internal/platform-secrets/{}",
        dashboard_url.trim_end_matches('/'),
        secret_key
    );
    let mut req = reqwest::Client::new().get(&url);
    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {api_key}"));
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()).into());
    }
    let body: serde_json::Value = resp.json().await?;
    body.get("value")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "missing value field".into())
}
