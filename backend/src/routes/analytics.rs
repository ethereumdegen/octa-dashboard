use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use sqlx::Row;

use crate::error::AppError;
use crate::AppState;

pub async fn summary(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await?;

    let total_agents: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agents")
        .fetch_one(&state.pool)
        .await?;

    // Documents now live in the RAG knowledgebase's `documents` table
    // (the legacy `kb_documents` table was dropped in migration 019).
    let total_docs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM documents")
        .fetch_one(&state.pool)
        .await?;

    let logins_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM analytics_events
         WHERE event_type = 'login' AND created_at >= CURRENT_DATE",
    )
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({
        "total_users": total_users,
        "total_agents": total_agents,
        "total_documents": total_docs,
        "logins_today": logins_today,
    })))
}

#[derive(Deserialize)]
pub struct ChartQuery {
    pub days: Option<i32>,
}

pub async fn chart(
    State(state): State<AppState>,
    Path(metric): Path<String>,
    Query(query): Query<ChartQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let days = query.days.unwrap_or(7);

    let rows = sqlx::query(
        "SELECT DATE(created_at) as date, COUNT(*) as count
         FROM analytics_events
         WHERE event_type = $1 AND created_at >= CURRENT_DATE - ($2 || ' days')::interval
         GROUP BY DATE(created_at)
         ORDER BY date",
    )
    .bind(metric.as_str())
    .bind(days.to_string())
    .fetch_all(&state.pool)
    .await?;

    let data: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            let date: chrono::NaiveDate = r.get("date");
            let count: i64 = r.get("count");
            serde_json::json!({"date": date.to_string(), "count": count})
        })
        .collect();

    Ok(Json(serde_json::json!({"metric": metric, "days": days, "data": data})))
}
