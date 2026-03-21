use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::session::{PasswordResetRequest, PasswordResetWithUser};

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

const RESET_TOKEN_HOURS: i64 = 24;

/// User requests a password reset. Creates a pending request.
pub async fn request_reset(pool: &PgPool, email: &str) -> AppResult<()> {
    // Find user by email
    let user: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM users WHERE email = $1 AND is_active = TRUE")
            .bind(email)
            .fetch_optional(pool)
            .await?;

    let user_id = match user {
        Some((id,)) => id,
        None => {
            // Don't reveal whether the email exists
            return Ok(());
        }
    };

    // Check if there's already a pending request
    let pending: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM password_reset_requests WHERE user_id = $1 AND status = 'pending'",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    if pending.is_some() {
        // Already has a pending request, silently succeed
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO password_reset_requests (user_id, status) VALUES ($1, 'pending')",
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// List password reset requests (for admin review).
pub async fn list_requests(pool: &PgPool) -> AppResult<Vec<PasswordResetWithUser>> {
    let requests = sqlx::query_as::<_, PasswordResetWithUser>(
        r#"SELECT r.id, r.user_id, r.status, r.requested_at,
            r.reviewed_by, r.reviewed_at, r.completed_at,
            u.email as user_email, u.full_name as user_full_name, u.role as user_role
        FROM password_reset_requests r
        JOIN users u ON r.user_id = u.id
        ORDER BY
            CASE r.status WHEN 'pending' THEN 0 ELSE 1 END,
            r.requested_at DESC"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(requests)
}

/// Admin approves a reset request. Generates a reset token and returns it.
pub async fn approve_request(
    pool: &PgPool,
    request_id: Uuid,
    reviewer_id: Uuid,
) -> AppResult<(PasswordResetRequest, String)> {
    // Fetch the request
    let request = sqlx::query_as::<_, PasswordResetRequest>(
        "SELECT * FROM password_reset_requests WHERE id = $1",
    )
    .bind(request_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Reset request not found".into()))?;

    if request.status != "pending" {
        return Err(AppError::BadRequest(
            format!("Request is already {}", request.status),
        ));
    }

    // Generate reset token
    let raw_token = format!("rst_{}_{}", Uuid::new_v4(), Uuid::new_v4());
    let token_hash = hash_token(&raw_token);
    let expires_at = Utc::now() + Duration::hours(RESET_TOKEN_HOURS);

    let updated = sqlx::query_as::<_, PasswordResetRequest>(
        r#"UPDATE password_reset_requests SET
            status = 'approved',
            reviewed_by = $2,
            reviewed_at = NOW(),
            reset_token_hash = $3,
            reset_token_expires_at = $4
        WHERE id = $1
        RETURNING *"#,
    )
    .bind(request_id)
    .bind(reviewer_id)
    .bind(&token_hash)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;

    Ok((updated, raw_token))
}

/// Admin rejects a reset request.
pub async fn reject_request(
    pool: &PgPool,
    request_id: Uuid,
    reviewer_id: Uuid,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"UPDATE password_reset_requests SET
            status = 'rejected',
            reviewed_by = $2,
            reviewed_at = NOW()
        WHERE id = $1 AND status = 'pending'"#,
    )
    .bind(request_id)
    .bind(reviewer_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::BadRequest(
            "Request not found or not pending".into(),
        ));
    }
    Ok(())
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

/// Count pending reset requests (for admin badge).
pub async fn count_pending(pool: &PgPool) -> AppResult<i64> {
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM password_reset_requests WHERE status = 'pending'")
            .fetch_one(pool)
            .await?;
    Ok(row.0)
}
