use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
};
use uuid::Uuid;
use webauthn_rs::prelude::*;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::core::extract::ValidatedJson;
use crate::handlers::auth::login_outcome_response;
use crate::models::audit::AuditRequestMeta;
use crate::models::passkey::{
    AuthBeginRequest, AuthBeginResponse, AuthCompleteRequest, CheckPasskeyRequest,
    DeletePasskeyRequest, DiscoverableAuthBeginRequest, DiscoverableAuthBeginResponse, PasskeyInfo,
    RegistrationBeginRequest, RegistrationBeginResponse, RegistrationCompleteRequest,
    RenamePasskeyRequest,
};
use crate::services::{audit_service, auth_service, passkey_service, totp_service};

async fn log_passkey_login_failed(
    state: &AppState,
    user_id: Option<Uuid>,
    reason: &str,
    audit_meta: &AuditRequestMeta,
) {
    let _ = audit_service::log_action_with_metadata(
        &state.pool,
        None,
        user_id,
        "login_failed",
        "auth",
        user_id,
        None::<serde_json::Value>,
        Some(serde_json::json!({
            "reason": reason,
            "method": "passkey",
        })),
        Some("Passkey authentication failed"),
        Some(audit_meta),
    )
    .await;
}

// ── Registration (authenticated user adds a passkey) ───────────────────

pub async fn registration_begin(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    ValidatedJson(req): ValidatedJson<RegistrationBeginRequest>,
) -> AppResult<Json<RegistrationBeginResponse>> {
    auth_service::verify_user_password(&state.pool, auth.0.sub, &req.password, Some(&audit_meta))
        .await?;
    totp_service::verify_step_up_if_enabled(
        &state.pool,
        auth.0.sub,
        req.totp_code.as_deref(),
        &state.config.totp_encryption_key,
        Some(&audit_meta),
    )
    .await?;

    let user_id = auth.0.sub;
    let user_email = &auth.0.email;

    // Get existing passkeys to exclude
    let existing = passkey_service::get_passkeys_for_user(&state.pool, user_id).await?;
    let exclude: Vec<CredentialID> = existing.iter().map(|p| p.cred_id().clone()).collect();

    let (ccr, reg_state) = state
        .webauthn
        .start_passkey_registration(user_id, user_email, user_email, Some(exclude))
        .map_err(|e| AppError::Internal(format!("WebAuthn registration start failed: {}", e)))?;

    let state_json = serde_json::to_value(&reg_state)
        .map_err(|e| AppError::Internal(format!("Failed to serialize reg state: {}", e)))?;

    let challenge_id = passkey_service::store_challenge(
        &state.pool,
        Some(user_id),
        None,
        "registration",
        &state_json,
    )
    .await?;

    Ok(Json(RegistrationBeginResponse {
        challenge_id,
        options: ccr,
    }))
}

pub async fn registration_complete(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Json(req): Json<RegistrationCompleteRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let state_json =
        passkey_service::consume_registration_challenge(&state.pool, req.challenge_id, auth.0.sub)
            .await?;

    let reg_state: PasskeyRegistration = serde_json::from_value(state_json)
        .map_err(|e| AppError::BadRequest(format!("Invalid registration state: {}", e)))?;

    let passkey = state
        .webauthn
        .finish_passkey_registration(&req.credential, &reg_state)
        .map_err(|e| AppError::BadRequest(format!("WebAuthn registration failed: {}", e)))?;

    let name = req.name.unwrap_or_else(|| "My Passkey".to_string());
    passkey_service::save_passkey(&state.pool, auth.0.sub, &name, &passkey, Some(&audit_meta))
        .await?;

    Ok(Json(
        serde_json::json!({"message": "Passkey registered successfully"}),
    ))
}

// ── Authentication (unauthenticated user logs in with passkey) ─────────

