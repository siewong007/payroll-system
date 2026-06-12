//! Data access for the `company_settings` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::setting::CompanySetting;

pub async fn list(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    category: Option<&str>,
) -> AppResult<Vec<CompanySetting>> {
    let settings = sqlx::query_as!(
        CompanySetting,
        r#"SELECT * FROM company_settings
        WHERE company_id = $1
        AND ($2::text IS NULL OR category = $2)
        ORDER BY category, key"#,
        company_id,
        category,
    )
    .fetch_all(executor)
    .await?;
    Ok(settings)
}

pub async fn get(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    category: &str,
    key: &str,
) -> AppResult<Option<CompanySetting>> {
    let setting = sqlx::query_as!(
        CompanySetting,
        "SELECT * FROM company_settings WHERE company_id = $1 AND category = $2 AND key = $3",
        company_id,
        category,
        key,
    )
    .fetch_optional(executor)
    .await?;
    Ok(setting)
}

/// Update a single setting's value, returning the updated row (or `None` if absent).
pub async fn update(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    category: &str,
    key: &str,
    value: &serde_json::Value,
    updated_by: Uuid,
) -> AppResult<Option<CompanySetting>> {
    let setting = sqlx::query_as!(
        CompanySetting,
        r#"UPDATE company_settings
        SET value = $4, updated_by = $5, updated_at = NOW()
        WHERE company_id = $1 AND category = $2 AND key = $3
        RETURNING *"#,
        company_id,
        category,
        key,
        value,
        updated_by,
    )
    .fetch_optional(executor)
    .await?;
    Ok(setting)
}
