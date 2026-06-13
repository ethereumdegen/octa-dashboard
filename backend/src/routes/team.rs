use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use crate::auth::session::Claims;
use crate::db::models::TeamMember;
use crate::error::AppError;
use crate::AppState;

pub async fn list_team(State(state): State<AppState>) -> Result<Json<Vec<TeamMember>>, AppError> {
    let rows = sqlx::query(
        "SELECT t.id, t.email, t.role, t.invited_by, t.created_at, u.last_login_at AS last_active
         FROM team_members t
         LEFT JOIN users u ON u.email = t.email
         ORDER BY t.created_at",
    )
    .fetch_all(&state.pool)
    .await?;

    let members: Vec<TeamMember> = rows
        .iter()
        .map(|r| TeamMember {
            id: r.get("id"),
            email: r.get("email"),
            role: r.get("role"),
            invited_by: r.get("invited_by"),
            created_at: r.get("created_at"),
            last_active: r.get("last_active"),
        })
        .collect();

    Ok(Json(members))
}

#[derive(Deserialize)]
pub struct AddMemberRequest {
    pub email: String,
    pub role: Option<String>,
}

pub async fn add_member(
    State(state): State<AppState>,
    claims: Claims,
    Json(body): Json<AddMemberRequest>,
) -> Result<Json<TeamMember>, AppError> {
    let inviter_id: Option<Uuid> = claims.user_id();
    let role = body.role.unwrap_or_else(|| "member".to_string());
    if role != "admin" && role != "member" {
        return Err(AppError::BadRequest("Role must be 'admin' or 'member'".into()));
    }

    let row = sqlx::query(
        "INSERT INTO team_members (email, role, invited_by) VALUES ($1, $2, $3)
         ON CONFLICT (email) DO UPDATE SET role = EXCLUDED.role
         RETURNING id, email, role, invited_by, created_at",
    )
    .bind(body.email.as_str())
    .bind(role.as_str())
    .bind(inviter_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(TeamMember {
        id: row.get("id"),
        email: row.get("email"),
        role: row.get("role"),
        invited_by: row.get("invited_by"),
        created_at: row.get("created_at"),
        last_active: None,
    }))
}

pub async fn remove_member(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let deleted = sqlx::query("DELETE FROM team_members WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?
        .rows_affected();

    if deleted == 0 {
        return Err(AppError::NotFound("Team member not found".into()));
    }

    Ok(Json(serde_json::json!({"deleted": true})))
}
