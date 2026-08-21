//! Data access for the `user_totp` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::totp::UserTotp;

/// Insert a new pending (unconfirmed) secret for a user, or replace an
/// existing one — either a fresh setup or a restart of an abandoned one.
/// Confirming a previous secret is never carried over.
pub async fn upsert_pending(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    secret_encrypted: &str,
) -> AppResult<UserTotp> {
    let row = sqlx::query_as!(
        UserTotp,
        r#"INSERT INTO user_totp (user_id, secret_encrypted, enabled, confirmed_at)
        VALUES ($1, $2, false, NULL)
        ON CONFLICT (user_id) DO UPDATE SET
            secret_encrypted = EXCLUDED.secret_encrypted,
            enabled = false,
            confirmed_at = NULL,
            updated_at = NOW()
        RETURNING *"#,
        user_id,
        secret_encrypted,
    )
    .fetch_one(executor)
    .await?;
    Ok(row)
}

pub async fn confirm(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE user_totp SET enabled = true, confirmed_at = NOW(), updated_at = NOW() WHERE user_id = $1",
        user_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_by_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<Option<UserTotp>> {
    let row = sqlx::query_as!(
        UserTotp,
        "SELECT * FROM user_totp WHERE user_id = $1",
        user_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

/// Every enrolment row, keyed for the startup re-encryption pass. The table
/// holds at most one row per 2FA user, so this is small by construction.
pub async fn list_all(
    executor: impl Executor<'_, Database = Postgres>,
) -> AppResult<Vec<UserTotp>> {
    let rows = sqlx::query_as!(UserTotp, "SELECT * FROM user_totp",)
        .fetch_all(executor)
        .await?;
    Ok(rows)
}

/// Replaces the stored ciphertext — used only by the startup re-encryption
/// pass when a legacy row has been successfully re-keyed.
pub async fn update_secret(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    secret_encrypted: &str,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE user_totp SET secret_encrypted = $2, updated_at = NOW() WHERE id = $1",
        id,
        secret_encrypted,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn delete_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<u64> {
    let rows = sqlx::query!("DELETE FROM user_totp WHERE user_id = $1", user_id,)
        .execute(executor)
        .await?
        .rows_affected();
    Ok(rows)
}
