use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

type DbResult<T> = Result<T, sqlx::Error>;

// ── Models ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Repo {
    pub id: Uuid,
    pub owner: String,
    pub repo: String,
    pub default_branch: String,
    pub enabled: bool,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Commit {
    pub id: Uuid,
    pub repo_id: Uuid,
    pub sha: String,
    pub author: Option<String>,
    pub author_email: Option<String>,
    pub message: Option<String>,
    pub url: Option<String>,
    pub committed_at: Option<DateTime<Utc>>,
    pub additions: Option<i32>,
    pub deletions: Option<i32>,
    pub files_changed: Option<serde_json::Value>,
    pub summary: Option<String>,
    pub summary_status: String,
    pub raw_data: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    // Joined from watcher_repos for index/show display.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRepo {
    pub owner: String,
    pub repo: String,
    pub default_branch: Option<String>,
}

/// A new commit ready to be inserted by the watch worker.
pub struct NewCommit {
    pub repo_id: Uuid,
    pub sha: String,
    pub author: Option<String>,
    pub author_email: Option<String>,
    pub message: Option<String>,
    pub url: Option<String>,
    pub committed_at: Option<DateTime<Utc>>,
    pub additions: Option<i32>,
    pub deletions: Option<i32>,
    pub files_changed: Option<serde_json::Value>,
    pub raw_data: Option<serde_json::Value>,
}

// ── Repo queries ────────────────────────────────────────────────────────

const REPO_COLS: &str =
    "id, owner, repo, default_branch, enabled, last_checked_at, last_error, created_at";

pub async fn list_repos(pool: &PgPool) -> DbResult<Vec<Repo>> {
    sqlx::query_as::<_, Repo>(&format!(
        "SELECT {REPO_COLS} FROM watcher_repos ORDER BY created_at DESC"
    ))
    .fetch_all(pool)
    .await
}

pub async fn list_enabled_repos(pool: &PgPool) -> DbResult<Vec<Repo>> {
    sqlx::query_as::<_, Repo>(&format!(
        "SELECT {REPO_COLS} FROM watcher_repos WHERE enabled = true"
    ))
    .fetch_all(pool)
    .await
}

pub async fn create_repo(pool: &PgPool, input: CreateRepo) -> DbResult<Repo> {
    let branch = input.default_branch.unwrap_or_else(|| "main".to_string());
    sqlx::query_as::<_, Repo>(&format!(
        "INSERT INTO watcher_repos (owner, repo, default_branch)
         VALUES ($1, $2, $3)
         ON CONFLICT (owner, repo) DO UPDATE SET enabled = true
         RETURNING {REPO_COLS}"
    ))
    .bind(input.owner)
    .bind(input.repo)
    .bind(branch)
    .fetch_one(pool)
    .await
}

pub async fn delete_repo(pool: &PgPool, id: Uuid) -> DbResult<bool> {
    let result = sqlx::query("DELETE FROM watcher_repos WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn mark_repo_checked(pool: &PgPool, id: Uuid, error: Option<&str>) -> DbResult<()> {
    sqlx::query(
        "UPDATE watcher_repos SET last_checked_at = now(), last_error = $1 WHERE id = $2",
    )
    .bind(error)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Commit queries ──────────────────────────────────────────────────────

const COMMIT_COLS: &str = "c.id, c.repo_id, c.sha, c.author, c.author_email, c.message, c.url, \
    c.committed_at, c.additions, c.deletions, c.files_changed, c.summary, c.summary_status, \
    c.raw_data, c.created_at, r.owner AS repo_owner, r.repo AS repo_name";

pub async fn list_commits(pool: &PgPool, limit: i64) -> DbResult<Vec<Commit>> {
    sqlx::query_as::<_, Commit>(&format!(
        "SELECT {COMMIT_COLS} FROM watcher_commits c
         JOIN watcher_repos r ON r.id = c.repo_id
         ORDER BY c.committed_at DESC NULLS LAST LIMIT $1"
    ))
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn get_commit(pool: &PgPool, id: Uuid) -> DbResult<Option<Commit>> {
    sqlx::query_as::<_, Commit>(&format!(
        "SELECT {COMMIT_COLS} FROM watcher_commits c
         JOIN watcher_repos r ON r.id = c.repo_id
         WHERE c.id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn commit_exists(pool: &PgPool, repo_id: Uuid, sha: &str) -> DbResult<bool> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM watcher_commits WHERE repo_id = $1 AND sha = $2)",
    )
    .bind(repo_id)
    .bind(sha)
    .fetch_one(pool)
    .await
}

pub async fn insert_commit(pool: &PgPool, c: &NewCommit) -> DbResult<()> {
    sqlx::query(
        "INSERT INTO watcher_commits
            (repo_id, sha, author, author_email, message, url, committed_at,
             additions, deletions, files_changed, raw_data)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         ON CONFLICT (repo_id, sha) DO NOTHING",
    )
    .bind(c.repo_id)
    .bind(&c.sha)
    .bind(&c.author)
    .bind(&c.author_email)
    .bind(&c.message)
    .bind(&c.url)
    .bind(c.committed_at)
    .bind(c.additions)
    .bind(c.deletions)
    .bind(&c.files_changed)
    .bind(&c.raw_data)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fetch commits awaiting an LLM summary (phase-2 worker input).
pub async fn list_pending_summaries(pool: &PgPool, limit: i64) -> DbResult<Vec<Commit>> {
    sqlx::query_as::<_, Commit>(&format!(
        "SELECT {COMMIT_COLS} FROM watcher_commits c
         JOIN watcher_repos r ON r.id = c.repo_id
         WHERE c.summary_status = 'pending'
         ORDER BY c.committed_at DESC NULLS LAST LIMIT $1"
    ))
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn set_commit_summary(
    pool: &PgPool,
    id: Uuid,
    summary: Option<&str>,
    status: &str,
) -> DbResult<()> {
    sqlx::query("UPDATE watcher_commits SET summary = $1, summary_status = $2 WHERE id = $3")
        .bind(summary)
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
