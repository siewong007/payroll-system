//! Data access for the `passkey_credentials` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::passkey::PasskeyCredential;

/// Credentials for a user, ordered by creation (for display).
pub async fn list_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<Vec<PasskeyCredential>> {
    let rows = sqlx::query_as!(
        PasskeyCredential,
        "SELECT * FROM passkey_credentials WHERE user_id = $1 ORDER BY created_at",
        user_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Credentials for a user, unordered (for WebAuthn verification).
pub async fn all_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<Vec<PasskeyCredential>> {
    let rows = sqlx::query_as!(
        PasskeyCredential,
        "SELECT * FROM passkey_credentials WHERE user_id = $1",
        user_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    credential_name: &str,
    credential_json: &serde_json::Value,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO passkey_credentials (user_id, credential_name, credential_json)
        VALUES ($1, $2, $3)"#,
        user_id,
        credential_name,
        credential_json,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Lock a single passkey row owned by `user_id` for atomic counter progression.
pub async fn get_by_id_for_update(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    user_id: Uuid,
) -> AppResult<Option<PasskeyCredential>> {
    let row = sqlx::query_as!(
        PasskeyCredential,
        r#"SELECT id, user_id, credential_name, credential_json, created_at, last_used_at
        FROM passkey_credentials
        WHERE id = $1 AND user_id = $2
        FOR UPDATE"#,
        id,
        user_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

/// Persist an updated credential (e.g. signature counter) matched by primary key `id`.
pub async fn update_credential_json_by_id(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    user_id: Uuid,
    credential_json: &serde_json::Value,
) -> AppResult<u64> {
    let result = sqlx::query!(
        r#"UPDATE passkey_credentials
        SET credential_json = $3, last_used_at = NOW()
        WHERE id = $1 AND user_id = $2
        "#,
        id,
        user_id,
        credential_json,
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

pub async fn rename(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    user_id: Uuid,
    name: &str,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        "UPDATE passkey_credentials SET credential_name = $3 WHERE id = $1 AND user_id = $2",
        id,
        user_id,
        name,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}

pub async fn delete(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    user_id: Uuid,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        "DELETE FROM passkey_credentials WHERE id = $1 AND user_id = $2",
        id,
        user_id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}
