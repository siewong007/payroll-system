use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::session::PasswordResetRequest;

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

const RESET_TOKEN_HOURS: i64 = 1;

/// User requests a password reset. Generates a token immediately (no admin approval).
/// Returns Some((user_email, user_full_name, raw_token)) if the email exists, None otherwise.
pub async fn request_reset(
    pool: &PgPool,
    email: &str,
) -> AppResult<Option<(String, String, String)>> {
    // Find user by email
    let user: Option<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id, email, full_name FROM users WHERE email = $1 AND is_active = TRUE",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    let (user_id, user_email, user_name) = match user {
        Some(u) => u,
        None => return Ok(None),
    };

    // Expire any existing pending/approved requests for this user
    sqlx::query(
        "UPDATE password_reset_requests SET status = 'expired' WHERE user_id = $1 AND status IN ('pending', 'approved')",
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    // Generate reset token
    let raw_token = format!("rst_{}_{}", Uuid::new_v4(), Uuid::new_v4());
    let token_hash = hash_token(&raw_token);
    let expires_at = Utc::now() + Duration::hours(RESET_TOKEN_HOURS);

    sqlx::query(
        r#"INSERT INTO password_reset_requests (user_id, status, reset_token_hash, reset_token_expires_at)
        VALUES ($1, 'approved', $2, $3)"#,
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok(Some((user_email, user_name, raw_token)))
}

/// Validates a reset token and returns the user_id.
pub async fn validate_reset_token(pool: &PgPool, raw_token: &str) -> AppResult<Uuid> {
    let token_hash = hash_token(raw_token);

    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"SELECT user_id FROM password_reset_requests
        WHERE reset_token_hash = $1
            AND status = 'approved'
            AND reset_token_expires_at > NOW()"#,
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?;

    row.map(|r| r.0)
        .ok_or_else(|| AppError::BadRequest("Invalid or expired reset link".into()))
}

/// Resets the user's password using a valid reset token.
pub async fn reset_password(pool: &PgPool, raw_token: &str, new_password: &str) -> AppResult<()> {
    super::auth_service::validate_password_strength(new_password)?;

    let token_hash = hash_token(raw_token);

    // Validate token
    let request = sqlx::query_as::<_, PasswordResetRequest>(
        r#"SELECT * FROM password_reset_requests
        WHERE reset_token_hash = $1
            AND status = 'approved'
            AND reset_token_expires_at > NOW()"#,
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Invalid or expired reset link".into()))?;

    // Hash new password
    let password_hash = bcrypt::hash(new_password, 12)
        .map_err(|e| AppError::Internal(format!("Failed to hash password: {}", e)))?;

    // Update password
    sqlx::query("UPDATE users SET password_hash = $2, updated_at = NOW() WHERE id = $1")
        .bind(request.user_id)
        .bind(&password_hash)
        .execute(pool)
        .await?;

    // Mark request as completed
    sqlx::query(
        "UPDATE password_reset_requests SET status = 'completed', completed_at = NOW() WHERE id = $1",
    )
    .bind(request.id)
    .execute(pool)
    .await?;

    // Revoke all refresh tokens for this user
    sqlx::query("UPDATE refresh_tokens SET revoked = TRUE WHERE user_id = $1 AND revoked = FALSE")
        .bind(request.user_id)
        .execute(pool)
        .await?;

    Ok(())
}
