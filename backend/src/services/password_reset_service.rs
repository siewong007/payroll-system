use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::repositories::{password_reset_requests, refresh_tokens, users};

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
    let Some(contact) = users::find_active_contact_by_email(pool, email).await? else {
        return Ok(None);
    };

    // Expire any existing pending/approved requests for this user
    password_reset_requests::expire_pending_for_user(pool, contact.id).await?;

    // Generate reset token
    let raw_token = format!("rst_{}_{}", Uuid::new_v4(), Uuid::new_v4());
    let token_hash = hash_token(&raw_token);
    let expires_at = Utc::now() + Duration::hours(RESET_TOKEN_HOURS);

    password_reset_requests::insert_approved(pool, contact.id, &token_hash, expires_at).await?;

    Ok(Some((contact.email, contact.full_name, raw_token)))
}

/// Validates a reset token and returns the user_id.
pub async fn validate_reset_token(pool: &PgPool, raw_token: &str) -> AppResult<Uuid> {
    let token_hash = hash_token(raw_token);

    password_reset_requests::find_user_id_by_valid_token(pool, &token_hash)
        .await?
        .ok_or_else(|| AppError::BadRequest("Invalid or expired reset link".into()))
}

/// Resets the user's password using a valid reset token.
pub async fn reset_password(pool: &PgPool, raw_token: &str, new_password: &str) -> AppResult<()> {
    super::auth_service::validate_password_strength(new_password)?;

    let token_hash = hash_token(raw_token);

    // Validate token
    let request = password_reset_requests::find_by_valid_token(pool, &token_hash)
        .await?
        .ok_or_else(|| AppError::BadRequest("Invalid or expired reset link".into()))?;

    // Hash new password
    let password_hash = bcrypt::hash(new_password, 12)
        .map_err(|e| AppError::Internal(format!("Failed to hash password: {}", e)))?;

    // Update password, complete the request, and revoke active sessions
    users::set_password(pool, request.user_id, &password_hash).await?;
    password_reset_requests::mark_completed(pool, request.id).await?;
    refresh_tokens::revoke_all_for_user(pool, request.user_id).await?;

    Ok(())
}
