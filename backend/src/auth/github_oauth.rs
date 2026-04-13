use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GitHubTokenResponse {
    pub access_token: String,
}

#[derive(Debug, Deserialize)]
pub struct GitHubUser {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubEmail {
    pub email: String,
    pub primary: bool,
    pub verified: bool,
}

pub async fn exchange_code(
    client_id: &str,
    client_secret: &str,
    code: &str,
) -> Result<String, reqwest::Error> {
    let client = Client::new();
    let resp: GitHubTokenResponse = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .json(&serde_json::json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "code": code,
        }))
        .send()
        .await?
        .json()
        .await?;
    Ok(resp.access_token)
}

pub async fn fetch_user(access_token: &str) -> Result<GitHubUser, reqwest::Error> {
    let client = Client::new();
    client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("User-Agent", "octa-dashboard")
        .send()
        .await?
        .json()
        .await
}

pub async fn fetch_primary_email(access_token: &str) -> Result<Option<String>, reqwest::Error> {
    let client = Client::new();
    let emails: Vec<GitHubEmail> = client
        .get("https://api.github.com/user/emails")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("User-Agent", "octa-dashboard")
        .send()
        .await?
        .json()
        .await?;

    Ok(emails
        .into_iter()
        .find(|e| e.primary && e.verified)
        .map(|e| e.email))
}
