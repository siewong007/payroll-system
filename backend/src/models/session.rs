use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Refresh Token ───

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RefreshToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub revoked: bool,
    pub created_at: DateTime<Utc>,
}

// ─── Password Reset ───

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PasswordResetRequest {
    pub id: Uuid,
    pub user_id: Uuid,
    pub status: String,
    pub requested_at: DateTime<Utc>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub reset_token_hash: Option<String>,
    pub reset_token_expires_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

// ─── Request/Response DTOs ───

#[derive(Debug, Serialize)]
pub struct LoginResponseWithRefresh {
    pub token: String,
    pub refresh_token: String,
    pub user: super::user::UserResponse,
}

#[derive(Debug, Deserialize, validator::Validate)]
pub struct ForgotPasswordRequest {
    #[validate(email(message = "must be a valid email address"))]
    pub email: String,
}

#[derive(Debug, Deserialize, validator::Validate)]
pub struct ResetPasswordRequest {
    #[validate(length(min = 1, message = "token is required"))]
    pub token: String,
    #[validate(length(min = 8, message = "must be at least 8 characters"))]
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}
