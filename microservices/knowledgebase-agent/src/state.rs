use std::sync::Arc;
use std::time::Instant;

use s3::Bucket;
use sqlx::PgPool;

use crate::config::Config;
use crate::error::AppError;
use crate::services::rag_cache::RagCache;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub bucket: Option<Arc<Box<Bucket>>>,
    pub config: Arc<Config>,
    pub rag_cache: Arc<RagCache>,
    pub start_time: Instant,
}

impl AppState {
    pub fn require_bucket(&self) -> Result<&Arc<Box<Bucket>>, AppError> {
        self.bucket
            .as_ref()
            .ok_or_else(|| AppError::BadRequest("S3 storage not configured".into()))
    }
}
