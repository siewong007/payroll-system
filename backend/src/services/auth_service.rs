use sqlx::PgPool;

use crate::core::auth::create_token_with_roles;
use crate::core::error::{AppError, AppResult};
use crate::models::user::{LoginRequest, LoginResponse, User, UserResponse};
use crate::services::session_service;

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

pub async fn login(
    pool: &PgPool,
    req: LoginRequest,
    jwt_secret: &str,
    jwt_expiry: i64,
) -> AppResult<LoginResponse> {
    let user =
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1 AND is_active = TRUE")
            .bind(&req.email)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::Unauthorized("Invalid email or password".into()))?;

    let valid = bcrypt::verify(&req.password, &user.password_hash)
        .map_err(|_| AppError::Internal("Password verification failed".into()))?;

    if !valid {
        return Err(AppError::Unauthorized("Invalid email or password".into()));
    }

    // Check if linked employee has been deleted
    if let Some(employee_id) = user.employee_id {
        let employee_active: Option<bool> =
            sqlx::query_scalar("SELECT is_active FROM employees WHERE id = $1")
                .bind(employee_id)
                .fetch_optional(pool)
                .await?;

        match employee_active {
            Some(false) | None => {
                return Err(AppError::Unauthorized(
                    "Your employee account has been deleted. Please contact your administrator."
                        .into(),
                ));
            }
            _ => {}
        }
    }

    // Update last login
    sqlx::query("UPDATE users SET last_login = NOW() WHERE id = $1")
        .bind(user.id)
        .execute(pool)
        .await?;

    let token = create_token_with_roles(
        user.id,
        &user.email,
        &user.role,
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
