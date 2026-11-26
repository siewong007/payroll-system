use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, create_token_with_exp, verify_token};
use crate::core::cookie;
use crate::core::error::{AppError, AppResult};
use crate::core::extract::ValidatedJson;
use crate::models::audit::AuditRequestMeta;
use crate::models::session::{
    ForgotPasswordRequest, LoginOutcome, ResetPasswordRequest, UserSessionResponse,
};
use crate::models::user::{
    ChangePasswordRequest, CodeLoginRequest, LoginRequest, LoginResponse, UserResponse,
};
use crate::models::user_company::{CompanySummary, SwitchCompanyRequest};
use crate::services::{
    auth_service, email_service, password_reset_service, session_service, user_service,
};

/// Turns a `LoginOutcome` into its HTTP response: a full session (JWT body +
/// httpOnly refresh cookie) when 2FA isn't required, or a small JSON marker
/// carrying the MFA-pending token when it is. Shared by every primary-auth
/// handler (password, passkey) so the response shape stays identical.
pub fn login_outcome_response(outcome: LoginOutcome, frontend_url: &str) -> Response {
    match outcome {
        LoginOutcome::Session(session) => {
            let mut headers = HeaderMap::new();
            let (name, value) = cookie::set_refresh_cookie(&session.refresh_token, frontend_url);
            headers.insert(name, value.parse().unwrap());

            // Don't include refresh_token in JSON body — it's in the cookie
            let body = LoginResponse {
                token: session.token,
                refresh_token: None,
                user: session.user,
            };
            (
                headers,
                Json(serde_json::to_value(body).unwrap_or_default()),
            )
                .into_response()
        }
        LoginOutcome::MfaRequired { mfa_token } => (
            HeaderMap::new(),
            Json(serde_json::json!({ "requires_2fa": true, "mfa_token": mfa_token })),
        )
            .into_response(),
    }
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    audit_meta: AuditRequestMeta,
    ValidatedJson(req): ValidatedJson<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    crate::core::turnstile::verify(
        &state.config,
        req.turnstile_token.as_deref(),
        audit_meta.ip_address.as_deref(),
    )
    .await?;

    let outcome = auth_service::login(
        &state.pool,
        req,
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
        Some(&audit_meta),
    )
    .await?;

    Ok(login_outcome_response(outcome, &state.config.frontend_url))
}

/// Passwordless sign-in: email + a TOTP authenticator code or one-time backup
/// code. Turnstile applies — a 6-digit secret is exactly what bots guess.
pub async fn code_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    audit_meta: AuditRequestMeta,
    ValidatedJson(req): ValidatedJson<CodeLoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    crate::core::turnstile::verify(
        &state.config,
        req.turnstile_token.as_deref(),
        audit_meta.ip_address.as_deref(),
    )
    .await?;

    let outcome = auth_service::code_login(
        &state.pool,
        req,
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
        &state.config.totp_encryption_key,
        Some(&audit_meta),
    )
    .await?;

    Ok(login_outcome_response(outcome, &state.config.frontend_url))
}

pub async fn me(State(state): State<AppState>, auth: AuthUser) -> AppResult<Json<UserResponse>> {
    let user = auth_service::get_user_by_id(&state.pool, auth.0.sub).await?;
    Ok(Json(
        auth_service::build_user_response(&state.pool, user).await?,
    ))
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
    let _ = crate::repositories::push_subscriptions::update_company_for_session(
        &state.pool,
        auth.0.sub,
        auth.0.sid,
        req.company_id,
    )
    .await;

    // Fetch updated user and re-check 2FA enforcement in case company group
    // permissions elevated the user into a 2FA-required tier (M5), while
    // preserving the original token expiration (H1).
    let user = auth_service::get_user_by_id(&state.pool, auth.0.sub).await?;
    auth_service::enforce_two_factor_policy(&state.pool, &user).await?;

    let token = create_token_with_exp(
        user.id,
        &user.email,
        &user.roles,
        user.company_id,
        user.employee_id,
        auth.0.sid,
        &state.config.jwt_secret,
        auth.0.exp,
    )?;

    Ok(Json(LoginResponse {
        token,
        refresh_token: None,
        user: auth_service::build_user_response(&state.pool, user).await?,
    }))
}