pub async fn authentication_begin(
    State(state): State<AppState>,
    audit_meta: AuditRequestMeta,
    Json(req): Json<AuthBeginRequest>,
) -> AppResult<Json<AuthBeginResponse>> {
    crate::core::turnstile::verify_any_action(
        &state.config,
        req.turnstile_token.as_deref(),
        audit_meta.ip_address.as_deref(),
        &["login", "passkey_auth"],
    )
    .await?;

    let user_id = passkey_service::get_user_id_by_email_with_passkeys(&state.pool, &req.email)
        .await?
        .ok_or_else(|| AppError::BadRequest("Unable to start passkey sign-in".into()))?;

    let passkeys = passkey_service::get_passkeys_for_user(&state.pool, user_id).await?;

    if passkeys.is_empty() {
        return Err(AppError::BadRequest(
            "Unable to start passkey sign-in".into(),
        ));
    }

    let (rcr, auth_state) = state
        .webauthn
        .start_passkey_authentication(&passkeys)
        .map_err(|e| AppError::Internal(format!("WebAuthn auth start failed: {}", e)))?;

    let state_json = serde_json::to_value(&auth_state)
        .map_err(|e| AppError::Internal(format!("Failed to serialize auth state: {}", e)))?;

    let challenge_id = passkey_service::store_challenge(
        &state.pool,
        Some(user_id),
        Some(&req.email),
        "authentication",
        &state_json,
    )
    .await?;

    Ok(Json(AuthBeginResponse {
        challenge_id,
        options: rcr,
    }))
}

pub async fn authentication_complete(
    State(state): State<AppState>,
    headers: HeaderMap,
    audit_meta: AuditRequestMeta,
    Json(req): Json<AuthCompleteRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Consume the challenge (which carries the target user_id)
    let (user_id, state_json) =
        passkey_service::take_authentication_challenge(&state.pool, req.challenge_id).await?;

    let auth_state: PasskeyAuthentication = serde_json::from_value(state_json)
        .map_err(|e| AppError::BadRequest(format!("Invalid auth state: {}", e)))?;

    let auth_result = match state
        .webauthn
        .finish_passkey_authentication(&req.credential, &auth_state)
    {
        Ok(res) => res,
        Err(e) => {
            log_passkey_login_failed(&state, Some(user_id), "invalid_assertion", &audit_meta).await;
            return Err(AppError::Unauthorized(format!(
                "Passkey authentication failed: {}",
                e
            )));
        }
    };

    // Update credential counter (fail closed if the credential disappeared concurrently)
    let passkeys = passkey_service::get_passkey_records_for_user(&state.pool, user_id).await?;
    let (pk_id, _) = passkeys
        .iter()
        .find(|(_, pk)| pk.cred_id() == auth_result.cred_id())
        .ok_or_else(|| AppError::Unauthorized("Passkey credential was removed".into()))?;
    passkey_service::update_passkey_after_auth(&state.pool, *pk_id, user_id, &auth_result).await?;

    // Issue tokens (same as password login, gated on 2FA if enabled)
    let outcome = auth_service::complete_login(
        &state.pool,
        user_id,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
        "passkey",
        Some(&audit_meta),
    )
    .await?;

    Ok(login_outcome_response(outcome, &state.config.frontend_url))
}

// ── Discoverable Authentication (no email required) ─────────────────────

pub async fn discoverable_auth_begin(
    State(state): State<AppState>,
    audit_meta: AuditRequestMeta,
    Json(req): Json<DiscoverableAuthBeginRequest>,
) -> AppResult<Json<DiscoverableAuthBeginResponse>> {
    crate::core::turnstile::verify_any_action(
        &state.config,
        req.turnstile_token.as_deref(),
        audit_meta.ip_address.as_deref(),
        &["login", "passkey_discoverable"],
    )
    .await?;

    let (mut rcr, auth_state) =
        state
            .webauthn
            .start_discoverable_authentication()
            .map_err(|e| {
                AppError::Internal(format!("WebAuthn discoverable auth start failed: {}", e))
            })?;

    // Remove conditional mediation so the browser shows a modal picker on button click
    rcr.mediation = None;

    let state_json = serde_json::to_value(&auth_state)
        .map_err(|e| AppError::Internal(format!("Failed to serialize auth state: {}", e)))?;

    let challenge_id =
        passkey_service::store_challenge(&state.pool, None, None, "discoverable", &state_json)
            .await?;

    Ok(Json(DiscoverableAuthBeginResponse {
        challenge_id,
        options: rcr,
    }))
}

