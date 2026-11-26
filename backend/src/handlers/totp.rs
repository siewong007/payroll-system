use axum::{Json, extract::State, http::HeaderMap, response::Response};

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, verify_mfa_pending_token};
use crate::core::error::AppResult;
use crate::core::extract::ValidatedJson;
use crate::handlers::auth::login_outcome_response;
use crate::models::audit::AuditRequestMeta;
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
    let resp =
        totp_service::begin_setup(&state.pool, &user, &state.config.totp_encryption_key).await?;
    Ok(Json(resp))
}

/// Confirms enrollment with the first code and returns one-time backup codes.
pub async fn setup_confirm(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    ValidatedJson(req): ValidatedJson<TotpConfirmRequest>,
) -> AppResult<Json<TotpConfirmResponse>> {
    let backup_codes = totp_service::confirm_setup_with_session(
        &state.pool,
        auth.0.sub,
        Some(auth.0.sid),
        Some(&req.password),
        &req.code,
        &state.config.totp_encryption_key,
        Some(&audit_meta),
    )
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
    audit_meta: AuditRequestMeta,
    ValidatedJson(req): ValidatedJson<TotpDisableRequest>,
) -> AppResult<Json<serde_json::Value>> {
    totp_service::disable_with_session(
        &state.pool,
        auth.0.sub,
        Some(auth.0.sid),
        &req.password,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(serde_json::json!({ "message": "2FA disabled" })))
}

pub async fn regenerate_backup_codes(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    ValidatedJson(req): ValidatedJson<TotpRegenerateBackupCodesRequest>,
) -> AppResult<Json<TotpConfirmResponse>> {
    let backup_codes = totp_service::regenerate_backup_codes(
        &state.pool,
        auth.0.sub,
        &req.password,
        &state.config.totp_encryption_key,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(TotpConfirmResponse { backup_codes }))
}

/// Second half of login when the account has 2FA enabled: verifies the
/// MFA-pending token + code, then issues the real session (same response
/// shape as `/auth/login`).
pub async fn verify_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    audit_meta: AuditRequestMeta,
    ValidatedJson(req): ValidatedJson<TotpVerifyLoginRequest>,
) -> AppResult<Response> {
    crate::core::turnstile::verify(
        &state.config,
        req.turnstile_token.as_deref(),
        audit_meta.ip_address.as_deref(),
    )
    .await?;

    let (user_id, jti) = verify_mfa_pending_token(&req.mfa_token, &state.config.jwt_secret)?;
    if let Err(err) = totp_service::verify_login_code_for_challenge(
        &state.pool,
        user_id,
        jti,
        &req.code,
        &state.config.totp_encryption_key,
    )
    .await
    {
        let company_id = crate::repositories::users::get_by_id(&state.pool, user_id)
            .await
            .ok()
            .flatten()
            .and_then(|u| u.company_id);
        let _ = crate::services::audit_service::log_action_with_metadata(
            &state.pool,
            company_id,
            Some(user_id),
            "login_failed",
            "auth",
            Some(user_id),
            None::<serde_json::Value>,
            Some(serde_json::json!({
                "reason": "invalid_2fa_code",
                "method": "totp",
            })),
            Some("Failed 2FA verification attempt"),
            Some(&audit_meta),
        )
        .await;
        return Err(err);
    }

    // This is the one minting path that does not go through `complete_login`,
    // which is why the terminated-employee gate lives inside `get_active_user`.
    let user = auth_service::get_active_user(&state.pool, user_id).await?;
    let session = auth_service::issue_session(
        &state.pool,
        user,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
        "totp",
        Some(&audit_meta),
    )
    .await?;

    Ok(login_outcome_response(
        LoginOutcome::Session(session),
        &state.config.frontend_url,
    ))
}
