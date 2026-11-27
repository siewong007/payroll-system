//! Data access for the `user_totp_backup_codes` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::totp::TotpBackupCode;

pub async fn insert_many(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    code_hashes: &[String],
    lookup_hashes: &[String],
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO user_totp_backup_codes (user_id, code_hash, code_lookup_hash)
        SELECT $1, hash, lookup_hash
        FROM UNNEST($2::text[], $3::text[]) AS t(hash, lookup_hash)"#,
        user_id,
        code_hashes,
        lookup_hashes,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_unused_by_lookup(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    lookup_hash: &str,
) -> AppResult<Option<TotpBackupCode>> {
    let row = sqlx::query_as!(
        TotpBackupCode,
        "SELECT id, user_id, code_hash, code_lookup_hash, used_at, created_at
         FROM user_totp_backup_codes
         WHERE user_id = $1 AND code_lookup_hash = $2 AND used_at IS NULL
         LIMIT 1",
        user_id,
        lookup_hash,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

pub async fn find_unused_legacy(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<Vec<TotpBackupCode>> {
    let rows = sqlx::query_as!(
        TotpBackupCode,
        "SELECT id, user_id, code_hash, code_lookup_hash, used_at, created_at
         FROM user_totp_backup_codes
         WHERE user_id = $1 AND used_at IS NULL AND code_lookup_hash IS NULL",
        user_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Scans every unused backup code for a user regardless of lookup-hash key
/// (up to 10 rows per account), ensuring key rotation of `TOTP_ENCRYPTION_KEY`
/// never breaks existing backup codes.
pub async fn find_all_unused_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<Vec<TotpBackupCode>> {
    let rows = sqlx::query_as!(
        TotpBackupCode,
        "SELECT id, user_id, code_hash, code_lookup_hash, used_at, created_at
         FROM user_totp_backup_codes
         WHERE user_id = $1 AND used_at IS NULL",
        user_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn mark_used(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<bool> {
    let rows = sqlx::query!(
        "UPDATE user_totp_backup_codes SET used_at = NOW() WHERE id = $1 AND used_at IS NULL",
        id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows == 1)
}

pub async fn delete_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        "DELETE FROM user_totp_backup_codes WHERE user_id = $1",
        user_id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}
