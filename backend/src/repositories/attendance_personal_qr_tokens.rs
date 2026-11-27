//! Data access for single-use personal rotating QR passes scanned by kiosks.

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::attendance::AttendancePersonalQrToken;

pub async fn revoke_active_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE attendance_personal_qr_tokens
         SET used = TRUE
         WHERE company_id = $1 AND employee_id = $2 AND used = FALSE",
        company_id,
        employee_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Uuid,
    user_id: Uuid,
    token_hash: &str,
    action: &str,
    device_id: Option<Uuid>,
    cookie_device_id: Option<Uuid>,
    session_id: Option<Uuid>,
    passkey_id: Option<Uuid>,
    user_agent: Option<&str>,
    expires_at: DateTime<Utc>,
) -> AppResult<AttendancePersonalQrToken> {
    let row = sqlx::query_as!(
        AttendancePersonalQrToken,
        r#"INSERT INTO attendance_personal_qr_tokens
           (company_id, employee_id, user_id, token_hash, action, device_id,
            cookie_device_id, session_id, passkey_id, user_agent, expires_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
           RETURNING id, company_id, employee_id, user_id, token_hash, action,
                     device_id, cookie_device_id, session_id, passkey_id, user_agent,
                     used, expires_at, created_at"#,
        company_id,
        employee_id,
        user_id,
        token_hash,
        action,
        device_id,
        cookie_device_id,
        session_id,
        passkey_id,
        user_agent,
        expires_at,
    )
    .fetch_one(executor)
    .await?;
    Ok(row)
}

/// Atomically claim a live personal QR token for `company_id`.
pub async fn consume_by_hash_for_company(
    executor: impl Executor<'_, Database = Postgres>,
    token_hash: &str,
    company_id: Uuid,
) -> AppResult<Option<AttendancePersonalQrToken>> {
    let row = sqlx::query_as!(
        AttendancePersonalQrToken,
        r#"UPDATE attendance_personal_qr_tokens
           SET used = TRUE
           WHERE token_hash = $1
             AND company_id = $2
             AND used = FALSE
             AND expires_at >= NOW()
           RETURNING id, company_id, employee_id, user_id, token_hash, action,
                     device_id, cookie_device_id, session_id, passkey_id, user_agent,
                     used, expires_at, created_at"#,
        token_hash,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

pub async fn purge_expired(executor: impl Executor<'_, Database = Postgres>) -> AppResult<u64> {
    let res = sqlx::query!(
        "DELETE FROM attendance_personal_qr_tokens
         WHERE expires_at < NOW() - INTERVAL '1 day'"
    )
    .execute(executor)
    .await?;
    Ok(res.rows_affected())
}
