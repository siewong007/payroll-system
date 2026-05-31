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

/// User id behind a valid (approved, unexpired) reset token, if any.
pub async fn find_user_id_by_valid_token(
    executor: impl Executor<'_, Database = Postgres>,
    reset_token_hash: &str,
) -> AppResult<Option<Uuid>> {
    let user_id = sqlx::query_scalar!(
        r#"SELECT user_id FROM password_reset_requests
        WHERE reset_token_hash = $1
            AND status = 'approved'
            AND reset_token_expires_at > NOW()"#,
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
        r#"SELECT * FROM password_reset_requests
        WHERE reset_token_hash = $1
            AND status = 'approved'
            AND reset_token_expires_at > NOW()"#,
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
