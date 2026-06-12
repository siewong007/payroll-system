//! Data access for the `oauth2_accounts` table.

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::oauth2::{LinkedAccount, OAuth2Account};

pub async fn find_by_provider_id(
    executor: impl Executor<'_, Database = Postgres>,
    provider: &str,
    provider_user_id: &str,
) -> AppResult<Option<OAuth2Account>> {
    let account = sqlx::query_as!(
        OAuth2Account,
        "SELECT * FROM oauth2_accounts WHERE provider = $1 AND provider_user_id = $2",
        provider,
        provider_user_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(account)
}

/// Insert or update (by provider identity) an OAuth2 account link, returning the row.
#[allow(clippy::too_many_arguments)]
pub async fn upsert(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    provider: &str,
    provider_user_id: &str,
    provider_email: Option<&str>,
    provider_name: Option<&str>,
    avatar_url: Option<&str>,
    access_token_hash: Option<&str>,
    refresh_token_hash: Option<&str>,
    token_expires_at: Option<DateTime<Utc>>,
) -> AppResult<OAuth2Account> {
    let account = sqlx::query_as!(
        OAuth2Account,
        r#"INSERT INTO oauth2_accounts (user_id, provider, provider_user_id, provider_email, provider_name, avatar_url, access_token_hash, refresh_token_hash, token_expires_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (provider, provider_user_id) DO UPDATE SET
            provider_email = EXCLUDED.provider_email,
            provider_name = EXCLUDED.provider_name,
            avatar_url = EXCLUDED.avatar_url,
            access_token_hash = COALESCE(EXCLUDED.access_token_hash, oauth2_accounts.access_token_hash),
            refresh_token_hash = COALESCE(EXCLUDED.refresh_token_hash, oauth2_accounts.refresh_token_hash),
            token_expires_at = COALESCE(EXCLUDED.token_expires_at, oauth2_accounts.token_expires_at),
            updated_at = NOW()
        RETURNING *"#,
        user_id,
        provider,
        provider_user_id,
        provider_email,
        provider_name,
        avatar_url,
        access_token_hash,
        refresh_token_hash,
        token_expires_at,
    )
    .fetch_one(executor)
    .await?;
    Ok(account)
}

pub async fn update_tokens(
    executor: impl Executor<'_, Database = Postgres>,
    access_token_hash: &str,
    refresh_token_hash: Option<&str>,
    token_expires_at: Option<DateTime<Utc>>,
    provider: &str,
    provider_user_id: &str,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE oauth2_accounts SET
            access_token_hash = $1,
            refresh_token_hash = COALESCE($2, refresh_token_hash),
            token_expires_at = COALESCE($3, token_expires_at),
            updated_at = NOW()
        WHERE provider = $4 AND provider_user_id = $5"#,
        access_token_hash,
        refresh_token_hash,
        token_expires_at,
        provider,
        provider_user_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn delete_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    provider: &str,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        "DELETE FROM oauth2_accounts WHERE user_id = $1 AND provider = $2",
        user_id,
        provider,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}

pub async fn list_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<Vec<LinkedAccount>> {
    let accounts = sqlx::query_as!(
        LinkedAccount,
        r#"SELECT id, provider, provider_email, provider_name, avatar_url, created_at AS linked_at
        FROM oauth2_accounts WHERE user_id = $1
        ORDER BY created_at"#,
        user_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(accounts)
}
