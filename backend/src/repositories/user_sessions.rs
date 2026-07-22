//! Data access for persistent per-device user sessions.

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::session::UserSession;

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    user_id: Uuid,
    user_agent: Option<&str>,
    expires_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO user_sessions (id, user_id, user_agent, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(user_id)
    .bind(user_agent)
    .bind(expires_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn is_active(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    session_id: Uuid,
) -> AppResult<bool> {
    let found = sqlx::query_scalar::<_, i32>(
        "SELECT 1 FROM user_sessions WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL AND expires_at > NOW()",
    ).bind(session_id).bind(user_id).fetch_optional(executor).await?;
    Ok(found.is_some())
}

pub async fn list_active(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<Vec<UserSession>> {
    Ok(sqlx::query_as::<_, UserSession>(
        "SELECT id, user_id, user_agent, created_at, last_seen_at, expires_at FROM user_sessions WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > NOW() ORDER BY last_seen_at DESC",
    ).bind(user_id).fetch_all(executor).await?)
}

pub async fn revoke(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    session_id: Uuid,
) -> AppResult<bool> {
    let result = sqlx::query("UPDATE user_sessions SET revoked_at = NOW() WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL")
        .bind(session_id).bind(user_id).execute(executor).await?;
    Ok(result.rows_affected() > 0)
}

pub async fn revoke_others(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    current_session_id: Uuid,
) -> AppResult<u64> {
    let result = sqlx::query("UPDATE user_sessions SET revoked_at = NOW() WHERE user_id = $1 AND id <> $2 AND revoked_at IS NULL")
        .bind(user_id).bind(current_session_id).execute(executor).await?;
    Ok(result.rows_affected())
}

pub async fn touch(
    executor: impl Executor<'_, Database = Postgres>,
    session_id: Uuid,
    expires_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query("UPDATE user_sessions SET last_seen_at = NOW(), expires_at = $2 WHERE id = $1")
        .bind(session_id)
        .bind(expires_at)
        .execute(executor)
        .await?;
    Ok(())
}
