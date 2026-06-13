use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ChatJob {
    pub id: Uuid,
    pub session_id: Uuid,
    pub kb_id: Uuid,
    pub owner_id: Option<Uuid>,
    pub status: String,
    pub worker_id: Option<String>,
    pub content: String,
    pub error: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
