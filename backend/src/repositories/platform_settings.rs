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

/// Both attendance-related platform settings in one round trip. Runs on every
/// check-in (method gate), so the two key lookups are folded into one query.
pub async fn get_attendance_settings(
    executor: impl Executor<'_, Database = Postgres>,
) -> AppResult<(Option<String>, Option<String>)> {
    let row = sqlx::query!(
        r#"SELECT
               MAX(value) FILTER (WHERE key = 'attendance_method') AS attendance_method,
               MAX(value) FILTER (WHERE key = 'allow_company_override') AS allow_override
           FROM platform_settings
           WHERE key IN ('attendance_method', 'allow_company_override')"#
    )
    .fetch_one(executor)
    .await?;
    Ok((row.attendance_method, row.allow_override))
}

/// Read an arbitrary platform setting value by key.
pub async fn get_value(
    executor: impl Executor<'_, Database = Postgres>,
    key: &str,
) -> AppResult<Option<String>> {
    let value = sqlx::query_scalar!("SELECT value FROM platform_settings WHERE key = $1", key)
        .fetch_optional(executor)
        .await?;
    Ok(value)
}

/// Upsert an arbitrary platform setting. `updated_by` is NULL for values
/// written by background tasks rather than a user action.
pub async fn set_value(
    executor: impl Executor<'_, Database = Postgres>,
    key: &str,
    value: &str,
    updated_by: Option<Uuid>,
) -> AppResult<()> {
    sqlx::query!(
        "INSERT INTO platform_settings (key, value, updated_at, updated_by)
         VALUES ($1, $2, NOW(), $3)
         ON CONFLICT (key) DO UPDATE SET value = $2, updated_at = NOW(), updated_by = $3",
        key,
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
