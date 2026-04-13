use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub listen_addr: String,
    pub static_dir: String,
    pub agent_secret: String,
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
            agent_secret: env::var("AGENT_SECRET").expect("AGENT_SECRET required"),
        }
    }
}
