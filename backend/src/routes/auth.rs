use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
    Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::github_oauth;
use crate::auth::session::{create_token, Claims};
use crate::error::AppError;
use crate::AppState;

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: Option<String>,
}

pub async fn github_callback(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<CallbackQuery>,
) -> Result<(CookieJar, Redirect), AppError> {
    // Validate OAuth state parameter to prevent CSRF
    if let Some(ref provided_state) = query.state {
        let stored_state = jar.get("oauth_state").map(|c| c.value().to_string());
        if stored_state.as_deref() != Some(provided_state) {
            return Err(AppError::Forbidden("Invalid OAuth state — possible CSRF".into()));
        }
    }
    let access_token = github_oauth::exchange_code(
        &state.config.github_client_id,
        &state.config.github_client_secret,
        &query.code,
    )
    .await
    .map_err(|e| AppError::Internal(format!("GitHub token exchange failed: {e}")))?;

    let gh_user = github_oauth::fetch_user(&access_token)
        .await
        .map_err(|e| AppError::Internal(format!("GitHub user fetch failed: {e}")))?;

    let email = github_oauth::fetch_primary_email(&access_token)
        .await
        .map_err(|e| AppError::Internal(format!("GitHub email fetch failed: {e}")))?
        .ok_or_else(|| AppError::Unauthorized("No verified primary email on GitHub account".into()))?;

    let client = state.pool.get().await?;

    // Check allowlist
    let allowed = client
        .query_opt("SELECT role FROM team_members WHERE email = $1", &[&email])
        .await?;

    let role = match allowed {
        Some(row) => row.get::<_, String>("role"),
        None => {
            return Err(AppError::Forbidden(
                "Your email is not on the team allowlist".into(),
            ));
        }
    };

    let name = gh_user.name.unwrap_or(gh_user.login);
    let avatar = gh_user.avatar_url.unwrap_or_default();
    let github_id = gh_user.id;

    // Upsert user
    let row = client
        .query_one(
            "INSERT INTO users (github_id, email, name, avatar_url, role, last_login_at)
             VALUES ($1, $2, $3, $4, $5, now())
             ON CONFLICT (github_id) DO UPDATE SET
                email = EXCLUDED.email,
                name = EXCLUDED.name,
                avatar_url = EXCLUDED.avatar_url,
                role = EXCLUDED.role,
                last_login_at = now()
             RETURNING id",
            &[&github_id, &email, &name, &avatar, &role],
        )
        .await?;

    let user_id: Uuid = row.get("id");

    // Record analytics event
    let _ = client
        .execute(
            "INSERT INTO analytics_events (event_type, metadata, user_id) VALUES ('login', $1, $2)",
            &[&serde_json::json!({"method": "github"}), &user_id],
        )
        .await;

    let token = create_token(user_id, &email, &role, &state.config.jwt_secret);

    let is_prod = !state.config.skip_login;
    let cookie = Cookie::build(("session", token))
        .path("/")
        .http_only(true)
        .secure(is_prod)
        .same_site(axum_extra::extract::cookie::SameSite::Lax)
        .max_age(time::Duration::days(7))
        .build();

    // Clear the oauth_state cookie
    let clear_state = Cookie::build(("oauth_state", ""))
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();

    Ok((jar.add(cookie).add(clear_state), Redirect::to("/")))
}

pub async fn me(claims: Claims) -> impl IntoResponse {
    Json(serde_json::json!({
        "id": claims.sub,
        "email": claims.email,
        "role": claims.role,
    }))
}

pub async fn logout(jar: CookieJar) -> impl IntoResponse {
    let cookie = Cookie::build(("session", ""))
        .path("/")
        .http_only(true)
        .max_age(time::Duration::seconds(0))
        .build();
    (jar.add(cookie), Redirect::to("/login"))
}
