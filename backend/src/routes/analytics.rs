use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::error::AppError;
use crate::AppState;

pub async fn summary(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let client = state.pool.get().await?;

    let total_users: i64 = client
        .query_one("SELECT COUNT(*) as count FROM users", &[])
        .await?
        .get("count");

    let total_agents: i64 = client
        .query_one("SELECT COUNT(*) as count FROM agents", &[])
        .await?
        .get("count");

    let total_docs: i64 = client
        .query_one("SELECT COUNT(*) as count FROM kb_documents", &[])
        .await?
        .get("count");

    let logins_today: i64 = client
        .query_one(
            "SELECT COUNT(*) as count FROM analytics_events
             WHERE event_type = 'login' AND created_at >= CURRENT_DATE",
            &[],
        )
        .await?
        .get("count");

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
    let client = state.pool.get().await?;

    let rows = client
        .query(
            "SELECT DATE(created_at) as date, COUNT(*) as count
             FROM analytics_events
             WHERE event_type = $1 AND created_at >= CURRENT_DATE - ($2 || ' days')::interval
             GROUP BY DATE(created_at)
             ORDER BY date",
            &[&metric, &days.to_string()],
        )
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
