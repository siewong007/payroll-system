//! Data access for the `oauth2_states` table (PKCE state + code-verifier storage).

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;

pub async fn delete_expired(executor: impl Executor<'_, Database = Postgres>) -> AppResult<()> {
    sqlx::query!("DELETE FROM oauth2_states WHERE expires_at < NOW()")
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    state: &str,
    code_verifier: &str,
    expires_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        "INSERT INTO oauth2_states (state, code_verifier, expires_at) VALUES ($1, $2, $3)",
        state,
        code_verifier,
        expires_at,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Atomically consume a non-expired state, returning its stored code verifier.
pub async fn consume(
    executor: impl Executor<'_, Database = Postgres>,
    state: &str,
) -> AppResult<Option<String>> {
    let code_verifier = sqlx::query_scalar!(
        "DELETE FROM oauth2_states WHERE state = $1 AND expires_at > NOW() RETURNING code_verifier",
        state,
    )
    .fetch_optional(executor)
    .await?;
    Ok(code_verifier)
}
