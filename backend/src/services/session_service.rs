use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::repositories::{refresh_tokens, user_sessions};

const REFRESH_TOKEN_DAYS: i64 = 30;

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Creates a new refresh token for a user, returns the raw token string.
pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    user_agent: Option<&str>,
) -> AppResult<(Uuid, String)> {
    let session_id = Uuid::now_v7();
    let raw_token = create_refresh_token(pool, user_id, session_id, user_agent).await?;
    Ok((session_id, raw_token))
}

pub async fn create_refresh_token(
    pool: &PgPool,
    user_id: Uuid,
    session_id: Uuid,
    user_agent: Option<&str>,
) -> AppResult<String> {
    let raw_token = format!("rt_{}_{}", Uuid::new_v4(), Uuid::new_v4());
    let token_hash = hash_token(&raw_token);
    let expires_at = Utc::now() + Duration::days(REFRESH_TOKEN_DAYS);

    user_sessions::insert(pool, session_id, user_id, user_agent, expires_at).await?;
    refresh_tokens::insert(pool, user_id, session_id, &token_hash, expires_at).await?;

    Ok(raw_token)
}

/// Validates a refresh token and returns the user_id if valid.
pub async fn verify_refresh_token(pool: &PgPool, raw_token: &str) -> AppResult<(Uuid, Uuid)> {
    let token_hash = hash_token(raw_token);

    refresh_tokens::find_active(pool, &token_hash)
        .await?
        .map(|token| (token.user_id, token.session_id))
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired refresh token".into()))
}

pub async fn rotate_refresh_token(
    pool: &PgPool,
    user_id: Uuid,
    session_id: Uuid,
    raw_token: &str,
) -> AppResult<String> {
    revoke_refresh_token(pool, raw_token).await?;
    let raw_token = format!("rt_{}_{}", Uuid::new_v4(), Uuid::new_v4());
    let token_hash = hash_token(&raw_token);
    let expires_at = Utc::now() + Duration::days(REFRESH_TOKEN_DAYS);
    refresh_tokens::insert(pool, user_id, session_id, &token_hash, expires_at).await?;
    user_sessions::touch(pool, session_id, expires_at).await?;
    Ok(raw_token)
}

pub async fn list_sessions(
    pool: &PgPool,
    user_id: Uuid,
) -> AppResult<Vec<crate::models::session::UserSession>> {
    user_sessions::list_active(pool, user_id).await
}

pub async fn revoke_session(pool: &PgPool, user_id: Uuid, session_id: Uuid) -> AppResult<bool> {
    if !user_sessions::revoke(pool, user_id, session_id).await? {
        return Ok(false);
    }
    refresh_tokens::revoke_for_session(pool, session_id).await?;
    Ok(true)
}

pub async fn revoke_other_sessions(
    pool: &PgPool,
    user_id: Uuid,
    current_session_id: Uuid,
) -> AppResult<u64> {
    let count = user_sessions::revoke_others(pool, user_id, current_session_id).await?;
    // The session state is the immediate JWT revocation source; stale refresh
    // rows are also revoked so they cannot be exchanged later.
    sqlx::query("UPDATE refresh_tokens SET revoked = TRUE WHERE session_id IN (SELECT id FROM user_sessions WHERE user_id = $1 AND id <> $2 AND revoked_at IS NOT NULL)")
        .bind(user_id).bind(current_session_id).execute(pool).await?;
    Ok(count)
}

/// Revokes a specific refresh token.
pub async fn revoke_refresh_token(pool: &PgPool, raw_token: &str) -> AppResult<()> {
    let token_hash = hash_token(raw_token);
    refresh_tokens::revoke_by_hash(pool, &token_hash).await
}
