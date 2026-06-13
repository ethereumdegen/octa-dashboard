use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Knowledgebase {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub system_prompt: String,
    pub model: String,
    pub accent_color: String,
    pub logo_url: Option<String>,
    /// Wrapper user-id of the creator (display only — no access control).
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
