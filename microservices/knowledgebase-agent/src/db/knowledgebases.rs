use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::knowledgebase::Knowledgebase;

pub async fn create(
    pool: &PgPool,
    created_by: Option<Uuid>,
    name: &str,
    slug: &str,
    description: &str,
    model: &str,
) -> AppResult<Knowledgebase> {
    let kb = sqlx::query_as::<_, Knowledgebase>(
        r#"
        INSERT INTO knowledgebases (created_by, name, slug, description, model)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(created_by)
    .bind(name)
    .bind(slug)
    .bind(description)
    .bind(model)
    .fetch_one(pool)
    .await?;
    Ok(kb)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<Knowledgebase>> {
    let kb = sqlx::query_as::<_, Knowledgebase>("SELECT * FROM knowledgebases WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(kb)
}

/// Global scope: every KB is visible to every caller (no login/ownership).
pub async fn list_all(pool: &PgPool) -> AppResult<Vec<Knowledgebase>> {
    let kbs = sqlx::query_as::<_, Knowledgebase>(
        "SELECT * FROM knowledgebases ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(kbs)
}

/// Update editable KB fields (settings panel).
pub async fn update(
    pool: &PgPool,
    id: Uuid,
    name: &str,
    description: &str,
    system_prompt: &str,
    model: &str,
    accent_color: &str,
) -> AppResult<Knowledgebase> {
    let kb = sqlx::query_as::<_, Knowledgebase>(
        r#"
        UPDATE knowledgebases
        SET name = $2, description = $3, system_prompt = $4, model = $5,
            accent_color = $6, updated_at = now()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(name)
    .bind(description)
    .bind(system_prompt)
    .bind(model)
    .bind(accent_color)
    .fetch_one(pool)
    .await?;
    Ok(kb)
}

pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<bool> {
    let result = sqlx::query("DELETE FROM knowledgebases WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
