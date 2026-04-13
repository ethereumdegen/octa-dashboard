use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::storage;
use crate::AppState;

pub async fn list_documents(State(state): State<AppState>) -> impl IntoResponse {
    match storage::list_documents(&state.pool).await {
        Ok(docs) => Json(serde_json::json!(docs)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn get_document(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match storage::get_document(&state.pool, id).await {
        Ok(Some(doc)) => Json(serde_json::json!(doc)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn create_document(
    State(state): State<AppState>,
    Json(body): Json<storage::CreateDocument>,
) -> impl IntoResponse {
    match storage::create_document(&state.pool, body).await {
        Ok(doc) => (StatusCode::CREATED, Json(serde_json::json!(doc))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn update_document(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<storage::UpdateDocument>,
) -> impl IntoResponse {
    match storage::update_document(&state.pool, id, body).await {
        Ok(Some(doc)) => Json(serde_json::json!(doc)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn delete_document(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match storage::delete_document(&state.pool, id).await {
        Ok(true) => Json(serde_json::json!({"deleted": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

pub async fn search_documents(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    match storage::search_documents(&state.pool, &query.q).await {
        Ok(docs) => Json(serde_json::json!(docs)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ── Export/Import ────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize, Deserialize)]
pub struct VaultExport {
    pub version: u32,
    pub exported_at: String,
    pub documents: Vec<VaultDocument>,
}

#[derive(Debug, serde::Serialize, Deserialize)]
pub struct VaultDocument {
    pub path: String,
    pub content: String,
    pub is_folder: bool,
}

#[derive(Debug)]
struct DbDoc {
    id: Uuid,
    title: String,
    slug: String,
    content: String,
    parent_id: Option<Uuid>,
    is_folder: bool,
    sort_order: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// GET /api/export
pub async fn export_documents(State(state): State<AppState>) -> impl IntoResponse {
    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let rows = match client
        .query(
            "SELECT id, title, slug, content, parent_id, is_folder, sort_order, created_at, updated_at
             FROM kb_documents ORDER BY sort_order, title",
            &[],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let docs: Vec<DbDoc> = rows
        .iter()
        .map(|r| DbDoc {
            id: r.get("id"),
            title: r.get("title"),
            slug: r.get("slug"),
            content: r.get("content"),
            parent_id: r.get("parent_id"),
            is_folder: r.get("is_folder"),
            sort_order: r.get("sort_order"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect();

    let vault_docs: Vec<VaultDocument> = docs
        .iter()
        .map(|doc| {
            let path = build_path(doc, &docs);
            let content = format_frontmatter(doc, &doc.content);
            VaultDocument {
                path,
                content,
                is_folder: doc.is_folder,
            }
        })
        .collect();

    let export = VaultExport {
        version: 1,
        exported_at: Utc::now().to_rfc3339(),
        documents: vault_docs,
    };

    Json(serde_json::json!(export)).into_response()
}

/// POST /api/import
pub async fn import_documents(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<VaultExport>,
) -> impl IntoResponse {
    // User ID can be forwarded from the dashboard proxy via X-User-Id header
    let user_id: Option<Uuid> = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok());

    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let mut imported = 0u32;
    let mut errors: Vec<String> = Vec::new();
    let mut path_to_id: std::collections::HashMap<String, Uuid> = std::collections::HashMap::new();

    let mut sorted_docs = body.documents;
    sorted_docs.sort_by(|a, b| {
        let a_depth = a.path.matches('/').count();
        let b_depth = b.path.matches('/').count();
        a.is_folder
            .cmp(&b.is_folder)
            .reverse()
            .then(a_depth.cmp(&b_depth))
    });

    for vault_doc in &sorted_docs {
        let (title, slug, content, _) = parse_obsidian_md(&vault_doc.content, &vault_doc.path);

        let parent_id = if let Some(parent_path) = parent_path_of(&vault_doc.path) {
            if let Some(pid) = path_to_id.get(&parent_path) {
                Some(*pid)
            } else {
                match ensure_folder(&client, &parent_path, &mut path_to_id, user_id).await {
                    Ok(pid) => Some(pid),
                    Err(e) => {
                        errors.push(format!(
                            "Failed to create folder for {}: {e}",
                            vault_doc.path
                        ));
                        continue;
                    }
                }
            }
        } else {
            None
        };

        let result = client
            .query_one(
                "INSERT INTO kb_documents (title, slug, content, parent_id, is_folder, created_by, updated_by)
                 VALUES ($1, $2, $3, $4, $5, $6, $6)
                 ON CONFLICT (slug) DO UPDATE SET
                    title = EXCLUDED.title,
                    content = EXCLUDED.content,
                    parent_id = EXCLUDED.parent_id,
                    is_folder = EXCLUDED.is_folder,
                    updated_by = EXCLUDED.updated_by,
                    updated_at = now()
                 RETURNING id",
                &[
                    &title,
                    &slug,
                    &content,
                    &parent_id,
                    &vault_doc.is_folder,
                    &user_id,
                ],
            )
            .await;

        match result {
            Ok(row) => {
                let id: Uuid = row.get("id");
                let key = vault_doc.path.trim_end_matches(".md").to_string();
                path_to_id.insert(key, id);
                imported += 1;
            }
            Err(e) => {
                errors.push(format!("{}: {e}", vault_doc.path));
            }
        }
    }

    let status = if errors.is_empty() {
        StatusCode::OK
    } else {
        StatusCode::MULTI_STATUS
    };

    (
        status,
        Json(serde_json::json!({
            "imported": imported,
            "errors": errors,
        })),
    )
        .into_response()
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn build_path(doc: &DbDoc, all_docs: &[DbDoc]) -> String {
    let mut segments = vec![doc_filename(doc)];
    let mut current = doc;

    while let Some(pid) = current.parent_id {
        if let Some(parent) = all_docs.iter().find(|d| d.id == pid) {
            segments.push(parent.slug.clone());
            current = parent;
        } else {
            break;
        }
    }

    segments.reverse();
    segments.join("/")
}

fn doc_filename(doc: &DbDoc) -> String {
    if doc.is_folder {
        doc.slug.clone()
    } else {
        format!("{}.md", doc.slug)
    }
}

fn format_frontmatter(doc: &DbDoc, content: &str) -> String {
    format!(
        "---\ntitle: \"{}\"\nslug: \"{}\"\nis_folder: {}\nsort_order: {}\ncreated_at: \"{}\"\nupdated_at: \"{}\"\n---\n\n{}",
        doc.title.replace('"', "\\\""),
        doc.slug,
        doc.is_folder,
        doc.sort_order,
        doc.created_at.to_rfc3339(),
        doc.updated_at.to_rfc3339(),
        content,
    )
}

fn parse_obsidian_md(raw: &str, path: &str) -> (String, String, String, Vec<(String, String)>) {
    let mut title = String::new();
    let mut slug = String::new();
    let mut content = raw.to_string();
    let mut frontmatter = Vec::new();

    if raw.starts_with("---\n") || raw.starts_with("---\r\n") {
        if let Some(end) = raw[4..].find("\n---") {
            let fm_str = &raw[4..4 + end];
            content = raw[4 + end + 4..].trim_start_matches('\n').to_string();

            for line in fm_str.lines() {
                if let Some((key, val)) = line.split_once(':') {
                    let key = key.trim().to_string();
                    let val = val.trim().trim_matches('"').to_string();
                    match key.as_str() {
                        "title" => title = val.clone(),
                        "slug" => slug = val.clone(),
                        _ => {}
                    }
                    frontmatter.push((key, val));
                }
            }
        }
    }

    if title.is_empty() {
        let filename = path.rsplit('/').next().unwrap_or(path);
        title = filename
            .trim_end_matches(".md")
            .replace('-', " ")
            .replace('_', " ");
        if let Some(first) = title.get(0..1) {
            title = format!("{}{}", first.to_uppercase(), &title[1..]);
        }
    }
    if slug.is_empty() {
        let filename = path.rsplit('/').next().unwrap_or(path);
        slug = filename
            .trim_end_matches(".md")
            .to_lowercase()
            .replace(' ', "-");
    }

    (title, slug, content, frontmatter)
}

fn parent_path_of(path: &str) -> Option<String> {
    let path = path.trim_end_matches(".md");
    if let Some(pos) = path.rfind('/') {
        Some(path[..pos].to_string())
    } else {
        None
    }
}

async fn ensure_folder(
    client: &deadpool_postgres::Object,
    folder_path: &str,
    path_to_id: &mut std::collections::HashMap<String, Uuid>,
    user_id: Option<Uuid>,
) -> Result<Uuid, String> {
    if let Some(id) = path_to_id.get(folder_path) {
        return Ok(*id);
    }

    let parent_id = if let Some(pp) = parent_path_of(&format!("{folder_path}.md")) {
        match Box::pin(ensure_folder(client, &pp, path_to_id, user_id)).await {
            Ok(pid) => Some(pid),
            Err(e) => return Err(e),
        }
    } else {
        None
    };

    let folder_name = folder_path.rsplit('/').next().unwrap_or(folder_path);
    let title = folder_name.replace('-', " ").replace('_', " ");
    let slug = folder_name.to_lowercase().replace(' ', "-");

    let result = client
        .query_one(
            "INSERT INTO kb_documents (title, slug, content, parent_id, is_folder, created_by, updated_by)
             VALUES ($1, $2, '', $3, true, $4, $4)
             ON CONFLICT (slug) DO UPDATE SET parent_id = EXCLUDED.parent_id
             RETURNING id",
            &[&title, &slug, &parent_id, &user_id],
        )
        .await
        .map_err(|e| e.to_string())?;

    let id: Uuid = result.get("id");
    path_to_id.insert(folder_path.to_string(), id);
    Ok(id)
}
