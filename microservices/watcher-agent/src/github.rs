use serde_json::Value;

use crate::storage::NewCommit;

type GhResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

const USER_AGENT: &str = "octa-watcher-agent";

/// A lightweight GitHub REST client backed by a personal access token.
pub struct GithubClient<'a> {
    http: &'a reqwest::Client,
    token: &'a str,
}

impl<'a> GithubClient<'a> {
    pub fn new(http: &'a reqwest::Client, token: &'a str) -> Self {
        Self { http, token }
    }

    async fn get(&self, url: &str) -> GhResult<Value> {
        let resp = self
            .http
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let reason = match status {
                reqwest::StatusCode::UNAUTHORIZED => "token invalid or revoked",
                reqwest::StatusCode::FORBIDDEN => "rate limited or forbidden",
                reqwest::StatusCode::NOT_FOUND => "repo not found or no access",
                _ => "unexpected error",
            };
            return Err(format!("GitHub {status}: {reason}").into());
        }
        Ok(resp.json().await?)
    }

    /// Resolve a repo's default branch (used when linking a repo).
    pub async fn default_branch(&self, owner: &str, repo: &str) -> GhResult<String> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}");
        let data = self.get(&url).await?;
        Ok(data["default_branch"]
            .as_str()
            .unwrap_or("main")
            .to_string())
    }

    /// List commit SHAs on `branch` since the given RFC3339 timestamp.
    pub async fn list_recent_shas(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
        since_rfc3339: &str,
    ) -> GhResult<Vec<String>> {
        let url = format!(
            "https://api.github.com/repos/{owner}/{repo}/commits?sha={branch}&since={since_rfc3339}&per_page=100"
        );
        let commits = self.get(&url).await?;
        let shas = commits
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| c["sha"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        Ok(shas)
    }

    /// Fetch a single commit's full detail and map it into a `NewCommit`.
    pub async fn fetch_commit(
        &self,
        repo_id: uuid::Uuid,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> GhResult<NewCommit> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/commits/{sha}");
        let data = self.get(&url).await?;

        let author = data["commit"]["author"]["name"]
            .as_str()
            .map(|s| s.to_string());
        let author_email = data["commit"]["author"]["email"]
            .as_str()
            .map(|s| s.to_string());
        let message = data["commit"]["message"].as_str().map(|s| s.to_string());
        let html_url = data["html_url"].as_str().map(|s| s.to_string());
        let committed_at = data["commit"]["author"]["date"]
            .as_str()
            .and_then(|d| d.parse::<chrono::DateTime<chrono::Utc>>().ok());

        let additions = data["stats"]["additions"].as_i64().map(|n| n as i32);
        let deletions = data["stats"]["deletions"].as_i64().map(|n| n as i32);

        let files_changed = data["files"].as_array().map(|files| {
            serde_json::json!(
                files
                    .iter()
                    .map(|f| serde_json::json!({
                        "filename": f["filename"],
                        "status": f["status"],
                        "additions": f["additions"],
                        "deletions": f["deletions"],
                    }))
                    .collect::<Vec<_>>()
            )
        });

        Ok(NewCommit {
            repo_id,
            sha: sha.to_string(),
            author,
            author_email,
            message,
            url: html_url,
            committed_at,
            additions,
            deletions,
            files_changed,
            raw_data: Some(data),
        })
    }
}
