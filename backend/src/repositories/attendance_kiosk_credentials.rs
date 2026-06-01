//! Data access for the `attendance_kiosk_credentials` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::attendance_kiosk::KioskCredential;

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    label: &str,
    token_hash: &str,
    token_prefix: &str,
    created_by: Uuid,
) -> AppResult<KioskCredential> {
    let row = sqlx::query_as!(
        KioskCredential,
        r#"INSERT INTO attendance_kiosk_credentials
            (company_id, label, token_hash, token_prefix, created_by)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, company_id, label, token_prefix, created_by,
                      created_at, last_used_at, last_used_ip, revoked_at"#,
        company_id,
        label,
        token_hash,
        token_prefix,
        created_by,
    )
    .fetch_one(executor)
    .await?;
    Ok(row)
}

pub async fn list_for_company(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<KioskCredential>> {
    let rows = sqlx::query_as!(
        KioskCredential,
        r#"SELECT id, company_id, label, token_prefix, created_by,
                  created_at, last_used_at, last_used_ip, revoked_at
           FROM attendance_kiosk_credentials
           WHERE company_id = $1
           ORDER BY revoked_at IS NOT NULL, created_at DESC"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn find_active_by_hash(
    executor: impl Executor<'_, Database = Postgres>,
    token_hash: &str,
) -> AppResult<Option<KioskCredential>> {
    let row = sqlx::query_as!(
        KioskCredential,
        r#"SELECT id, company_id, label, token_prefix, created_by,
                  created_at, last_used_at, last_used_ip, revoked_at
           FROM attendance_kiosk_credentials
           WHERE token_hash = $1 AND revoked_at IS NULL"#,
        token_hash,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

pub async fn mark_used(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    ip: Option<&str>,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE attendance_kiosk_credentials
         SET last_used_at = NOW(), last_used_ip = $2
         WHERE id = $1",
        id,
        ip,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn revoke(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<bool> {
    let result = sqlx::query!(
        "UPDATE attendance_kiosk_credentials
         SET revoked_at = NOW()
         WHERE id = $1 AND company_id = $2 AND revoked_at IS NULL",
        id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected() > 0)
}
