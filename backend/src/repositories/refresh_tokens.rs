//! Data access for the `refresh_tokens` table.

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    token_hash: &str,
    expires_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO refresh_tokens (user_id, token_hash, expires_at)
        VALUES ($1, $2, $3)"#,
        user_id,
        token_hash,
        expires_at,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Returns the owning `user_id` of a non-revoked, unexpired token, if any.
pub async fn find_active_user_id(
    executor: impl Executor<'_, Database = Postgres>,
    token_hash: &str,
) -> AppResult<Option<Uuid>> {
    let user_id = sqlx::query_scalar!(
        r#"SELECT user_id FROM refresh_tokens
        WHERE token_hash = $1 AND revoked = FALSE AND expires_at > NOW()"#,
        token_hash,
    )
    .fetch_optional(executor)
    .await?;
    Ok(user_id)
}

pub async fn revoke_by_hash(
    executor: impl Executor<'_, Database = Postgres>,
    token_hash: &str,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE refresh_tokens SET revoked = TRUE WHERE token_hash = $1",
        token_hash,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Revoke every active refresh token for a user (e.g. after a password reset).
pub async fn revoke_all_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE refresh_tokens SET revoked = TRUE WHERE user_id = $1 AND revoked = FALSE",
        user_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn delete_by_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<()> {
    sqlx::query!("DELETE FROM refresh_tokens WHERE user_id = $1", user_id)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn delete_by_employee(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM refresh_tokens WHERE user_id IN (SELECT id FROM users WHERE employee_id = $1)",
        employee_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}
