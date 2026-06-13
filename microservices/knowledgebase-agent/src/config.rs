use std::env;
use tracing::{info, warn};

#[derive(Clone, Debug)]
pub struct S3Config {
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub endpoint: Option<String>,
}

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub listen_addr: String,
    pub static_dir: String,
    pub agent_secret: String,
    pub dashboard_url: String,
    pub openai_api_key: String,
    pub openai_model: String,
    pub s3_region: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_bucket: String,
    pub s3_endpoint: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL required"),
            listen_addr: env::var("KB_LISTEN_ADDR").unwrap_or_else(|_| {
                let port = env::var("PORT").unwrap_or_else(|_| "4001".to_string());
                format!("0.0.0.0:{port}")
            }),
            static_dir: env::var("KB_STATIC_DIR").unwrap_or_else(|_| "static".to_string()),
            agent_secret: env::var("AGENT_SECRET").unwrap_or_default(),
            dashboard_url: env::var("DASHBOARD_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            openai_api_key: env::var("OPENAI_API_KEY").unwrap_or_default(),
            openai_model: env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5.4".into()),
            s3_region: env::var("S3_REGION").unwrap_or_else(|_| "nyc3".into()),
            s3_access_key: env::var("S3_ACCESS_KEY").unwrap_or_default(),
            s3_secret_key: env::var("S3_SECRET_KEY").unwrap_or_default(),
            s3_bucket: env::var("S3_BUCKET").unwrap_or_else(|_| "knowledgebase-docs".into()),
            s3_endpoint: env::var("S3_ENDPOINT").unwrap_or_default(),
        }
    }

    /// Fill any secret left empty by the environment from the dashboard's
    /// platform-secrets store (same pattern other agents use).
    pub async fn resolve_secrets(&mut self) {
        let keys: [(&str, &mut String); 6] = [
            ("OPENAI_API_KEY", &mut self.openai_api_key),
            ("S3_REGION", &mut self.s3_region),
            ("S3_ACCESS_KEY", &mut self.s3_access_key),
            ("S3_SECRET_KEY", &mut self.s3_secret_key),
            ("S3_BUCKET", &mut self.s3_bucket),
            ("S3_ENDPOINT", &mut self.s3_endpoint),
        ];
        if self.dashboard_url.is_empty() {
            return;
        }
        for (key, slot) in keys {
            if !slot.is_empty() {
                continue;
            }
            match fetch_platform_secret(&self.dashboard_url, &self.agent_secret, key).await {
                Ok(value) if !value.is_empty() => {
                    info!("Resolved {key} from dashboard platform secrets");
                    *slot = value;
                }
                Ok(_) => {}
                Err(e) => warn!("Could not fetch {key} from dashboard: {e}"),
            }
        }
    }

    /// S3 config if credentials are present, else None (upload/indexing disabled).
    pub fn s3_config(&self) -> Option<S3Config> {
        if self.s3_access_key.is_empty() || self.s3_secret_key.is_empty() {
            warn!("S3 not configured — document upload/indexing disabled");
            return None;
        }
        Some(S3Config {
            region: self.s3_region.clone(),
            access_key: self.s3_access_key.clone(),
            secret_key: self.s3_secret_key.clone(),
            bucket: self.s3_bucket.clone(),
            endpoint: Some(self.s3_endpoint.clone()).filter(|s| !s.is_empty()),
        })
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
