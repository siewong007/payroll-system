use axum::{extract::State, Json};

use crate::core::app_state::AppState;
use crate::core::auth::{create_token, AuthUser};
use crate::core::error::AppResult;
use crate::models::session::{ForgotPasswordRequest, RefreshTokenRequest, ResetPasswordRequest};
use crate::models::user::{LoginRequest, LoginResponse, User, UserResponse};
use crate::models::user_company::{CompanySummary, SwitchCompanyRequest};
use crate::services::{auth_service, password_reset_service, session_service, user_service};

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<LoginResponse>> {
    let response = auth_service::login(
        &state.pool,
        req,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
    )
    .await?;
    Ok(Json(response))
}

pub async fn me(auth: AuthUser) -> AppResult<Json<UserResponse>> {
    Ok(Json(UserResponse {
        id: auth.0.sub,
        email: auth.0.email,
        full_name: String::new(),
        role: auth.0.role,
        company_id: auth.0.company_id,
        employee_id: auth.0.employee_id,
    }))
}

pub async fn my_companies(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<CompanySummary>>> {
    let companies = user_service::get_user_companies(&state.pool, auth.0.sub).await?;
    Ok(Json(companies))
}

pub async fn switch_company(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<SwitchCompanyRequest>,
) -> AppResult<Json<LoginResponse>> {
    // Validate access and update active company
    user_service::switch_company(&state.pool, auth.0.sub, req.company_id).await?;

    // Fetch updated user
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(auth.0.sub)
        .fetch_one(&state.pool)
        .await?;

    // Issue new token with updated company_id
    let token = create_token(
        user.id,
        &user.email,
        &user.role,
        user.company_id,
        user.employee_id,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
    )?;

    Ok(Json(LoginResponse {
        token,
        refresh_token: None,
        user: UserResponse::from(user),
    }))
}

/// Refresh an expired JWT using a valid refresh token.
pub async fn refresh_token(
    State(state): State<AppState>,
    Json(req): Json<RefreshTokenRequest>,
) -> AppResult<Json<LoginResponse>> {
    let user_id = session_service::verify_refresh_token(&state.pool, &req.refresh_token).await?;

    // Fetch user
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1 AND is_active = TRUE")
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| crate::core::error::AppError::Unauthorized("User not found or inactive".into()))?;

    // Revoke old refresh token and issue new one (rotation)
    session_service::revoke_refresh_token(&state.pool, &req.refresh_token).await?;
    let new_refresh = session_service::create_refresh_token(&state.pool, user.id).await?;

    let token = create_token(
        user.id,
        &user.email,
        &user.role,
        user.company_id,
        user.employee_id,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
    )?;

    Ok(Json(LoginResponse {
        token,
        refresh_token: Some(new_refresh),
        user: UserResponse::from(user),
    }))
}

/// Logout: revoke the refresh token.
pub async fn logout(
    State(state): State<AppState>,
    Json(req): Json<RefreshTokenRequest>,
) -> AppResult<Json<serde_json::Value>> {
    session_service::revoke_refresh_token(&state.pool, &req.refresh_token).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// User requests a password reset.
pub async fn forgot_password(
    State(state): State<AppState>,
    Json(req): Json<ForgotPasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    password_reset_service::request_reset(&state.pool, &req.email).await?;
    Ok(Json(serde_json::json!({
        "message": "If the email exists, a reset request has been submitted for admin approval."
    })))
}

/// User resets password using an approved token.
pub async fn reset_password(
    State(state): State<AppState>,
    Json(req): Json<ResetPasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    password_reset_service::reset_password(&state.pool, &req.token, &req.new_password).await?;
    Ok(Json(serde_json::json!({
        "message": "Password has been reset successfully. Please log in with your new password."
    })))
}

/// Validate a reset token (used by the frontend to check if the link is still valid).
pub async fn validate_reset_token(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let token = req.get("token")
        .and_then(|t| t.as_str())
        .ok_or_else(|| crate::core::error::AppError::BadRequest("Token is required".into()))?;
    password_reset_service::validate_reset_token(&state.pool, token).await?;
    Ok(Json(serde_json::json!({ "valid": true })))
}
