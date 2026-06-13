use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::document::{DocumentIndex, PageIndex};

pub async fn insert_page(
    pool: &PgPool,
    document_id: Uuid,
    page_num: i32,
    content: &str,
    tree_index: &serde_json::Value,
) -> AppResult<PageIndex> {
    let page = sqlx::query_as::<_, PageIndex>(
        r#"
        INSERT INTO page_indexes (document_id, page_num, content, tree_index)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (document_id, page_num) DO UPDATE
        SET content = EXCLUDED.content, tree_index = EXCLUDED.tree_index
        RETURNING *
        "#,
    )
    .bind(document_id)
    .bind(page_num)
    .bind(content)
    .bind(tree_index)
    .fetch_one(pool)
    .await?;
    Ok(page)
}

pub async fn insert_document_index(
    pool: &PgPool,
    document_id: Uuid,
    root_index: &serde_json::Value,
) -> AppResult<DocumentIndex> {
    let idx = sqlx::query_as::<_, DocumentIndex>(
        r#"
        INSERT INTO document_indexes (document_id, root_index)
        VALUES ($1, $2)
        ON CONFLICT (document_id) DO UPDATE
        SET root_index = EXCLUDED.root_index
        RETURNING *
        "#,
    )
    .bind(document_id)
    .bind(root_index)
    .fetch_one(pool)
    .await?;
    Ok(idx)
}

pub async fn get_pages_for_document(
    pool: &PgPool,
    document_id: Uuid,
) -> AppResult<Vec<PageIndex>> {
    let pages = sqlx::query_as::<_, PageIndex>(
        "SELECT * FROM page_indexes WHERE document_id = $1 ORDER BY page_num",
    )
    .bind(document_id)
    .fetch_all(pool)
    .await?;
    Ok(pages)
}

pub async fn get_page(
    pool: &PgPool,
    document_id: Uuid,
    page_num: i32,
) -> AppResult<Option<PageIndex>> {
    let page = sqlx::query_as::<_, PageIndex>(
        "SELECT * FROM page_indexes WHERE document_id = $1 AND page_num = $2",
    )
    .bind(document_id)
    .bind(page_num)
    .fetch_optional(pool)
    .await?;
    Ok(page)
}

pub async fn get_document_index(
    pool: &PgPool,
    document_id: Uuid,
) -> AppResult<Option<DocumentIndex>> {
    let idx = sqlx::query_as::<_, DocumentIndex>(
        "SELECT * FROM document_indexes WHERE document_id = $1",
    )
    .bind(document_id)
    .fetch_optional(pool)
    .await?;
    Ok(idx)
}

/// Get all document indexes for a specific KB (scoped by kb_id via join).
pub async fn get_document_indexes_for_kb(
    pool: &PgPool,
    kb_id: Uuid,
) -> AppResult<Vec<DocumentIndex>> {
    let indexes = sqlx::query_as::<_, DocumentIndex>(
        r#"
        SELECT di.* FROM document_indexes di
        JOIN documents d ON d.id = di.document_id
        WHERE d.kb_id = $1
        ORDER BY di.created_at DESC
        "#,
    )
    .bind(kb_id)
    .fetch_all(pool)
    .await?;
    Ok(indexes)
}

/// Get pages for a document, but only if that document belongs to the given KB.
/// Full-text search across page content, scoped to a KB.
pub async fn search_pages_fts(
    pool: &PgPool,
    kb_id: Uuid,
    query: &str,
    limit: i64,
) -> AppResult<Vec<FtsPageHit>> {
    let hits = sqlx::query_as::<_, FtsPageHit>(
        r#"
        SELECT pi.document_id, pi.page_num, d.filename,
               ts_rank(pi.content_tsv, websearch_to_tsquery('english', $1)) AS rank,
               ts_headline('english', pi.content, websearch_to_tsquery('english', $1),
                   'MaxWords=60, MinWords=20, StartSel=>>>, StopSel=<<<') AS snippet
        FROM page_indexes pi
        JOIN documents d ON d.id = pi.document_id
        WHERE d.kb_id = $2
          AND pi.content_tsv @@ websearch_to_tsquery('english', $1)
        ORDER BY rank DESC
        LIMIT $3
        "#,
    )
    .bind(query)
    .bind(kb_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(hits)
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct FtsPageHit {
    pub document_id: Uuid,
    pub page_num: i32,
    pub filename: String,
    pub rank: f32,
    pub snippet: String,
}

/// Get page-level tree indexes (no content) for a document, scoped to a KB.
pub async fn get_tree_indexes_for_doc(
    pool: &PgPool,
    kb_id: Uuid,
    document_id: Uuid,
) -> AppResult<Vec<PageTreeSummary>> {
    let rows = sqlx::query_as::<_, PageTreeSummary>(
        r#"
        SELECT pi.page_num, pi.tree_index
        FROM page_indexes pi
        JOIN documents d ON d.id = pi.document_id
        WHERE pi.document_id = $1 AND d.kb_id = $2
        ORDER BY pi.page_num
        "#,
    )
    .bind(document_id)
    .bind(kb_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct PageTreeSummary {
    pub page_num: i32,
    pub tree_index: serde_json::Value,
}

pub async fn get_page_scoped(
    pool: &PgPool,
    kb_id: Uuid,
    document_id: Uuid,
    page_num: i32,
) -> AppResult<Option<PageIndex>> {
    let page = sqlx::query_as::<_, PageIndex>(
        r#"
        SELECT pi.* FROM page_indexes pi
        JOIN documents d ON d.id = pi.document_id
        WHERE pi.document_id = $1 AND pi.page_num = $2 AND d.kb_id = $3
        "#,
    )
    .bind(document_id)
    .bind(page_num)
    .bind(kb_id)
    .fetch_optional(pool)
    .await?;
    Ok(page)
}