/// Refresh an expired JWT using the refresh token from the httpOnly cookie.
pub async fn refresh_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    audit_meta: AuditRequestMeta,
) -> Result<impl IntoResponse, AppError> {
    let refresh = cookie::extract_refresh_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("No refresh token".into()))?;

    let ip_address = audit_meta.ip_address.as_deref();
    let refreshed = auth_service::refresh_session(
        &state.pool,
        &refresh,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
        ip_address,
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
    audit_meta: AuditRequestMeta,
) -> Result<impl IntoResponse, AppError> {
    let mut revoked = false;
    if let Some(refresh) = cookie::extract_refresh_token(&headers) {
        if let Ok((user_id, session_id)) =
            session_service::verify_refresh_token(&state.pool, &refresh).await
        {
            let _ = session_service::revoke_session(&state.pool, user_id, session_id).await;
            revoked = true;
            // Best-effort, after the revocation: the logout itself must not
            // fail because its trail row could not be written, but the row
            // still answers "when did this account sign out".
            if let Ok(user) = auth_service::get_user_by_id(&state.pool, user_id).await {
                let _ = crate::services::audit_service::log_action_with_metadata(
                    &state.pool,
                    user.company_id,
                    Some(user_id),
                    "logout",
                    "auth",
                    Some(user_id),
                    None::<serde_json::Value>,
                    Some(serde_json::json!({ "session_id": session_id })),
                    Some("Logout"),
                    Some(&audit_meta),
                )
                .await;
            }
        } else {
            let _ = session_service::revoke_refresh_token(&state.pool, &refresh).await;
        }
    }

    // Fallback when refresh cookie is missing or was already rotated by another
    // tab: revoke the Bearer JWT's session directly so logout never leaves an
    // active session usable until token expiry.
    if !revoked
        && let Some(token) = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
        && let Ok(claims) = verify_token(token, &state.config.jwt_secret)
    {
        let _ = session_service::revoke_session(&state.pool, claims.sub, claims.sid).await;
        let _ = crate::services::audit_service::log_action_with_metadata(
            &state.pool,
            claims.company_id,
            Some(claims.sub),
            "logout",
            "auth",
            Some(claims.sub),
            None::<serde_json::Value>,
            Some(serde_json::json!({ "session_id": claims.sid })),
            Some("Logout"),
            Some(&audit_meta),
        )
        .await;
    }

    let mut resp_headers = HeaderMap::new();
    let (name, value) = cookie::clear_refresh_cookie(&state.config.frontend_url);
    resp_headers.insert(name, value.parse().unwrap());

    Ok((resp_headers, Json(serde_json::json!({ "ok": true }))))
}

pub async fn list_sessions(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<UserSessionResponse>>> {
    let sessions = session_service::list_sessions(&state.pool, auth.0.sub).await?;
    Ok(Json(
        sessions
            .into_iter()
            .map(|session| UserSessionResponse::from_session(session, auth.0.sid))
            .collect(),
    ))
}

pub async fn revoke_session(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(session_id): Path<uuid::Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    if session_id == auth.0.sid {
        return Err(AppError::BadRequest(
            "Use logout to sign out this device".into(),
        ));
    }
    if !session_service::revoke_session(&state.pool, auth.0.sub, session_id).await? {
        return Err(AppError::NotFound("Session not found".into()));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn revoke_other_sessions(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let revoked =
        session_service::revoke_other_sessions(&state.pool, auth.0.sub, auth.0.sid).await?;
    Ok(Json(serde_json::json!({ "ok": true, "revoked": revoked })))
}

/// User requests a password reset. Sends reset link via email automatically.
pub async fn forgot_password(
    State(state): State<AppState>,
    audit_meta: AuditRequestMeta,
    ValidatedJson(req): ValidatedJson<ForgotPasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    crate::core::turnstile::verify(
        &state.config,
        req.turnstile_token.as_deref(),
        audit_meta.ip_address.as_deref(),
    )
    .await?;

    let result =
        password_reset_service::request_reset(&state.pool, &req.email, Some(&audit_meta)).await?;

    // Send reset email asynchronously if user exists — never block the HTTP
    // response on SMTP or expose account existence through response timing.
    if let Some((user_email, user_name, raw_token)) = result {
        let reset_url = format!(
            "{}/reset-password#token={}",
            state.config.frontend_url, raw_token
        );
        let body_html = email_service::password_reset_html(&user_name, &reset_url);
        let mailer = state.mailer.clone();
        tokio::spawn(async move {
            email_service::send_system_email_with_retry(
                mailer,
                user_email,
                user_name,
                "Reset your PayrollMY password".to_string(),
                body_html,
            )
            .await;
        });
    }

    Ok(Json(serde_json::json!({
        "message": "If the email exists, a password reset link has been sent."
    })))
}

/// User resets password using an approved token.
pub async fn reset_password(
    State(state): State<AppState>,
    audit_meta: AuditRequestMeta,
    ValidatedJson(req): ValidatedJson<ResetPasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    crate::core::turnstile::verify(
        &state.config,
        req.turnstile_token.as_deref(),
        audit_meta.ip_address.as_deref(),
    )
    .await?;

    password_reset_service::reset_password(
        &state.pool,
        &req.token,
        &req.new_password,
        Some(&audit_meta),
    )
    .await?;
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
    audit_meta: AuditRequestMeta,
    Json(req): Json<ChangePasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    auth_service::change_password(
        &state.pool,
        auth.0.sub,
        &req.current_password,
        &req.new_password,
        Some(&audit_meta),
    )
    .await?;

    Ok(Json(serde_json::json!({
        "message": "Password changed successfully."
    })))
}
