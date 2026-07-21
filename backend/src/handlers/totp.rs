use axum::{Json, extract::State, response::Response};

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, verify_mfa_pending_token};
use crate::core::error::AppResult;
use crate::core::extract::ValidatedJson;
use crate::handlers::auth::login_outcome_response;
use crate::models::session::LoginOutcome;
use crate::models::totp::{
    TotpConfirmRequest, TotpConfirmResponse, TotpDisableRequest, TotpRegenerateBackupCodesRequest,
    TotpSetupResponse, TotpStatusResponse, TotpVerifyLoginRequest,
};
use crate::services::{auth_service, totp_service};

/// Starts (or restarts) TOTP enrollment for the current user.
pub async fn setup_begin(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<TotpSetupResponse>> {
    let user = auth_service::get_user_by_id(&state.pool, auth.0.sub).await?;
    let resp = totp_service::begin_setup(&state.pool, &user, &state.config.jwt_secret).await?;
    Ok(Json(resp))
}

/// Confirms enrollment with the first code and returns one-time backup codes.
pub async fn setup_confirm(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<TotpConfirmRequest>,
) -> AppResult<Json<TotpConfirmResponse>> {
    let backup_codes =
        totp_service::confirm_setup(&state.pool, auth.0.sub, &req.code, &state.config.jwt_secret)
            .await?;
    Ok(Json(TotpConfirmResponse { backup_codes }))
}

pub async fn status(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<TotpStatusResponse>> {
    let enabled = totp_service::is_enabled(&state.pool, auth.0.sub).await?;
    Ok(Json(TotpStatusResponse { enabled }))
}

pub async fn disable(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<TotpDisableRequest>,
) -> AppResult<Json<serde_json::Value>> {
    totp_service::disable(&state.pool, auth.0.sub, &req.password).await?;
    Ok(Json(serde_json::json!({ "message": "2FA disabled" })))
}

pub async fn regenerate_backup_codes(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<TotpRegenerateBackupCodesRequest>,
) -> AppResult<Json<TotpConfirmResponse>> {
    let backup_codes =
        totp_service::regenerate_backup_codes(&state.pool, auth.0.sub, &req.password).await?;
    Ok(Json(TotpConfirmResponse { backup_codes }))
}

/// Second half of login when the account has 2FA enabled: verifies the
/// MFA-pending token + code, then issues the real session (same response
/// shape as `/auth/login`).
pub async fn verify_login(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<TotpVerifyLoginRequest>,
) -> AppResult<Response> {
    let user_id = verify_mfa_pending_token(&req.mfa_token, &state.config.jwt_secret)?;
    totp_service::verify_login_code(&state.pool, user_id, &req.code, &state.config.jwt_secret)
        .await?;

    let user = auth_service::get_active_user(&state.pool, user_id).await?;
    let session = auth_service::issue_session(
        &state.pool,
        user,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
    )
    .await?;

    Ok(login_outcome_response(
        LoginOutcome::Session(session),
        &state.config.frontend_url,
    ))
}
