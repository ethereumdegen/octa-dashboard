use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub github_client_id: String,
    pub github_client_secret: String,
    pub github_redirect_uri: String,
    pub jwt_secret: String,
    pub initial_admin_email: String,
    pub agent_urls: Vec<String>,
    pub listen_addr: String,
    pub skip_login: bool,
    pub agent_secret: String,
}

impl Config {
    pub fn from_env() -> Self {
        let skip_login = env::var("SKIP_LOGIN")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let config = Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL required"),
            github_client_id: env::var("GITHUB_CLIENT_ID").unwrap_or_default(),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET").unwrap_or_default(),
            github_redirect_uri: env::var("GITHUB_REDIRECT_URI").unwrap_or_default(),
            jwt_secret: env::var("JWT_SECRET").expect("JWT_SECRET required"),
            initial_admin_email: env::var("INITIAL_ADMIN_EMAIL").unwrap_or_default(),
            agent_urls: env::var("AGENT_URLS")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.trim().to_string())
                .collect(),
            listen_addr: env::var("LISTEN_ADDR").unwrap_or_else(|_| {
                let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
                format!("0.0.0.0:{port}")
            }),
            skip_login,
            agent_secret: if skip_login {
                env::var("AGENT_SECRET").unwrap_or_default()
            } else {
                env::var("AGENT_SECRET").expect("AGENT_SECRET required (set SKIP_LOGIN=true for dev)")
            },
        };

        config
    }
}
