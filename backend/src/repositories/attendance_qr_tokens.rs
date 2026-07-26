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

/// Purge tokens that expired more than `retain_days` ago and were never used
/// for a check-in (no attendance_records row references them). Referenced
/// tokens are kept as history. A kiosk mints ~288 rows/day, so without this
/// the table grows without bound. The NOT EXISTS probe is served by the
/// partial index on attendance_records(qr_token_id).
pub async fn purge_expired(
    executor: impl Executor<'_, Database = Postgres>,
    retain_days: i32,
) -> AppResult<u64> {
    let result = sqlx::query!(
        r#"DELETE FROM attendance_qr_tokens t
           WHERE t.expires_at < NOW() - ($1::int * INTERVAL '1 day')
             AND NOT EXISTS (
                 SELECT 1 FROM attendance_records ar WHERE ar.qr_token_id = t.id
             )"#,
        retain_days,
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

/// Whether this token was minted by a kiosk credential rather than the admin
/// console.
///
/// The attendance network learner treats a kiosk-minted token as corroboration:
/// the code was displayed by a device that is physically in the building and
/// holds a secret the employee does not, so it is not something an employee can
/// manufacture from home. A console-minted token proves nothing of the sort —
/// an administrator can generate one anywhere.
pub async fn is_kiosk_minted(
    executor: impl Executor<'_, Database = Postgres>,
    token_id: Uuid,
) -> AppResult<bool> {
    let minted = sqlx::query_scalar!(
        r#"SELECT (kiosk_credential_id IS NOT NULL) AS "minted!"
           FROM attendance_qr_tokens WHERE id = $1"#,
        token_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(minted.unwrap_or(false))
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
