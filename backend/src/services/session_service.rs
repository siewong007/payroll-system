use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::AppResult;

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

    sqlx::query(
        r#"INSERT INTO refresh_tokens (user_id, token_hash, expires_at)
        VALUES ($1, $2, $3)"#,
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok(raw_token)
}

/// Validates a refresh token and returns the user_id if valid.
pub async fn verify_refresh_token(pool: &PgPool, raw_token: &str) -> AppResult<Uuid> {
    let token_hash = hash_token(raw_token);

    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"SELECT user_id FROM refresh_tokens
        WHERE token_hash = $1 AND revoked = FALSE AND expires_at > NOW()"#,
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?;

    row.map(|r| r.0)
        .ok_or_else(|| crate::core::error::AppError::Unauthorized("Invalid or expired refresh token".into()))
}

/// Revokes a specific refresh token.
pub async fn revoke_refresh_token(pool: &PgPool, raw_token: &str) -> AppResult<()> {
    let token_hash = hash_token(raw_token);
    sqlx::query("UPDATE refresh_tokens SET revoked = TRUE WHERE token_hash = $1")
        .bind(&token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

/// Revokes all refresh tokens for a user (e.g., on password change).
pub async fn revoke_all_user_tokens(pool: &PgPool, user_id: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE refresh_tokens SET revoked = TRUE WHERE user_id = $1 AND revoked = FALSE")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
