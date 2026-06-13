use axum::{
    body::Body,
    extract::{Path, Request, State},
    http::{header, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
};

use sqlx::Row;

use crate::auth::session::Claims;
use crate::AppState;

pub async fn proxy_agent(
    State(state): State<AppState>,
    Path((agent_id, path)): Path<(String, String)>,
    claims: Claims,
    method: Method,
    uri: Uri,
    req: Request,
) -> Result<Response, StatusCode> {
    let row = sqlx::query("SELECT url FROM agents WHERE id = $1")
        .bind(agent_id.as_str())
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let base_url: String = row.get("url");

    // Preserve query string from the original request
    let target_url = match uri.query() {
        Some(qs) => format!("{base_url}/{path}?{qs}"),
        None => format!("{base_url}/{path}"),
    };

    let http_client = reqwest::Client::new();

    let mut builder = match method {
        Method::GET => http_client.get(&target_url),
        Method::POST => http_client.post(&target_url),
        Method::PUT => http_client.put(&target_url),
        Method::DELETE => http_client.delete(&target_url),
        Method::PATCH => http_client.patch(&target_url),
        _ => return Err(StatusCode::METHOD_NOT_ALLOWED),
    };

    // Authenticate with the agent
    let agent_secret = &state.config.agent_secret;
    if !agent_secret.is_empty() {
        builder = builder.header("Authorization", format!("Bearer {agent_secret}"));
    }

    // Forward authenticated user identity to the microservice
    if let Some(uid) = claims.user_id() {
        builder = builder.header("x-user-id", uid.to_string());
    }
    builder = builder.header("x-user-email", &claims.email);

    if method != Method::GET {
        if let Some(content_type) = req.headers().get("content-type") {
            builder = builder.header("content-type", content_type);
        }
        let body_bytes = axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        builder = builder.body(body_bytes);
    }

    let resp = builder
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let upstream_content_type = resp.headers().get(header::CONTENT_TYPE).cloned();
    let body_bytes = resp.bytes().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    let mut response = (status, Body::from(body_bytes)).into_response();

    // Forward Content-Type so JS/CSS/HTML are served with correct MIME types
    if let Some(ct) = upstream_content_type {
        response.headers_mut().insert(header::CONTENT_TYPE, ct);
    }

    Ok(response)
}
