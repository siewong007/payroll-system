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
use crate::handlers::auth::login_outcome_response;
use crate::models::passkey::{
    AuthBeginRequest, AuthBeginResponse, AuthCompleteRequest, CheckPasskeyRequest,
    DiscoverableAuthBeginResponse, PasskeyInfo, RegistrationBeginResponse,
    RegistrationCompleteRequest, RenamePasskeyRequest,
};
use crate::services::{auth_service, passkey_service};

// ── Registration (authenticated user adds a passkey) ───────────────────

pub async fn registration_begin(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<RegistrationBeginResponse>> {
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
    Json(req): Json<RegistrationCompleteRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let state_json =
        passkey_service::get_and_delete_challenge(&state.pool, req.challenge_id, "registration")
            .await?;

    let reg_state: PasskeyRegistration = serde_json::from_value(state_json)
        .map_err(|e| AppError::BadRequest(format!("Invalid registration state: {}", e)))?;

    let passkey = state
        .webauthn
        .finish_passkey_registration(&req.credential, &reg_state)
        .map_err(|e| AppError::BadRequest(format!("WebAuthn registration failed: {}", e)))?;

    let name = req.name.unwrap_or_else(|| "My Passkey".to_string());
    passkey_service::save_passkey(&state.pool, auth.0.sub, &name, &passkey).await?;

    Ok(Json(
        serde_json::json!({"message": "Passkey registered successfully"}),
    ))
}

// ── Authentication (unauthenticated user logs in with passkey) ─────────

pub async fn authentication_begin(
    State(state): State<AppState>,
    Json(req): Json<AuthBeginRequest>,
) -> AppResult<Json<AuthBeginResponse>> {
    let user_id = passkey_service::get_user_id_by_email_with_passkeys(&state.pool, &req.email)
        .await?
        .ok_or_else(|| AppError::BadRequest("No passkeys registered for this email".into()))?;

    let passkeys = passkey_service::get_passkeys_for_user(&state.pool, user_id).await?;

    if passkeys.is_empty() {
        return Err(AppError::BadRequest(
            "No passkeys registered for this email".into(),
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
    Json(req): Json<AuthCompleteRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Consume the challenge (which carries the target user_id)
    let (user_id, state_json) =
        passkey_service::take_authentication_challenge(&state.pool, req.challenge_id).await?;

    let auth_state: PasskeyAuthentication = serde_json::from_value(state_json)
        .map_err(|e| AppError::BadRequest(format!("Invalid auth state: {}", e)))?;

    let auth_result = state
        .webauthn
        .finish_passkey_authentication(&req.credential, &auth_state)
        .map_err(|e| AppError::Unauthorized(format!("Passkey authentication failed: {}", e)))?;

    // Update credential counter
    let mut passkeys = passkey_service::get_passkeys_for_user(&state.pool, user_id).await?;
    for pk in passkeys.iter_mut() {
        if pk.cred_id() == auth_result.cred_id() {
            pk.update_credential(&auth_result);
            passkey_service::update_passkey_after_auth(&state.pool, user_id, pk).await?;
            break;
        }
    }

    // Issue tokens (same as password login, gated on 2FA if enabled)
    let outcome = auth_service::complete_login(
        &state.pool,
        user_id,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
    )
    .await?;

    Ok(login_outcome_response(outcome, &state.config.frontend_url))
}

// ── Discoverable Authentication (no email required) ─────────────────────

pub async fn discoverable_auth_begin(
    State(state): State<AppState>,
) -> AppResult<Json<DiscoverableAuthBeginResponse>> {
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
    Json(req): Json<AuthCompleteRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Consume the challenge
    let state_json =
        passkey_service::take_discoverable_challenge(&state.pool, req.challenge_id).await?;

    let auth_state: DiscoverableAuthentication = serde_json::from_value(state_json)
        .map_err(|e| AppError::BadRequest(format!("Invalid auth state: {}", e)))?;

    // Identify which user is authenticating from the credential response
    let (user_uuid, _cred_id_bytes) = state
        .webauthn
        .identify_discoverable_authentication(&req.credential)
        .map_err(|e| AppError::Unauthorized(format!("Failed to identify credential: {}", e)))?;

    let user_id = user_uuid;

    // Load the user's passkeys to finish verification
    let passkeys = passkey_service::get_passkeys_for_user(&state.pool, user_id).await?;
    let discoverable_keys: Vec<DiscoverableKey> =
        passkeys.iter().map(DiscoverableKey::from).collect();

    let auth_result = state
        .webauthn
        .finish_discoverable_authentication(&req.credential, auth_state, &discoverable_keys)
        .map_err(|e| AppError::Unauthorized(format!("Passkey authentication failed: {}", e)))?;

    // Update credential counter
    let mut passkeys = passkey_service::get_passkeys_for_user(&state.pool, user_id).await?;
    for pk in passkeys.iter_mut() {
        if pk.cred_id() == auth_result.cred_id() {
            pk.update_credential(&auth_result);
            passkey_service::update_passkey_after_auth(&state.pool, user_id, pk).await?;
            break;
        }
    }

    // Issue tokens (gated on 2FA if enabled)
    let outcome = auth_service::complete_login(
        &state.pool,
        user_id,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
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
    passkey_service::rename_passkey(&state.pool, auth.0.sub, id, &req.name).await?;
    Ok(Json(serde_json::json!({"message": "Passkey renamed"})))
}

pub async fn delete_passkey(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    passkey_service::delete_passkey(&state.pool, auth.0.sub, id).await?;
    Ok(Json(serde_json::json!({"message": "Passkey deleted"})))
}

/// Check if a given email has passkeys (used by frontend to show passkey button)
pub async fn check_passkey(
    State(state): State<AppState>,
    Json(req): Json<CheckPasskeyRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let has = passkey_service::get_user_id_by_email_with_passkeys(&state.pool, &req.email)
        .await?
        .is_some();
    Ok(Json(serde_json::json!({"has_passkey": has})))
}
