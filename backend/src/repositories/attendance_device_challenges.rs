//! Data access for short-lived WebCrypto attendance device challenges.

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    user_id: Uuid,
    employee_id: Uuid,
    nonce: &str,
    purpose: &str,
    expires_at: DateTime<Utc>,
) -> AppResult<Uuid> {
    let id = sqlx::query_scalar!(
        r#"INSERT INTO attendance_device_challenges
           (company_id, user_id, employee_id, nonce, purpose, expires_at)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id"#,
        company_id,
        user_id,
        employee_id,
        nonce,
        purpose,
        expires_at,
    )
    .fetch_one(executor)
    .await?;
    Ok(id)
}

pub async fn consume_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    challenge_id: Uuid,
    company_id: Uuid,
    user_id: Uuid,
    purpose: &str,
) -> AppResult<Option<String>> {
    let nonce = sqlx::query_scalar!(
        r#"DELETE FROM attendance_device_challenges
           WHERE id = $1
             AND company_id = $2
             AND user_id = $3
             AND purpose = $4
             AND expires_at >= NOW()
           RETURNING nonce"#,
        challenge_id,
        company_id,
        user_id,
        purpose,
    )
    .fetch_optional(executor)
    .await?;
    Ok(nonce)
}

pub async fn purge_expired(executor: impl Executor<'_, Database = Postgres>) -> AppResult<u64> {
    let res = sqlx::query!("DELETE FROM attendance_device_challenges WHERE expires_at < NOW()")
        .execute(executor)
        .await?;
    Ok(res.rows_affected())
}
