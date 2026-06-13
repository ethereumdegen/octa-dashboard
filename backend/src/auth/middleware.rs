use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use axum_extra::extract::CookieJar;
use sha2::{Digest, Sha256};
use sqlx::Row;

use crate::auth::session::{verify_token, Claims};

/// Marker inserted via Extension when SKIP_LOGIN=true
#[derive(Clone)]
pub struct SkipLogin;

/// Database pool passed via Extension for API key lookups
#[derive(Clone)]
pub struct AuthPool(pub sqlx::PgPool);

fn dev_claims() -> Claims {
    Claims {
        sub: "00000000-0000-0000-0000-000000000000".to_string(),
        email: "dev@localhost".to_string(),
        role: "admin".to_string(),
        exp: i64::MAX,
        iat: 0,
        is_user: false,
    }
}

fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

/// Extract API key token from Authorization header if present
fn extract_api_token(req: &Request) -> Option<String> {
    let header = req.headers().get("authorization")?.to_str().ok()?;
    let token = header.strip_prefix("Bearer ")?;
    if token.starts_with("tk_") {
        Some(token.to_string())
    } else {
        None
    }
}

/// Look up an API key and return Claims for the owning user
async fn resolve_api_key(pool: &sqlx::PgPool, token: &str) -> Option<Claims> {
    let key_hash = hash_api_key(token);

    let row = sqlx::query(
        "SELECT ak.id, ak.project_id
         FROM api_keys ak
         WHERE ak.key_hash = $1 AND ak.revoked_at IS NULL",
    )
    .bind(key_hash.as_str())
    .fetch_optional(pool)
    .await
    .ok()??;

    let key_id: uuid::Uuid = row.get("id");
    let project_id: uuid::Uuid = row.get("project_id");

    // Update last_used_at (fire and forget)
    let _ = sqlx::query("UPDATE api_keys SET last_used_at = now() WHERE id = $1")
        .bind(key_id)
        .execute(pool)
        .await;

    // API key auth gets admin-level access scoped to its project
    Some(Claims {
        sub: project_id.to_string(),
        email: "api-key".to_string(),
        role: "admin".to_string(),
        exp: i64::MAX,
        iat: 0,
        is_user: false,
    })
}

pub async fn require_auth(
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if req.extensions().get::<SkipLogin>().is_some() {
        req.extensions_mut().insert(dev_claims());
        return Ok(next.run(req).await);
    }

    // Try API key auth first
    let api_token = extract_api_token(&req);
    if let Some(token) = api_token {
        let pool = req
            .extensions()
            .get::<AuthPool>()
            .cloned()
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        let claims = resolve_api_key(&pool.0, &token)
            .await
            .ok_or(StatusCode::UNAUTHORIZED)?;
        req.extensions_mut().insert(claims);
        return Ok(next.run(req).await);
    }

    // Fall back to cookie/JWT auth
    let jwt_secret = req
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_default();

    let token = jar
        .get("session")
        .map(|c| c.value().to_string())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let mut claims = verify_token(&token, &jwt_secret).ok_or(StatusCode::UNAUTHORIZED)?;
    claims.is_user = true;
    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

pub async fn require_admin(
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if req.extensions().get::<SkipLogin>().is_some() {
        req.extensions_mut().insert(dev_claims());
        return Ok(next.run(req).await);
    }

    // Try API key auth first
    let api_token = extract_api_token(&req);
    if let Some(token) = api_token {
        let pool = req
            .extensions()
            .get::<AuthPool>()
            .cloned()
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        let claims = resolve_api_key(&pool.0, &token)
            .await
            .ok_or(StatusCode::UNAUTHORIZED)?;
        if claims.role != "admin" {
            return Err(StatusCode::FORBIDDEN);
        }
        req.extensions_mut().insert(claims);
        return Ok(next.run(req).await);
    }

    // Fall back to cookie/JWT auth
    let jwt_secret = req
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_default();

    let token = jar
        .get("session")
        .map(|c| c.value().to_string())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let mut claims = verify_token(&token, &jwt_secret).ok_or(StatusCode::UNAUTHORIZED)?;
    claims.is_user = true;
    if claims.role != "admin" {
        return Err(StatusCode::FORBIDDEN);
    }
    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}
