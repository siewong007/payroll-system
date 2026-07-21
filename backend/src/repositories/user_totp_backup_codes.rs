//! Data access for the `user_totp_backup_codes` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::totp::TotpBackupCode;

pub async fn insert_many(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    code_hashes: &[String],
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO user_totp_backup_codes (user_id, code_hash)
        SELECT $1, hash FROM UNNEST($2::text[]) AS hash"#,
        user_id,
        code_hashes,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_unused(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<Vec<TotpBackupCode>> {
    let rows = sqlx::query_as!(
        TotpBackupCode,
        "SELECT * FROM user_totp_backup_codes WHERE user_id = $1 AND used_at IS NULL",
        user_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn mark_used(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE user_totp_backup_codes SET used_at = NOW() WHERE id = $1",
        id,
    )
    .execute(executor)
    .await?;
    Ok(())
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
