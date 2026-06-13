use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::folder::{BreadcrumbEntry, DocFolder};

pub async fn insert(
    pool: &PgPool,
    kb_id: Uuid,
    parent_id: Option<Uuid>,
    name: &str,
    category: Option<&str>,
    created_by: Option<Uuid>,
) -> AppResult<DocFolder> {
    let folder = sqlx::query_as::<_, DocFolder>(
        r#"
        INSERT INTO doc_folders (kb_id, parent_id, name, category, created_by)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(kb_id)
    .bind(parent_id)
    .bind(name)
    .bind(category)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.code().as_deref() == Some("23505") {
                return AppError::BadRequest(format!(
                    "a folder named \"{name}\" already exists here"
                ));
            }
        }
        AppError::Database(e)
    })?;
    Ok(folder)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<DocFolder>> {
    let folder = sqlx::query_as::<_, DocFolder>("SELECT * FROM doc_folders WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(folder)
}

pub async fn list_children(
    pool: &PgPool,
    kb_id: Uuid,
    parent_id: Option<Uuid>,
) -> AppResult<Vec<DocFolder>> {
    let folders = sqlx::query_as::<_, DocFolder>(
        r#"
        SELECT * FROM doc_folders
        WHERE kb_id = $1 AND parent_id IS NOT DISTINCT FROM $2
        ORDER BY name ASC
        "#,
    )
    .bind(kb_id)
    .bind(parent_id)
    .fetch_all(pool)
    .await?;
    Ok(folders)
}

pub async fn breadcrumb(pool: &PgPool, folder_id: Uuid) -> AppResult<Vec<BreadcrumbEntry>> {
    let entries = sqlx::query_as::<_, BreadcrumbEntry>(
        r#"
        WITH RECURSIVE chain AS (
            SELECT id, name, parent_id, 0 AS depth
            FROM doc_folders WHERE id = $1
            UNION ALL
            SELECT f.id, f.name, f.parent_id, c.depth + 1
            FROM doc_folders f
            JOIN chain c ON f.id = c.parent_id
        )
        SELECT id, name, parent_id FROM chain ORDER BY depth DESC
        "#,
    )
    .bind(folder_id)
    .fetch_all(pool)
    .await?;
    Ok(entries)
}

pub async fn rename(pool: &PgPool, id: Uuid, name: &str) -> AppResult<DocFolder> {
    let folder = sqlx::query_as::<_, DocFolder>(
        "UPDATE doc_folders SET name = $2, updated_at = now() WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(name)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.code().as_deref() == Some("23505") {
                return AppError::BadRequest(format!(
                    "a folder named \"{name}\" already exists here"
                ));
            }
        }
        AppError::Database(e)
    })?;
    Ok(folder)
}

pub async fn move_folder(pool: &PgPool, id: Uuid, parent_id: Option<Uuid>) -> AppResult<DocFolder> {
    let folder = sqlx::query_as::<_, DocFolder>(
        "UPDATE doc_folders SET parent_id = $2, updated_at = now() WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(parent_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.code().as_deref() == Some("23505") {
                return AppError::BadRequest(
                    "a folder with that name already exists in the target location".into(),
                );
            }
        }
        AppError::Database(e)
    })?;
    Ok(folder)
}

pub async fn update_category(
    pool: &PgPool,
    id: Uuid,
    category: Option<&str>,
) -> AppResult<DocFolder> {
    let folder = sqlx::query_as::<_, DocFolder>(
        "UPDATE doc_folders SET category = $2, updated_at = now() WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(category)
    .fetch_one(pool)
    .await?;
    Ok(folder)
}

pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<bool> {
    let result = sqlx::query("DELETE FROM doc_folders WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Check if `target_id` is a descendant of `folder_id` (circular ref check).
pub async fn is_descendant(pool: &PgPool, folder_id: Uuid, target_id: Uuid) -> AppResult<bool> {
    if folder_id == target_id {
        return Ok(true);
    }
    let row: (bool,) = sqlx::query_as(
        r#"
        WITH RECURSIVE chain AS (
            SELECT id, parent_id FROM doc_folders WHERE id = $2
            UNION ALL
            SELECT f.id, f.parent_id
            FROM doc_folders f
            JOIN chain c ON f.id = c.parent_id
        )
        SELECT EXISTS(SELECT 1 FROM chain WHERE id = $1)
        "#,
    )
    .bind(folder_id)
    .bind(target_id)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)?;
    Ok(row.0)
}
