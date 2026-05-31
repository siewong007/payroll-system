//! Data access for the `passkey_challenges` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

/// A consumed authentication challenge: the WebAuthn state plus the (nullable) user it targets.
pub struct ConsumedChallenge {
    pub user_id: Option<Uuid>,
    pub state_json: serde_json::Value,
}

pub async fn delete_expired(executor: impl Executor<'_, Database = Postgres>) -> AppResult<()> {
    sqlx::query!("DELETE FROM passkey_challenges WHERE expires_at < NOW()")
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Option<Uuid>,
    email: Option<&str>,
    challenge_type: &str,
    state_json: &serde_json::Value,
) -> AppResult<Uuid> {
    let id = sqlx::query_scalar!(
        r#"INSERT INTO passkey_challenges (user_id, email, challenge_type, state_json)
        VALUES ($1, $2, $3, $4) RETURNING id"#,
        user_id,
        email,
        challenge_type,
        state_json,
    )
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// Consume a challenge of a given type, returning its WebAuthn state.
pub async fn consume(
    executor: impl Executor<'_, Database = Postgres>,
    challenge_id: Uuid,
    challenge_type: &str,
) -> AppResult<Option<serde_json::Value>> {
    let state_json = sqlx::query_scalar!(
        r#"DELETE FROM passkey_challenges
        WHERE id = $1 AND challenge_type = $2 AND expires_at > NOW()
        RETURNING state_json"#,
        challenge_id,
        challenge_type,
    )
    .fetch_optional(executor)
    .await?;
    Ok(state_json)
}

/// Consume an `authentication` challenge, returning the targeted user and state.
pub async fn consume_authentication(
    executor: impl Executor<'_, Database = Postgres>,
    challenge_id: Uuid,
) -> AppResult<Option<ConsumedChallenge>> {
    let row = sqlx::query_as!(
        ConsumedChallenge,
        r#"DELETE FROM passkey_challenges
        WHERE id = $1 AND challenge_type = 'authentication' AND expires_at > NOW()
        RETURNING user_id, state_json"#,
        challenge_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

/// Consume a `discoverable` challenge, returning its WebAuthn state.
pub async fn consume_discoverable(
    executor: impl Executor<'_, Database = Postgres>,
    challenge_id: Uuid,
) -> AppResult<Option<serde_json::Value>> {
    let state_json = sqlx::query_scalar!(
        r#"DELETE FROM passkey_challenges
        WHERE id = $1 AND challenge_type = 'discoverable' AND expires_at > NOW()
        RETURNING state_json"#,
        challenge_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(state_json)
}
