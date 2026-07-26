//! Data access for the `refresh_tokens` table.

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ActiveRefreshToken {
    pub user_id: Uuid,
    pub session_id: Uuid,
}

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    session_id: Uuid,
    token_hash: &str,
    expires_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query("INSERT INTO refresh_tokens (user_id, session_id, token_hash, expires_at) VALUES ($1, $2, $3, $4)")
        .bind(user_id).bind(session_id).bind(token_hash).bind(expires_at)
        .execute(executor).await?;
    Ok(())
}

/// Returns the owning `user_id` of a non-revoked, unexpired token, if any.
pub async fn find_active(
    executor: impl Executor<'_, Database = Postgres>,
    token_hash: &str,
) -> AppResult<Option<ActiveRefreshToken>> {
    Ok(sqlx::query_as::<_, ActiveRefreshToken>(
        "SELECT rt.user_id, rt.session_id FROM refresh_tokens rt JOIN user_sessions us ON us.id = rt.session_id WHERE rt.token_hash = $1 AND rt.revoked = FALSE AND rt.expires_at > NOW() AND us.revoked_at IS NULL AND us.expires_at > NOW()",
    ).bind(token_hash).fetch_optional(executor).await?)
}

/// Atomically retire a live token and return its owner.
///
/// The UPDATE *is* the guard. A `SELECT` followed by an `UPDATE` lets two tabs
/// presenting the same cookie both pass the read and both mint a successor,
/// leaving a second live credential on the session that `/auth/sessions` cannot
/// show or revoke. Here the row lock serialises them and exactly one gets a row
/// back; `None` means the token was already revoked, expired, or its session is
/// gone — a distinction the caller must classify rather than treat as a miss.
pub async fn consume_active(
    executor: impl Executor<'_, Database = Postgres>,
    token_hash: &str,
) -> AppResult<Option<ActiveRefreshToken>> {
    Ok(sqlx::query_as::<_, ActiveRefreshToken>(
        "UPDATE refresh_tokens rt SET revoked = TRUE \
         FROM user_sessions us \
         WHERE rt.token_hash = $1 AND rt.revoked = FALSE AND rt.expires_at > NOW() \
           AND us.id = rt.session_id AND us.revoked_at IS NULL AND us.expires_at > NOW() \
         RETURNING rt.user_id, rt.session_id",
    )
    .bind(token_hash)
    .fetch_optional(executor)
    .await?)
}

/// The owner of a token hash regardless of its revoked/expired state. Used only
/// to tell "this token was never issued here" apart from "this token was
/// already rotated away", which are the same 401 to the client but very
/// different events.
pub async fn find_owner_of_hash(
    executor: impl Executor<'_, Database = Postgres>,
    token_hash: &str,
) -> AppResult<Option<ActiveRefreshToken>> {
    Ok(sqlx::query_as::<_, ActiveRefreshToken>(
        "SELECT user_id, session_id FROM refresh_tokens WHERE token_hash = $1",
    )
    .bind(token_hash)
    .fetch_optional(executor)
    .await?)
}

/// True if this session was issued a still-live token within the last `seconds`.
///
/// `created_at` on the successor row is what makes the reuse grace window
/// expressible without a `revoked_at` column: a presented-but-dead token whose
/// session has a brand-new sibling is a tab that lost a rotation race, not a
/// replay.
pub async fn has_fresh_sibling(
    executor: impl Executor<'_, Database = Postgres>,
    session_id: Uuid,
    seconds: f64,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM refresh_tokens \
         WHERE session_id = $1 AND revoked = FALSE AND expires_at > NOW() \
           AND created_at > NOW() - make_interval(secs => $2))",
    )
    .bind(session_id)
    .bind(seconds)
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// Revoke a token by hash, reporting how many rows it actually retired.
///
/// `AND revoked = FALSE` plus the returned count is what lets a caller use this
/// as a guard rather than a fire-and-forget write.
pub async fn revoke_by_hash(
    executor: impl Executor<'_, Database = Postgres>,
    token_hash: &str,
) -> AppResult<u64> {
    let result = sqlx::query!(
        "UPDATE refresh_tokens SET revoked = TRUE WHERE token_hash = $1 AND revoked = FALSE",
        token_hash,
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

/// Revoke the refresh tokens of every session this user has already revoked,
/// except the one they are currently using.
pub async fn revoke_for_revoked_sessions(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    current_session_id: Uuid,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE refresh_tokens SET revoked = TRUE WHERE session_id IN (SELECT id FROM user_sessions WHERE user_id = $1 AND id <> $2 AND revoked_at IS NOT NULL)",
    )
    .bind(user_id)
    .bind(current_session_id)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn revoke_for_session(
    executor: impl Executor<'_, Database = Postgres>,
    session_id: Uuid,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE refresh_tokens SET revoked = TRUE WHERE session_id = $1 AND revoked = FALSE",
    )
    .bind(session_id)
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