pub async fn discoverable_auth_complete(
    State(state): State<AppState>,
    headers: HeaderMap,
    audit_meta: AuditRequestMeta,
    Json(req): Json<AuthCompleteRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Consume the challenge
    let state_json =
        passkey_service::take_discoverable_challenge(&state.pool, req.challenge_id).await?;

    let auth_state: DiscoverableAuthentication = serde_json::from_value(state_json)
        .map_err(|e| AppError::BadRequest(format!("Invalid auth state: {}", e)))?;

    // Identify which user is authenticating from the credential response
    let (user_uuid, cred_id_bytes) = match state
        .webauthn
        .identify_discoverable_authentication(&req.credential)
    {
        Ok(id) => id,
        Err(e) => {
            log_passkey_login_failed(&state, None, "unknown_credential", &audit_meta).await;
            return Err(AppError::Unauthorized(format!(
                "Failed to identify credential: {}",
                e
            )));
        }
    };

    let user_id = user_uuid;

    // Load the user's passkeys to finish verification
    let passkeys = passkey_service::get_passkey_records_for_user(&state.pool, user_id).await?;
    let Some((matched_id, matched_passkey)) = passkeys
        .iter()
        .find(|(_, pk)| pk.cred_id().as_slice() == cred_id_bytes)
    else {
        log_passkey_login_failed(&state, Some(user_id), "credential_not_found", &audit_meta).await;
        return Err(AppError::Unauthorized(
            "Passkey authentication failed".into(),
        ));
    };

    let matched_id = *matched_id;
    let auth_result = match state.webauthn.finish_discoverable_authentication_v2(
        &req.credential,
        auth_state,
        DiscoverableKey::from(matched_passkey),
    ) {
        Ok(res) => res,
        Err(e) => {
            log_passkey_login_failed(&state, Some(user_id), "invalid_assertion", &audit_meta).await;
            return Err(AppError::Unauthorized(format!(
                "Passkey authentication failed: {}",
                e
            )));
        }
    };

    // Update credential counter
    passkey_service::update_passkey_after_auth(&state.pool, matched_id, user_id, &auth_result)
        .await?;

    // Issue tokens (gated on 2FA if enabled)
    let outcome = auth_service::complete_login(
        &state.pool,
        user_id,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
        "passkey",
        Some(&audit_meta),
    )
    .await?;

    Ok(login_outcome_response(outcome, &state.config.frontend_url))
}

// ── Passkey Management (authenticated) ─────────────────────────────────

pub async fn list_passkeys(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<PasskeyInfo>>> {
    let passkeys = passkey_service::list_passkeys(&state.pool, auth.0.sub).await?;
    Ok(Json(passkeys))
}

pub async fn rename_passkey(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<RenamePasskeyRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let trimmed = req.name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 100 || trimmed.chars().any(char::is_control)
    {
        return Err(AppError::BadRequest(
            "Passkey name must be 1-100 characters without control characters".into(),
        ));
    }
    passkey_service::rename_passkey(&state.pool, auth.0.sub, id, trimmed).await?;
    Ok(Json(serde_json::json!({"message": "Passkey renamed"})))
}

pub async fn delete_passkey(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<DeletePasskeyRequest>,
) -> AppResult<Json<serde_json::Value>> {
    auth_service::verify_user_password(&state.pool, auth.0.sub, &req.password, Some(&audit_meta))
        .await?;
    totp_service::verify_step_up_if_enabled(
        &state.pool,
        auth.0.sub,
        req.totp_code.as_deref(),
        &state.config.totp_encryption_key,
        Some(&audit_meta),
    )
    .await?;
    passkey_service::delete_passkey(&state.pool, auth.0.sub, id, Some(&audit_meta)).await?;
    Ok(Json(serde_json::json!({"message": "Passkey deleted"})))
}

/// Deprecated passkey existence check: kept for route compatibility while
/// always returning `false` so unauthenticated callers cannot enumerate emails.
pub async fn check_passkey(
    State(state): State<AppState>,
    audit_meta: AuditRequestMeta,
    Json(req): Json<CheckPasskeyRequest>,
) -> AppResult<Json<serde_json::Value>> {
    crate::core::turnstile::verify(
        &state.config,
        req.turnstile_token.as_deref(),
        audit_meta.ip_address.as_deref(),
        "passkey_check",
    )
    .await?;

    let _ = &req.email;
    Ok(Json(serde_json::json!({"has_passkey": false})))
}
