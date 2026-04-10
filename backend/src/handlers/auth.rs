use axum::{
    extract::State,
    http::HeaderMap,
    response::IntoResponse,
    Json,
};

use crate::core::app_state::AppState;
use crate::core::auth::{create_token, AuthUser};
use crate::core::cookie;
use crate::core::error::{AppError, AppResult};
use crate::models::session::{ForgotPasswordRequest, ResetPasswordRequest};
use crate::models::user::{LoginRequest, LoginResponse, User, UserResponse};
use crate::models::user_company::{CompanySummary, SwitchCompanyRequest};
use crate::services::{auth_service, email_service, password_reset_service, session_service, user_service};

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let response = auth_service::login(
        &state.pool,
        req,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
    )
    .await?;

    let mut headers = HeaderMap::new();
    if let Some(ref refresh_token) = response.refresh_token {
        let (name, value) = cookie::set_refresh_cookie(refresh_token, &state.config.frontend_url);
        headers.insert(name, value.parse().unwrap());
    }

    // Don't include refresh_token in JSON body — it's in the cookie
    let body = LoginResponse {
        token: response.token,
        refresh_token: None,
        user: response.user,
    };

    Ok((headers, Json(body)))
}

pub async fn me(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<UserResponse>> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(auth.0.sub)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(UserResponse::from(user)))
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

/// Refresh an expired JWT using the refresh token from the httpOnly cookie.
pub async fn refresh_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let refresh = cookie::extract_refresh_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("No refresh token".into()))?;

    let user_id = session_service::verify_refresh_token(&state.pool, &refresh).await?;

    // Fetch user
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1 AND is_active = TRUE")
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::Unauthorized("User not found or inactive".into()))?;

    // Check if linked employee has been deleted
    if let Some(employee_id) = user.employee_id {
        let employee_active: Option<bool> = sqlx::query_scalar(
            "SELECT is_active FROM employees WHERE id = $1",
        )
        .bind(employee_id)
        .fetch_optional(&state.pool)
        .await?;

        if matches!(employee_active, Some(false) | None) {
            session_service::revoke_refresh_token(&state.pool, &refresh).await?;
            return Err(AppError::Unauthorized(
                "Your employee account has been deleted. Please contact your administrator.".into(),
            ));
        }
    }

    // Revoke old refresh token and issue new one (rotation)
    session_service::revoke_refresh_token(&state.pool, &refresh).await?;
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

    let mut resp_headers = HeaderMap::new();
    let (name, value) = cookie::set_refresh_cookie(&new_refresh, &state.config.frontend_url);
    resp_headers.insert(name, value.parse().unwrap());

    let body = LoginResponse {
        token,
        refresh_token: None,
        user: UserResponse::from(user),
    };

    Ok((resp_headers, Json(body)))
}

/// Logout: revoke the refresh token and clear the cookie.
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    if let Some(refresh) = cookie::extract_refresh_token(&headers) {
        let _ = session_service::revoke_refresh_token(&state.pool, &refresh).await;
    }

    let mut resp_headers = HeaderMap::new();
    let (name, value) = cookie::clear_refresh_cookie(&state.config.frontend_url);
    resp_headers.insert(name, value.parse().unwrap());

    Ok((resp_headers, Json(serde_json::json!({ "ok": true }))))
}

/// User requests a password reset. Sends reset link via email automatically.
pub async fn forgot_password(
    State(state): State<AppState>,
    Json(req): Json<ForgotPasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let result = password_reset_service::request_reset(&state.pool, &req.email).await?;

    // Send reset email if user exists (fire-and-forget, don't reveal whether email exists)
    if let Some((user_email, user_name, raw_token)) = result {
        let reset_url = format!("{}/reset-password?token={}", state.config.frontend_url, raw_token);
        let body_html = email_service::password_reset_html(&user_name, &reset_url);

        // Log but don't fail the request if email sending fails
        if let Err(e) = email_service::send_system_email(
            &state.config,
            &user_email,
            &user_name,
            "Reset your PayrollMY password",
            &body_html,
        )
        .await
        {
            tracing::error!("Failed to send password reset email to {}: {}", user_email, e);
        }
    }

    Ok(Json(serde_json::json!({
        "message": "If the email exists, a password reset link has been sent."
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
        .ok_or_else(|| AppError::BadRequest("Token is required".into()))?;
    password_reset_service::validate_reset_token(&state.pool, token).await?;
    Ok(Json(serde_json::json!({ "valid": true })))
}

#[derive(Debug, serde::Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

pub async fn change_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<ChangePasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    auth_service::validate_password_strength(&req.new_password)?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(auth.0.sub)
        .fetch_one(&state.pool)
        .await?;

    let valid = bcrypt::verify(&req.current_password, &user.password_hash)
        .map_err(|_| AppError::Internal("Password verification failed".into()))?;

    if !valid {
        return Err(AppError::BadRequest("Current password is incorrect".into()));
    }

    let new_hash = bcrypt::hash(&req.new_password, 10)
        .map_err(|_| AppError::Internal("Password hashing failed".into()))?;

    sqlx::query("UPDATE users SET password_hash = $1, must_change_password = FALSE, updated_at = NOW() WHERE id = $2")
        .bind(&new_hash)
        .bind(auth.0.sub)
        .execute(&state.pool)
        .await?;

    Ok(Json(serde_json::json!({
        "message": "Password changed successfully."
    })))
}
