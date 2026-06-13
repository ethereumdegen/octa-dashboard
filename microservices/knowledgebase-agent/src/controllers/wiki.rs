use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::auth::KbCtx;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::services::s3;
use crate::state::AppState;

/// GET /api/kb/:kb_id/wiki — list auto-generated wiki pages
pub async fn list_pages(ctx: KbCtx, State(state): State<AppState>) -> AppResult<Json<Value>> {
    let pages = db::wiki_pages::list_for_kb(&state.db, ctx.kb.id).await?;
    Ok(Json(json!({
        "pages": pages,
        "total": pages.len(),
    })))
}

/// GET /api/kb/:kb_id/wiki/:slug — wiki page content (markdown from S3)
pub async fn get_page(
    ctx: KbCtx,
    State(state): State<AppState>,
    Path((_kb_id, slug)): Path<(Uuid, String)>,
) -> AppResult<Json<Value>> {
    let bucket = state.require_bucket()?;
    let page = db::wiki_pages::get_by_slug(&state.db, ctx.kb.id, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("wiki page '{slug}' not found")))?;

    let bytes = s3::download_bytes(bucket, &page.content_s3_key).await?;
    let markdown = String::from_utf8_lossy(&bytes).to_string();

    Ok(Json(json!({
        "slug": page.slug,
        "title": page.title,
        "summary": page.summary,
        "page_type": page.page_type,
        "sources": page.sources,
        "markdown": markdown,
        "document_id": page.document_id,
        "created_at": page.created_at,
        "updated_at": page.updated_at,
    })))
}
