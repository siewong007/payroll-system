use sqlx::PgPool;
use uuid::Uuid;

use crate::core::auth::create_token;
use crate::core::error::{AppError, AppResult};
use crate::models::session::LoginResponseWithRefresh;
use crate::models::user::{LoginRequest, LoginResponse, User, UserResponse};
use crate::repositories::{employees, users};
use crate::services::session_service;

const EMPLOYEE_DELETED_MSG: &str =
    "Your employee account has been deleted. Please contact your administrator.";

pub fn validate_password_strength(password: &str) -> AppResult<()> {
    if password.len() < 10 {
        return Err(AppError::Validation(
            "Password must be at least 10 characters".into(),
        ));
    }
    let has_upper = password.chars().any(|c| c.is_uppercase());
    let has_lower = password.chars().any(|c| c.is_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !has_upper || !has_lower || !has_digit {
        return Err(AppError::Validation(
            "Password must contain uppercase, lowercase, and a digit".into(),
        ));
    }
    Ok(())
}

/// True if the user's linked employee (if any) is still active. Users with no linked
/// employee are always considered active.
async fn linked_employee_active(pool: &PgPool, employee_id: Option<Uuid>) -> AppResult<bool> {
    match employee_id {
        Some(eid) => Ok(matches!(
            employees::get_active_status(pool, eid).await?,
            Some(true)
        )),
        None => Ok(true),
    }
}

pub async fn login(
    pool: &PgPool,
    req: LoginRequest,
    jwt_secret: &str,
    jwt_expiry: i64,
) -> AppResult<LoginResponse> {
    let user = users::find_active_by_email(pool, &req.email)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid email or password".into()))?;

    let valid = bcrypt::verify(&req.password, &user.password_hash)
        .map_err(|_| AppError::Internal("Password verification failed".into()))?;

    if !valid {
        return Err(AppError::Unauthorized("Invalid email or password".into()));
    }

    // Check if linked employee has been deleted
    if !linked_employee_active(pool, user.employee_id).await? {
        return Err(AppError::Unauthorized(EMPLOYEE_DELETED_MSG.into()));
    }

    // Update last login
    users::update_last_login(pool, user.id).await?;

    let token = create_token(
        user.id,
        &user.email,
        &user.roles,
        user.company_id,
        user.employee_id,
        jwt_secret,
        jwt_expiry,
    )?;

    let refresh_token = session_service::create_refresh_token(pool, user.id).await?;

    Ok(LoginResponse {
        token,
        refresh_token: Some(refresh_token),
        user: UserResponse::from(user),
    })
}

/// Verify a refresh token, rotate it, and mint a fresh JWT. Returns the new JWT, the
/// new raw refresh token (for the cookie), and the user.
pub async fn refresh_session(
    pool: &PgPool,
    raw_token: &str,
    jwt_secret: &str,
    jwt_expiry: i64,
) -> AppResult<LoginResponseWithRefresh> {
    let user_id = session_service::verify_refresh_token(pool, raw_token).await?;

    let user = users::get_active_by_id(pool, user_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("User not found or inactive".into()))?;

    // Check if linked employee has been deleted
    if !linked_employee_active(pool, user.employee_id).await? {
        session_service::revoke_refresh_token(pool, raw_token).await?;
        return Err(AppError::Unauthorized(EMPLOYEE_DELETED_MSG.into()));
    }

    // Revoke old refresh token and issue new one (rotation)
    session_service::revoke_refresh_token(pool, raw_token).await?;
    let new_refresh = session_service::create_refresh_token(pool, user.id).await?;

    let token = create_token(
        user.id,
        &user.email,
        &user.roles,
        user.company_id,
        user.employee_id,
        jwt_secret,
        jwt_expiry,
    )?;

    Ok(LoginResponseWithRefresh {
        token,
        refresh_token: new_refresh,
        user: UserResponse::from(user),
    })
}

/// Fetch a user by id. A missing row is a server inconsistency (the caller is already
/// authenticated), surfaced as a 500 to match the previous `fetch_one` behavior.
pub async fn get_user_by_id(pool: &PgPool, user_id: Uuid) -> AppResult<User> {
    users::get_by_id(pool, user_id)
        .await?
        .ok_or_else(|| AppError::Internal("User not found".into()))
}

pub async fn change_password(
    pool: &PgPool,
    user_id: Uuid,
    current_password: &str,
    new_password: &str,
) -> AppResult<()> {
    validate_password_strength(new_password)?;

    let user = get_user_by_id(pool, user_id).await?;

    let valid = bcrypt::verify(current_password, &user.password_hash)
        .map_err(|_| AppError::Internal("Password verification failed".into()))?;

    if !valid {
        return Err(AppError::BadRequest("Current password is incorrect".into()));
    }

    let new_hash = bcrypt::hash(new_password, 10)
        .map_err(|_| AppError::Internal("Password hashing failed".into()))?;

    users::update_password(pool, user_id, &new_hash).await
}

pub async fn skip_change_password(pool: &PgPool, user_id: Uuid) -> AppResult<()> {
    users::clear_must_change_password(pool, user_id).await
}
