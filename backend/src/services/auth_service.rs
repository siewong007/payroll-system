use sqlx::PgPool;

use crate::core::auth::create_token;
use crate::core::error::{AppError, AppResult};
use crate::models::user::{LoginRequest, LoginResponse, User, UserResponse};
use crate::services::session_service;

pub async fn login(
    pool: &PgPool,
    req: LoginRequest,
    jwt_secret: &str,
    jwt_expiry: i64,
) -> AppResult<LoginResponse> {
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = $1 AND is_active = TRUE",
    )
    .bind(&req.email)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Invalid email or password".into()))?;

    let valid = bcrypt::verify(&req.password, &user.password_hash)
        .map_err(|_| AppError::Internal("Password verification failed".into()))?;

    if !valid {
        return Err(AppError::Unauthorized("Invalid email or password".into()));
    }

    // Update last login
    sqlx::query("UPDATE users SET last_login = NOW() WHERE id = $1")
        .bind(user.id)
        .execute(pool)
        .await?;

    let token = create_token(
        user.id,
        &user.email,
        &user.role,
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
