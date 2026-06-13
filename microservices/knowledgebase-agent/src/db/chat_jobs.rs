use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::chat_job::ChatJob;

pub async fn create(
    pool: &PgPool,
    session_id: Uuid,
    kb_id: Uuid,
    owner_id: Option<Uuid>,
    content: &str,
) -> AppResult<Uuid> {
    let id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO chat_jobs (session_id, kb_id, owner_id, content)
        VALUES ($1, $2, $3, $4)
        RETURNING id
        "#,
    )
    .bind(session_id)
    .bind(kb_id)
    .bind(owner_id)
    .bind(content)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Atomically claim a ready job using FOR UPDATE SKIP LOCKED.
pub async fn find_and_claim(pool: &PgPool, worker_id: &str) -> AppResult<Option<ChatJob>> {
    let job = sqlx::query_as::<_, ChatJob>(
        r#"
        UPDATE chat_jobs SET status = 'in_progress', worker_id = $1,
            claimed_at = NOW(), updated_at = NOW()
        WHERE id = (
            SELECT id FROM chat_jobs WHERE status = 'ready'
            ORDER BY created_at
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        RETURNING *
        "#,
    )
    .bind(worker_id)
    .fetch_optional(pool)
    .await?;
    Ok(job)
}

pub async fn complete(pool: &PgPool, job_id: Uuid, worker_id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE chat_jobs SET status = 'completed', completed_at = NOW(), updated_at = NOW() \
         WHERE id = $1 AND worker_id = $2",
    )
    .bind(job_id)
    .bind(worker_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fail(pool: &PgPool, job_id: Uuid, worker_id: &str, error: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE chat_jobs SET status = 'failed', error = $1, completed_at = NOW(), updated_at = NOW() \
         WHERE id = $2 AND worker_id = $3",
    )
    .bind(error)
    .bind(job_id)
    .bind(worker_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fail stale in_progress jobs (>5 min without completion).
pub async fn fail_stale(pool: &PgPool) -> AppResult<u64> {
    let rows = sqlx::query(
        "UPDATE chat_jobs SET status = 'failed', error = 'Timed out', completed_at = NOW(), updated_at = NOW() \
         WHERE status = 'in_progress' AND updated_at < NOW() - INTERVAL '5 minutes'",
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(rows)
}

/// Get a single chat job by ID (admin).
pub async fn get(pool: &PgPool, id: Uuid) -> AppResult<Option<ChatJob>> {
    let job = sqlx::query_as::<_, ChatJob>(
        "SELECT * FROM chat_jobs WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(job)
}

/// List recent chat jobs (admin).
pub async fn list(pool: &PgPool, limit: i64, offset: i64) -> AppResult<Vec<ChatJob>> {
    let jobs = sqlx::query_as::<_, ChatJob>(
        "SELECT * FROM chat_jobs ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(jobs)
}

pub async fn count(pool: &PgPool) -> AppResult<i64> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM chat_jobs")
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

/// Fail stale ready jobs (>10 min without being picked up).
pub async fn fail_stale_ready(pool: &PgPool) -> AppResult<u64> {
    let rows = sqlx::query(
        "UPDATE chat_jobs SET status = 'failed', error = 'No worker available', completed_at = NOW(), updated_at = NOW() \
         WHERE status = 'ready' AND created_at < NOW() - INTERVAL '10 minutes'",
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(rows)
}
