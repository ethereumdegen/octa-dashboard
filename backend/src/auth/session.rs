use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // user id (or project id for API key auth)
    pub email: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
    /// true when the caller authenticated via a real user session (not API key or dev mode)
    #[serde(skip)]
    pub is_user: bool,
}

impl Claims {
    /// Returns the user UUID only if authenticated as a real user.
    /// Returns None for API key auth, dev mode, etc.
    pub fn user_id(&self) -> Option<uuid::Uuid> {
        if self.is_user {
            self.sub.parse().ok()
        } else {
            None
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Claims {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Claims>()
            .cloned()
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

pub fn create_token(user_id: Uuid, email: &str, role: &str, secret: &str) -> String {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        role: role.to_string(),
        iat: now.timestamp(),
        exp: (now + Duration::days(7)).timestamp(),
        is_user: true,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("JWT encoding should not fail")
}

pub fn verify_token(token: &str, secret: &str) -> Option<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .ok()
    .map(|data| data.claims)
}
