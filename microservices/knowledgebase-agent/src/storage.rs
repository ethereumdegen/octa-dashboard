use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: Uuid,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub parent_id: Option<Uuid>,
    pub is_folder: bool,
    pub sort_order: i32,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDocument {
    pub title: String,
    pub slug: String,
    pub content: Option<String>,
    pub parent_id: Option<Uuid>,
    pub is_folder: Option<bool>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDocument {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub content: Option<String>,
    pub parent_id: Option<Uuid>,
    pub is_folder: Option<bool>,
    pub sort_order: Option<i32>,
}

const SELECT_COLS: &str = "id, title, slug, content, parent_id, is_folder, sort_order, created_by, updated_by, created_at, updated_at";

pub async fn list_documents(pool: &Pool) -> Result<Vec<Document>, Box<dyn std::error::Error>> {
    let client = pool.get().await?;
    let rows = client
        .query(
            &format!("SELECT {SELECT_COLS} FROM kb_documents ORDER BY sort_order, title"),
            &[],
        )
        .await?;

    Ok(rows.iter().map(row_to_doc).collect())
}

pub async fn get_document(pool: &Pool, id: Uuid) -> Result<Option<Document>, Box<dyn std::error::Error>> {
    let client = pool.get().await?;
    let row = client
        .query_opt(
            &format!("SELECT {SELECT_COLS} FROM kb_documents WHERE id = $1"),
            &[&id],
        )
        .await?;

    Ok(row.as_ref().map(row_to_doc))
}

pub async fn create_document(pool: &Pool, doc: CreateDocument) -> Result<Document, Box<dyn std::error::Error>> {
    let client = pool.get().await?;
    let content = doc.content.unwrap_or_default();
    let sort_order = doc.sort_order.unwrap_or(0);
    let is_folder = doc.is_folder.unwrap_or(false);
    let row = client
        .query_one(
            &format!(
                "INSERT INTO kb_documents (title, slug, content, parent_id, is_folder, sort_order)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 RETURNING {SELECT_COLS}"
            ),
            &[&doc.title, &doc.slug, &content, &doc.parent_id, &is_folder, &sort_order],
        )
        .await?;

    Ok(row_to_doc(&row))
}

pub async fn update_document(pool: &Pool, id: Uuid, doc: UpdateDocument) -> Result<Option<Document>, Box<dyn std::error::Error>> {
    let client = pool.get().await?;

    let existing = client
        .query_opt("SELECT id FROM kb_documents WHERE id = $1", &[&id])
        .await?;
    if existing.is_none() {
        return Ok(None);
    }

    if let Some(title) = &doc.title {
        client.execute("UPDATE kb_documents SET title = $1, updated_at = now() WHERE id = $2", &[title, &id]).await?;
    }
    if let Some(slug) = &doc.slug {
        client.execute("UPDATE kb_documents SET slug = $1, updated_at = now() WHERE id = $2", &[slug, &id]).await?;
    }
    if let Some(content) = &doc.content {
        client.execute("UPDATE kb_documents SET content = $1, updated_at = now() WHERE id = $2", &[content, &id]).await?;
    }
    if let Some(parent_id) = &doc.parent_id {
        client.execute("UPDATE kb_documents SET parent_id = $1, updated_at = now() WHERE id = $2", &[parent_id, &id]).await?;
    }
    if let Some(is_folder) = &doc.is_folder {
        client.execute("UPDATE kb_documents SET is_folder = $1, updated_at = now() WHERE id = $2", &[is_folder, &id]).await?;
    }
    if let Some(sort_order) = &doc.sort_order {
        client.execute("UPDATE kb_documents SET sort_order = $1, updated_at = now() WHERE id = $2", &[sort_order, &id]).await?;
    }

    get_document(pool, id).await
}

pub async fn delete_document(pool: &Pool, id: Uuid) -> Result<bool, Box<dyn std::error::Error>> {
    let client = pool.get().await?;
    let deleted = client
        .execute("DELETE FROM kb_documents WHERE id = $1", &[&id])
        .await?;
    Ok(deleted > 0)
}

pub async fn search_documents(pool: &Pool, query: &str) -> Result<Vec<Document>, Box<dyn std::error::Error>> {
    let client = pool.get().await?;
    let rows = client
        .query(
            &format!(
                "SELECT {SELECT_COLS}
                 FROM kb_documents
                 WHERE to_tsvector('english', title || ' ' || content) @@ plainto_tsquery('english', $1)
                 ORDER BY ts_rank(to_tsvector('english', title || ' ' || content), plainto_tsquery('english', $1)) DESC"
            ),
            &[&query],
        )
        .await?;

    Ok(rows.iter().map(row_to_doc).collect())
}

fn row_to_doc(r: &tokio_postgres::Row) -> Document {
    Document {
        id: r.get("id"),
        title: r.get("title"),
        slug: r.get("slug"),
        content: r.get("content"),
        parent_id: r.get("parent_id"),
        is_folder: r.get("is_folder"),
        sort_order: r.get("sort_order"),
        created_by: r.get("created_by"),
        updated_by: r.get("updated_by"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }
}
