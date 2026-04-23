use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct KioskCredential {
    pub id: Uuid,
    pub company_id: Uuid,
    pub label: String,
    pub token_prefix: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub last_used_ip: Option<String>,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub async fn insert_kiosk_credential(
    pool: &PgPool,
    company_id: Uuid,
    label: &str,
    token_hash: &str,
    token_prefix: &str,
    created_by: Uuid,
) -> AppResult<KioskCredential> {
    let row = sqlx::query_as::<_, KioskCredential>(
        r#"INSERT INTO attendance_kiosk_credentials
            (company_id, label, token_hash, token_prefix, created_by)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, company_id, label, token_prefix, created_by,
                      created_at, last_used_at, last_used_ip, revoked_at"#,
    )
    .bind(company_id)
    .bind(label)
    .bind(token_hash)
    .bind(token_prefix)
    .bind(created_by)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn list_kiosk_credentials(
    pool: &PgPool,
    company_id: Uuid,
) -> AppResult<Vec<KioskCredential>> {
    let rows = sqlx::query_as::<_, KioskCredential>(
        r#"SELECT id, company_id, label, token_prefix, created_by,
                  created_at, last_used_at, last_used_ip, revoked_at
           FROM attendance_kiosk_credentials
           WHERE company_id = $1
           ORDER BY revoked_at IS NOT NULL, created_at DESC"#,
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_active_by_hash(
    pool: &PgPool,
    token_hash: &str,
) -> AppResult<Option<KioskCredential>> {
    let row = sqlx::query_as::<_, KioskCredential>(
        r#"SELECT id, company_id, label, token_prefix, created_by,
                  created_at, last_used_at, last_used_ip, revoked_at
           FROM attendance_kiosk_credentials
           WHERE token_hash = $1 AND revoked_at IS NULL"#,
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn mark_used(pool: &PgPool, id: Uuid, ip: Option<&str>) -> AppResult<()> {
    sqlx::query(
        "UPDATE attendance_kiosk_credentials
         SET last_used_at = NOW(), last_used_ip = $2
         WHERE id = $1",
    )
    .bind(id)
    .bind(ip)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn revoke(pool: &PgPool, id: Uuid, company_id: Uuid) -> AppResult<bool> {
    let result = sqlx::query(
        "UPDATE attendance_kiosk_credentials
         SET revoked_at = NOW()
         WHERE id = $1 AND company_id = $2 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(company_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}
