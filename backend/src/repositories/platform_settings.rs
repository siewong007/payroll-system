//! Data access for the `platform_settings` key/value table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

pub async fn get_attendance_method(
    executor: impl Executor<'_, Database = Postgres>,
) -> AppResult<Option<String>> {
    let value =
        sqlx::query_scalar!("SELECT value FROM platform_settings WHERE key = 'attendance_method'")
            .fetch_optional(executor)
            .await?;
    Ok(value)
}

pub async fn get_allow_override(
    executor: impl Executor<'_, Database = Postgres>,
) -> AppResult<Option<String>> {
    let value = sqlx::query_scalar!(
        "SELECT value FROM platform_settings WHERE key = 'allow_company_override'"
    )
    .fetch_optional(executor)
    .await?;
    Ok(value)
}

pub async fn set_attendance_method(
    executor: impl Executor<'_, Database = Postgres>,
    value: &str,
    updated_by: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "INSERT INTO platform_settings (key, value, updated_at, updated_by)
         VALUES ('attendance_method', $1, NOW(), $2)
         ON CONFLICT (key) DO UPDATE SET value = $1, updated_at = NOW(), updated_by = $2",
        value,
        updated_by,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn set_allow_override(
    executor: impl Executor<'_, Database = Postgres>,
    value: &str,
    updated_by: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "INSERT INTO platform_settings (key, value, updated_at, updated_by)
         VALUES ('allow_company_override', $1, NOW(), $2)
         ON CONFLICT (key) DO UPDATE SET value = $1, updated_at = NOW(), updated_by = $2",
        value,
        updated_by,
    )
    .execute(executor)
    .await?;
    Ok(())
}
