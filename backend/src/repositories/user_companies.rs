//! Data access for the `user_companies` link table.
//!
//! Partial: seeded with what `employee_service` needs. Other domains add their own.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

/// Idempotent link insert (`ON CONFLICT DO NOTHING`); safe to call repeatedly.
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO user_companies (user_id, company_id)
        VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
        user_id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Strict link insert (no `ON CONFLICT`); errors on a duplicate. Callers use this
/// after clearing the user's existing links, where a conflict shouldn't occur.
pub async fn add(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "INSERT INTO user_companies (user_id, company_id) VALUES ($1, $2)",
        user_id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// True if the user is linked to the given company.
pub async fn user_has_company(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    company_id: Uuid,
) -> AppResult<bool> {
    let found = sqlx::query_scalar!(
        "SELECT user_id FROM user_companies WHERE user_id = $1 AND company_id = $2",
        user_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(found.is_some())
}

pub async fn delete_by_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<()> {
    sqlx::query!("DELETE FROM user_companies WHERE user_id = $1", user_id)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn delete_by_employee(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM user_companies WHERE user_id IN (SELECT id FROM users WHERE employee_id = $1)",
        employee_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}
