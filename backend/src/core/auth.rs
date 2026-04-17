use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::{AppError, AppResult};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid, // user ID
    pub email: String,
    pub role: String,
    pub company_id: Option<Uuid>,
    pub employee_id: Option<Uuid>,
    pub exp: i64,
    pub iat: i64,
}

pub fn create_token(
    user_id: Uuid,
    email: &str,
    role: &str,
    company_id: Option<Uuid>,
    employee_id: Option<Uuid>,
    secret: &str,
    expiry_hours: i64,
) -> AppResult<String> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id,
        email: email.to_string(),
        role: role.to_string(),
        company_id,
        employee_id,
        exp: (now + Duration::hours(expiry_hours)).timestamp(),
        iat: now.timestamp(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Failed to create token: {}", e)))
}

pub fn verify_token(token: &str, secret: &str) -> AppResult<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| AppError::Unauthorized(format!("Invalid token: {}", e)))
}

/// Extractor for authenticated user claims from JWT
#[derive(Debug, Clone)]
pub struct AuthUser(pub Claims);

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("Missing authorization header".to_string()))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::Unauthorized("Invalid authorization format".to_string()))?;

        let secret = parts
            .extensions
            .get::<JwtSecret>()
            .ok_or_else(|| AppError::Internal("JWT secret not configured".to_string()))?;

        let claims = verify_token(token, &secret.0)?;
        Ok(AuthUser(claims))
    }
}

impl AuthUser {
    /// Returns true if the user's role is 'exec'.
    pub fn is_exec(&self) -> bool {
        self.0.role == "exec"
    }

    /// Rejects the request if role is 'exec'. Use to guard payroll endpoints.
    pub fn deny_exec(&self) -> AppResult<()> {
        if self.is_exec() {
            return Err(AppError::Forbidden(
                "Payroll access not available for this role".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct JwtSecret(pub String);
