//! Data access for the `password_reset_requests` table.

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::session::PasswordResetRequest;

/// Expire any pending/approved requests for a user (called before issuing a new one).
pub async fn expire_pending_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE password_reset_requests SET status = 'expired' WHERE user_id = $1 AND status IN ('pending', 'approved')",
        user_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Insert a pre-approved reset request carrying the hashed token.
pub async fn insert_approved(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    reset_token_hash: &str,
    expires_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO password_reset_requests (user_id, status, reset_token_hash, reset_token_expires_at)
        VALUES ($1, 'approved', $2, $3)"#,
        user_id,
        reset_token_hash,
        expires_at,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn expire_pending_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE password_reset_requests SET status = 'expired' WHERE user_id IN (SELECT id FROM users WHERE employee_id = $1) AND status IN ('pending', 'approved')",
        employee_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// User id behind a valid (approved, unexpired) reset token, if any.
pub async fn find_user_id_by_valid_token(
    executor: impl Executor<'_, Database = Postgres>,
    reset_token_hash: &str,
) -> AppResult<Option<Uuid>> {
    let user_id = sqlx::query_scalar!(
        r#"SELECT prr.user_id FROM password_reset_requests prr
        JOIN users u ON u.id = prr.user_id
        WHERE prr.reset_token_hash = $1
            AND prr.status = 'approved'
            AND prr.reset_token_expires_at > NOW()
            AND u.is_active = TRUE
            AND u.deleted_at IS NULL"#,
        reset_token_hash,
    )
    .fetch_optional(executor)
    .await?;
    Ok(user_id)
}

/// Full request behind a valid (approved, unexpired) reset token, if any.
pub async fn find_by_valid_token(
    executor: impl Executor<'_, Database = Postgres>,
    reset_token_hash: &str,
) -> AppResult<Option<PasswordResetRequest>> {
    let request = sqlx::query_as!(
        PasswordResetRequest,
        r#"SELECT prr.* FROM password_reset_requests prr
        JOIN users u ON u.id = prr.user_id
        WHERE prr.reset_token_hash = $1
            AND prr.status = 'approved'
            AND prr.reset_token_expires_at > NOW()
            AND u.is_active = TRUE
            AND u.deleted_at IS NULL"#,
        reset_token_hash,
    )
    .fetch_optional(executor)
    .await?;
    Ok(request)
}

pub async fn mark_completed(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE password_reset_requests SET status = 'completed', completed_at = NOW() WHERE id = $1",
        id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Atomically claim and complete a valid reset token inside a transaction.
pub async fn consume_valid_token(
    executor: impl Executor<'_, Database = Postgres>,
    reset_token_hash: &str,
) -> AppResult<Option<PasswordResetRequest>> {
    let request = sqlx::query_as!(
        PasswordResetRequest,
        r#"UPDATE password_reset_requests prr
           SET status = 'completed', completed_at = NOW()
           FROM users u
           WHERE prr.user_id = u.id
             AND prr.reset_token_hash = $1
             AND prr.status = 'approved'
             AND prr.reset_token_expires_at > NOW()
             AND u.is_active = TRUE
             AND u.deleted_at IS NULL
           RETURNING prr.*"#,
        reset_token_hash,
    )
    .fetch_optional(executor)
    .await?;
    Ok(request)
}

/// Purge terminal or expired password reset requests older than `retain_days`.
pub async fn delete_expired_older_than_days(
    executor: impl Executor<'_, Database = Postgres>,
    retain_days: i32,
) -> AppResult<u64> {
    let result = sqlx::query!(
        r#"DELETE FROM password_reset_requests
           WHERE (status IN ('completed', 'expired', 'rejected') OR reset_token_expires_at < NOW())
             AND COALESCE(reset_token_expires_at, requested_at) < NOW() - ($1::int * INTERVAL '1 day')"#,
        retain_days,
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}
