use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::session::Claims;
use crate::db::models::TeamMember;
use crate::error::AppError;
use crate::AppState;

pub async fn list_team(State(state): State<AppState>) -> Result<Json<Vec<TeamMember>>, AppError> {
    let client = state.pool.get().await?;
    let rows = client
        .query(
            "SELECT t.id, t.email, t.role, t.invited_by, t.created_at, u.last_login_at AS last_active
             FROM team_members t
             LEFT JOIN users u ON u.email = t.email
             ORDER BY t.created_at",
            &[],
        )
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

    let client = state.pool.get().await?;
    let row = client
        .query_one(
            "INSERT INTO team_members (email, role, invited_by) VALUES ($1, $2, $3)
             ON CONFLICT (email) DO UPDATE SET role = EXCLUDED.role
             RETURNING id, email, role, invited_by, created_at",
            &[&body.email, &role, &inviter_id],
        )
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
    let client = state.pool.get().await?;
    let deleted = client
        .execute("DELETE FROM team_members WHERE id = $1", &[&id])
        .await?;

    if deleted == 0 {
        return Err(AppError::NotFound("Team member not found".into()));
    }

    Ok(Json(serde_json::json!({"deleted": true})))
}
