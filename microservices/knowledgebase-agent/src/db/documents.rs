use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::document::{DocStatus, Document};

pub async fn insert(
    pool: &PgPool,
    kb_id: Uuid,
    filename: &str,
    mime_type: &str,
    s3_key: &str,
    size_bytes: i64,
    uploaded_by: Option<Uuid>,
    folder_id: Option<Uuid>,
) -> AppResult<Document> {
    let doc = sqlx::query_as::<_, Document>(
        r#"
        INSERT INTO documents (kb_id, filename, mime_type, s3_key, size_bytes, uploaded_by, folder_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *, 0::bigint AS pages_indexed
        "#,
    )
    .bind(kb_id)
    .bind(filename)
    .bind(mime_type)
    .bind(s3_key)
    .bind(size_bytes)
    .bind(uploaded_by)
    .bind(folder_id)
    .fetch_one(pool)
    .await?;
    Ok(doc)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<Document>> {
    let doc = sqlx::query_as::<_, Document>(
        r#"SELECT d.*, (SELECT COUNT(*) FROM page_indexes pi WHERE pi.document_id = d.id) AS pages_indexed
           FROM documents d WHERE d.id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(doc)
}

pub async fn list_for_kb(pool: &PgPool, kb_id: Uuid) -> AppResult<Vec<Document>> {
    let docs = sqlx::query_as::<_, Document>(
        r#"SELECT d.*, (SELECT COUNT(*) FROM page_indexes pi WHERE pi.document_id = d.id) AS pages_indexed
           FROM documents d WHERE d.kb_id = $1 ORDER BY d.created_at DESC"#,
    )
    .bind(kb_id)
    .fetch_all(pool)
    .await?;
    Ok(docs)
}

pub async fn update_status(
    pool: &PgPool,
    id: Uuid,
    status: DocStatus,
    error_msg: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE documents SET status = $2, error_msg = $3, updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .bind(error_msg)
    .execute(pool)
    .await?;
    Ok(())
}

/// Only update status if the document is still in 'processing'.
/// Prevents a completed indexer run from overwriting a reindex reset.
pub async fn update_status_if_processing(
    pool: &PgPool,
    id: Uuid,
    status: DocStatus,
    error_msg: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE documents SET status = $2, error_msg = $3, updated_at = now() WHERE id = $1 AND status = 'processing'",
    )
    .bind(id)
    .bind(status)
    .bind(error_msg)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_page_count(pool: &PgPool, id: Uuid, page_count: i32) -> AppResult<()> {
    sqlx::query("UPDATE documents SET page_count = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(page_count)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<bool> {
    let result = sqlx::query("DELETE FROM documents WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Find pending documents globally (all KBs) for the background indexer.
pub async fn find_pending_global(pool: &PgPool) -> AppResult<Vec<Document>> {
    let docs = sqlx::query_as::<_, Document>(
        r#"SELECT d.*, (SELECT COUNT(*) FROM page_indexes pi WHERE pi.document_id = d.id) AS pages_indexed
           FROM documents d WHERE d.status = 'uploaded' ORDER BY d.created_at ASC LIMIT 5"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(docs)
}

/// Reset documents stuck in "processing" for over 10 minutes back to "uploaded".
/// Also clears partial indexes so the retry starts fresh.
pub async fn reset_stuck_processing(pool: &PgPool) -> AppResult<u64> {
    // Clear partial indexes first to avoid unique constraint violations on retry
    sqlx::query(
        "DELETE FROM document_indexes WHERE document_id IN (SELECT id FROM documents WHERE status = 'processing' AND updated_at < now() - interval '10 minutes')",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "DELETE FROM page_indexes WHERE document_id IN (SELECT id FROM documents WHERE status = 'processing' AND updated_at < now() - interval '10 minutes')",
    )
    .execute(pool)
    .await?;

    let result = sqlx::query(
        "UPDATE documents SET status = 'uploaded', page_count = NULL, error_msg = NULL, updated_at = now() WHERE status = 'processing' AND updated_at < now() - interval '10 minutes'",
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Reset a document to "uploaded" and clear its indexes so the indexer re-processes it.
/// Also clears wiki pages generated from this document to prevent stale orphans.
pub async fn reset_for_reindex(pool: &PgPool, id: Uuid) -> AppResult<()> {
    sqlx::query("DELETE FROM document_indexes WHERE document_id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM page_indexes WHERE document_id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wiki_pages WHERE document_id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query(
        "UPDATE documents SET status = 'uploaded', page_count = NULL, error_msg = NULL, updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn count_for_kb(pool: &PgPool, kb_id: Uuid) -> AppResult<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM documents WHERE kb_id = $1",
    )
    .bind(kb_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn list_for_folder(
    pool: &PgPool,
    kb_id: Uuid,
    folder_id: Option<Uuid>,
) -> AppResult<Vec<Document>> {
    let docs = sqlx::query_as::<_, Document>(
        r#"SELECT d.*, (SELECT COUNT(*) FROM page_indexes pi WHERE pi.document_id = d.id) AS pages_indexed
           FROM documents d
           WHERE d.kb_id = $1 AND d.folder_id IS NOT DISTINCT FROM $2
           ORDER BY d.created_at DESC"#,
    )
    .bind(kb_id)
    .bind(folder_id)
    .fetch_all(pool)
    .await?;
    Ok(docs)
}

pub async fn move_to_folder(
    pool: &PgPool,
    id: Uuid,
    folder_id: Option<Uuid>,
) -> AppResult<()> {
    sqlx::query("UPDATE documents SET folder_id = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(folder_id)
        .execute(pool)
        .await?;
    Ok(())
}
