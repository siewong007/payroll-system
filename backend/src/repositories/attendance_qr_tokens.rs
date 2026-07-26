//! Data access for the `attendance_qr_tokens` table.

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::attendance::AttendanceQrToken;

/// Revoke (mark used) the currently-active tokens issued by one display surface.
///
/// Scoped to `kiosk_credential_id` (NULL = the admin console) so that several
/// kiosks in one company do not revoke each other's still-displayed codes.
pub async fn revoke_unused_for_issuer(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    kiosk_credential_id: Option<Uuid>,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE attendance_qr_tokens SET used = TRUE
         WHERE company_id = $1 AND used = FALSE
           AND kiosk_credential_id IS NOT DISTINCT FROM $2",
        company_id,
        kiosk_credential_id,
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
    kiosk_credential_id: Option<Uuid>,
) -> AppResult<()> {
    sqlx::query!(
        "INSERT INTO attendance_qr_tokens (company_id, token, expires_at, kiosk_credential_id)
         VALUES ($1, $2, $3, $4)",
        company_id,
        token,
        expires_at,
        kiosk_credential_id,
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
