//! Data access for the `attendance_qr_tokens` table.

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::attendance::AttendanceQrToken;

/// Revoke (mark used) all currently-active tokens for a company.
pub async fn revoke_unused(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE attendance_qr_tokens SET used = TRUE
         WHERE company_id = $1 AND used = FALSE",
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    token: &str,
    expires_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        "INSERT INTO attendance_qr_tokens (company_id, token, expires_at)
         VALUES ($1, $2, $3)",
        company_id,
        token,
        expires_at,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_by_token(
    executor: impl Executor<'_, Database = Postgres>,
    token: &str,
) -> AppResult<Option<AttendanceQrToken>> {
    let row = sqlx::query_as!(
        AttendanceQrToken,
        "SELECT id, company_id, token, expires_at, used, created_at
         FROM attendance_qr_tokens WHERE token = $1",
        token,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}
