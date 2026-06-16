use std::time::Duration;

use serde_json::{json, Value};
use sqlx::PgPool;
use tracing::{info, warn};

use crate::config::Config;
use crate::github::GithubClient;
use crate::storage;

/// How far back to look for commits on each tick.
const LOOKBACK_HOURS: i64 = 24;
/// Watch loop interval (5 minutes).
const WATCH_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// Summary loop interval (30 seconds).
const SUMMARY_INTERVAL: Duration = Duration::from_secs(30);
/// Max commits to summarize per summary tick.
const SUMMARY_BATCH: i64 = 10;

// ── Watch loop ──────────────────────────────────────────────────────────

/// Polls GitHub every 5 minutes for commits in the last 24h and stores new ones.
pub async fn start_watch_loop(pool: PgPool, http: reqwest::Client, config: Config) {
    info!("[Watcher] Watch loop started (every {}s)", WATCH_INTERVAL.as_secs());
    loop {
        if let Err(e) = watch_tick(&pool, &http, &config).await {
            warn!("[Watcher] watch tick error: {e}");
        }
        tokio::time::sleep(WATCH_INTERVAL).await;
    }
}

async fn watch_tick(
    pool: &PgPool,
    http: &reqwest::Client,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token = match config.resolve_github_token().await {
        Some(t) => t,
        None => {
            warn!("[Watcher] GITHUB_TOKEN not configured — skipping watch tick");
            return Ok(());
        }
    };

    let repos = storage::list_enabled_repos(pool).await?;
    if repos.is_empty() {
        return Ok(());
    }

    let gh = GithubClient::new(http, &token);
    let since = (chrono::Utc::now() - chrono::Duration::hours(LOOKBACK_HOURS)).to_rfc3339();

    for repo in repos {
        match watch_repo(pool, &gh, &repo, &since).await {
            Ok(count) => {
                if count > 0 {
                    info!("[Watcher] {}/{}: {count} new commit(s)", repo.owner, repo.repo);
                }
                let _ = storage::mark_repo_checked(pool, repo.id, None).await;
            }
            Err(e) => {
                warn!("[Watcher] {}/{}: {e}", repo.owner, repo.repo);
                let _ = storage::mark_repo_checked(pool, repo.id, Some(&e.to_string())).await;
            }
        }
    }
    Ok(())
}

async fn watch_repo(
    pool: &PgPool,
    gh: &GithubClient<'_>,
    repo: &storage::Repo,
    since: &str,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let shas = gh
        .list_recent_shas(&repo.owner, &repo.repo, &repo.default_branch, since)
        .await?;

    let mut new_count = 0;
    for sha in shas {
        if storage::commit_exists(pool, repo.id, &sha).await? {
            continue;
        }
        let commit = gh
            .fetch_commit(repo.id, &repo.owner, &repo.repo, &sha)
            .await?;
        storage::insert_commit(pool, &commit).await?;
        new_count += 1;
    }
    Ok(new_count)
}

/// Run a single watch tick on demand (used by the manual trigger endpoint).
pub async fn run_once(
    pool: &PgPool,
    http: &reqwest::Client,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    watch_tick(pool, http, config).await
}

// ── Summary loop (phase 2) ────────────────────────────────────────────────

/// Generates an LLM summary for each unsummarized commit, every 30 seconds.
pub async fn start_summary_loop(pool: PgPool, http: reqwest::Client, config: Config) {
    info!("[Watcher] Summary loop started (every {}s)", SUMMARY_INTERVAL.as_secs());
    loop {
        if let Err(e) = summary_tick(&pool, &http, &config).await {
            warn!("[Watcher] summary tick error: {e}");
        }
        tokio::time::sleep(SUMMARY_INTERVAL).await;
    }
}

async fn summary_tick(
    pool: &PgPool,
    http: &reqwest::Client,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pending = storage::list_pending_summaries(pool, SUMMARY_BATCH).await?;
    if pending.is_empty() {
        return Ok(());
    }

    let key = match config.resolve_openai_key().await {
        Some(k) => k,
        None => {
            // No key yet — leave commits pending so they're summarized once one is set.
            return Ok(());
        }
    };

    for commit in pending {
        let message = commit.message.as_deref().unwrap_or("");
        match generate_summary(http, &key, message, &commit.files_changed).await {
            Ok(text) => {
                storage::set_commit_summary(pool, commit.id, Some(&text), "done").await?;
            }
            Err(e) => {
                warn!("[Watcher] summary failed for {}: {e}", commit.sha);
                storage::set_commit_summary(pool, commit.id, None, "error").await?;
            }
        }
    }
    Ok(())
}

async fn generate_summary(
    http: &reqwest::Client,
    openai_key: &str,
    message: &str,
    files_changed: &Option<Value>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let files_desc = files_changed
        .as_ref()
        .map(|f| serde_json::to_string_pretty(f).unwrap_or_default())
        .unwrap_or_else(|| "No file data".into());

    let prompt = format!(
        "Summarize this git commit in 2-3 sentences for a project status report. \
         Focus on what changed and why it matters.\n\n\
         Commit message: {message}\n\nFiles changed:\n{files_desc}"
    );

    let body = json!({
        "model": "gpt-4o-mini",
        "messages": [
            { "role": "system", "content": "You are a concise code reviewer. Summarize commits for non-technical stakeholders." },
            { "role": "user", "content": prompt },
        ],
        "max_tokens": 200,
    });

    let resp = http
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {openai_key}"))
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(format!("OpenAI {}", resp.status()).into());
    }

    let data: Value = resp.json().await?;
    data["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "OpenAI returned no content".into())
}
