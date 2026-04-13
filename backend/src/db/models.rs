use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub github_id: i64,
    pub email: String,
    pub name: String,
    pub avatar_url: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub last_login_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub invited_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub last_active: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub url: String,
    pub manifest: serde_json::Value,
    pub status: String,
    pub last_health_check: Option<DateTime<Utc>>,
    pub registered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbDocument {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsEvent {
    pub id: Uuid,
    pub event_type: String,
    pub metadata: serde_json::Value,
    pub user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Microservice {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub slug: String,
    pub nav_path: String,
    pub enabled: bool,
    pub source_url: Option<String>,
    pub installed_at: DateTime<Utc>,
}
