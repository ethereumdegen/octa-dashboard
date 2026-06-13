use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::chat_session::{ChatMessage, ChatRole, ChatSession};

pub async fn create_session(
    pool: &PgPool,
    kb_id: Uuid,
    user_id: Option<Uuid>,
    title: &str,
) -> AppResult<ChatSession> {
    let session = sqlx::query_as::<_, ChatSession>(
        r#"
        INSERT INTO chat_sessions (kb_id, user_id, title)
        VALUES ($1, $2, $3)
        RETURNING *
        "#,
    )
    .bind(kb_id)
    .bind(user_id)
    .bind(title)
    .fetch_one(pool)
    .await?;
    Ok(session)
}

/// Global scope: all sessions for a KB are listed (not filtered by user).
pub async fn list_sessions(pool: &PgPool, kb_id: Uuid) -> AppResult<Vec<ChatSession>> {
    let sessions = sqlx::query_as::<_, ChatSession>(
        "SELECT * FROM chat_sessions WHERE kb_id = $1 ORDER BY updated_at DESC",
    )
    .bind(kb_id)
    .fetch_all(pool)
    .await?;
    Ok(sessions)
}

pub async fn get_session(pool: &PgPool, id: Uuid) -> AppResult<Option<ChatSession>> {
    let session = sqlx::query_as::<_, ChatSession>("SELECT * FROM chat_sessions WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(session)
}

/// Deletes a session; chat_messages and chat_jobs cascade via FK ON DELETE CASCADE.
pub async fn delete_session(pool: &PgPool, id: Uuid) -> AppResult<()> {
    sqlx::query("DELETE FROM chat_sessions WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_title(pool: &PgPool, id: Uuid, title: &str) -> AppResult<()> {
    sqlx::query("UPDATE chat_sessions SET title = $1, updated_at = now() WHERE id = $2")
        .bind(title)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn add_message(
    pool: &PgPool,
    session_id: Uuid,
    role: ChatRole,
    content: &str,
    metadata: Option<&serde_json::Value>,
) -> AppResult<ChatMessage> {
    sqlx::query("UPDATE chat_sessions SET updated_at = now() WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await?;

    let msg = sqlx::query_as::<_, ChatMessage>(
        r#"
        INSERT INTO chat_messages (session_id, role, content, metadata)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
    )
    .bind(session_id)
    .bind(role)
    .bind(content)
    .bind(metadata)
    .fetch_one(pool)
    .await?;
    Ok(msg)
}

pub async fn get_messages(pool: &PgPool, session_id: Uuid) -> AppResult<Vec<ChatMessage>> {
    let messages = sqlx::query_as::<_, ChatMessage>(
        "SELECT * FROM chat_messages WHERE session_id = $1 ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(messages)
}

/// Most recent N messages (oldest-first) for injecting conversation history.
pub async fn get_recent_messages(
    pool: &PgPool,
    session_id: Uuid,
    limit: i64,
) -> AppResult<Vec<ChatMessage>> {
    let messages = sqlx::query_as::<_, ChatMessage>(
        r#"
        SELECT * FROM (
            SELECT * FROM chat_messages
            WHERE session_id = $1
            ORDER BY created_at DESC
            LIMIT $2
        ) sub ORDER BY created_at ASC
        "#,
    )
    .bind(session_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(messages)
}
