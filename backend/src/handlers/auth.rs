use axum::{Json, extract::State, http::HeaderMap, response::IntoResponse};

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, create_token};
use crate::core::cookie;
use crate::core::error::{AppError, AppResult};
use crate::core::extract::ValidatedJson;
use crate::models::session::{ForgotPasswordRequest, ResetPasswordRequest};
use crate::models::user::{ChangePasswordRequest, LoginRequest, LoginResponse, UserResponse};
use crate::models::user_company::{CompanySummary, SwitchCompanyRequest};
use crate::services::{
    auth_service, email_service, password_reset_service, session_service, user_service,
};

pub async fn login(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<LoginRequest>,
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

pub async fn me(State(state): State<AppState>, auth: AuthUser) -> AppResult<Json<UserResponse>> {
    let user = auth_service::get_user_by_id(&state.pool, auth.0.sub).await?;
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
    let user = auth_service::get_user_by_id(&state.pool, auth.0.sub).await?;

    // Issue new token with updated company_id
    let token = create_token(
        user.id,
        &user.email,
        &user.roles,
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

    let refreshed = auth_service::refresh_session(
        &state.pool,
        &refresh,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
    )
    .await?;

    let mut resp_headers = HeaderMap::new();
    let (name, value) =
        cookie::set_refresh_cookie(&refreshed.refresh_token, &state.config.frontend_url);
    resp_headers.insert(name, value.parse().unwrap());

    let body = LoginResponse {
        token: refreshed.token,
        refresh_token: None,
        user: refreshed.user,
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
    ValidatedJson(req): ValidatedJson<ForgotPasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let result = password_reset_service::request_reset(&state.pool, &req.email).await?;

    // Send reset email if user exists (fire-and-forget, don't reveal whether email exists)
    if let Some((user_email, user_name, raw_token)) = result {
        let reset_url = format!(
            "{}/reset-password?token={}",
            state.config.frontend_url, raw_token
        );
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
            tracing::error!(
                "Failed to send password reset email to {}: {}",
                user_email,
                e
            );
        }
    }

    Ok(Json(serde_json::json!({
        "message": "If the email exists, a password reset link has been sent."
    })))
}

/// User resets password using an approved token.
pub async fn reset_password(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<ResetPasswordRequest>,
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
    let token = req
        .get("token")
        .and_then(|t| t.as_str())
        .ok_or_else(|| AppError::BadRequest("Token is required".into()))?;
    password_reset_service::validate_reset_token(&state.pool, token).await?;
    Ok(Json(serde_json::json!({ "valid": true })))
}

pub async fn change_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<ChangePasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    auth_service::change_password(
        &state.pool,
        auth.0.sub,
        &req.current_password,
        &req.new_password,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "message": "Password changed successfully."
    })))
}

pub async fn skip_change_password(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    auth_service::skip_change_password(&state.pool, auth.0.sub).await?;

    Ok(Json(serde_json::json!({
        "message": "Password change skipped."
    })))
}
