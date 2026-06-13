use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::wiki_page::WikiPage;

pub async fn upsert(
    pool: &PgPool,
    kb_id: Uuid,
    document_id: Option<Uuid>,
    slug: &str,
    title: &str,
    summary: Option<&str>,
    content_s3_key: &str,
    page_type: &str,
    sources: &serde_json::Value,
) -> AppResult<WikiPage> {
    let page = sqlx::query_as::<_, WikiPage>(
        r#"
        INSERT INTO wiki_pages (kb_id, document_id, slug, title, summary, content_s3_key, page_type, sources)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (kb_id, slug) DO UPDATE
        SET document_id = EXCLUDED.document_id,
            title = EXCLUDED.title,
            summary = EXCLUDED.summary,
            content_s3_key = EXCLUDED.content_s3_key,
            page_type = EXCLUDED.page_type,
            sources = EXCLUDED.sources,
            updated_at = now()
        RETURNING *
        "#,
    )
    .bind(kb_id)
    .bind(document_id)
    .bind(slug)
    .bind(title)
    .bind(summary)
    .bind(content_s3_key)
    .bind(page_type)
    .bind(sources)
    .fetch_one(pool)
    .await?;
    Ok(page)
}

pub async fn list_for_kb(pool: &PgPool, kb_id: Uuid) -> AppResult<Vec<WikiPage>> {
    let pages = sqlx::query_as::<_, WikiPage>(
        "SELECT * FROM wiki_pages WHERE kb_id = $1 ORDER BY title",
    )
    .bind(kb_id)
    .fetch_all(pool)
    .await?;
    Ok(pages)
}

pub async fn get_by_slug(pool: &PgPool, kb_id: Uuid, slug: &str) -> AppResult<Option<WikiPage>> {
    let page = sqlx::query_as::<_, WikiPage>(
        "SELECT * FROM wiki_pages WHERE kb_id = $1 AND slug = $2",
    )
    .bind(kb_id)
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    Ok(page)
}

pub async fn delete_for_document(pool: &PgPool, document_id: Uuid) -> AppResult<u64> {
    let result = sqlx::query("DELETE FROM wiki_pages WHERE document_id = $1")
        .bind(document_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
