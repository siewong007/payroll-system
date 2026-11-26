//! Data access for single-use server-tracked pre-2FA MFA challenges.

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MfaChallenge {
    pub jti: Uuid,
    pub user_id: Uuid,
    pub attempts: i32,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    jti: Uuid,
    user_id: Uuid,
    expires_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO mfa_challenges (jti, user_id, expires_at)
           VALUES ($1, $2, $3)"#,
        jti,
        user_id,
        expires_at,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_for_update(
    executor: impl Executor<'_, Database = Postgres>,
    jti: Uuid,
    user_id: Uuid,
) -> AppResult<Option<MfaChallenge>> {
    let row = sqlx::query_as!(
        MfaChallenge,
        r#"SELECT jti, user_id, attempts, expires_at, consumed_at, created_at
           FROM mfa_challenges
           WHERE jti = $1 AND user_id = $2
           FOR UPDATE"#,
        jti,
        user_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

pub async fn consume(
    executor: impl Executor<'_, Database = Postgres>,
    jti: Uuid,
    user_id: Uuid,
    max_attempts: i32,
) -> AppResult<bool> {
    let rows = sqlx::query!(
        r#"UPDATE mfa_challenges
           SET consumed_at = NOW()
           WHERE jti = $1
             AND user_id = $2
             AND consumed_at IS NULL
             AND expires_at > NOW()
             AND attempts < $3"#,
        jti,
        user_id,
        max_attempts,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows == 1)
}

pub async fn record_failed_attempt(
    executor: impl Executor<'_, Database = Postgres>,
    jti: Uuid,
    user_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE mfa_challenges
           SET attempts = attempts + 1
           WHERE jti = $1 AND user_id = $2 AND consumed_at IS NULL"#,
        jti,
        user_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn revoke_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<()> {
    sqlx::query!("DELETE FROM mfa_challenges WHERE user_id = $1", user_id)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn revoke_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM mfa_challenges WHERE user_id IN (SELECT id FROM users WHERE employee_id = $1)",
        employee_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn purge_expired(executor: impl Executor<'_, Database = Postgres>) -> AppResult<u64> {
    let rows = sqlx::query!(
        r#"DELETE FROM mfa_challenges
           WHERE expires_at < NOW() - INTERVAL '1 day'
              OR consumed_at < NOW() - INTERVAL '1 day'"#,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}
