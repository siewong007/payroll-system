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
