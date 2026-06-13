use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WikiPage {
    pub id: Uuid,
    pub kb_id: Uuid,
    pub document_id: Option<Uuid>,
    pub slug: String,
    pub title: String,
    pub summary: Option<String>,
    pub content_s3_key: String,
    pub page_type: String,
    pub sources: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
