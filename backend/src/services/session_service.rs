use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::repositories::refresh_tokens;

const REFRESH_TOKEN_DAYS: i64 = 30;

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Creates a new refresh token for a user, returns the raw token string.
pub async fn create_refresh_token(pool: &PgPool, user_id: Uuid) -> AppResult<String> {
    let raw_token = format!("rt_{}_{}", Uuid::new_v4(), Uuid::new_v4());
    let token_hash = hash_token(&raw_token);
    let expires_at = Utc::now() + Duration::days(REFRESH_TOKEN_DAYS);

    refresh_tokens::insert(pool, user_id, &token_hash, expires_at).await?;

    Ok(raw_token)
}

/// Validates a refresh token and returns the user_id if valid.
pub async fn verify_refresh_token(pool: &PgPool, raw_token: &str) -> AppResult<Uuid> {
    let token_hash = hash_token(raw_token);

    refresh_tokens::find_active_user_id(pool, &token_hash)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired refresh token".into()))
}

/// Revokes a specific refresh token.
pub async fn revoke_refresh_token(pool: &PgPool, raw_token: &str) -> AppResult<()> {
    let token_hash = hash_token(raw_token);
    refresh_tokens::revoke_by_hash(pool, &token_hash).await
}
