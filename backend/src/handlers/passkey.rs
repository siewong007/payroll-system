use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use webauthn_rs::prelude::*;

use crate::core::app_state::AppState;
use crate::core::auth::{create_token, AuthUser};
use crate::core::cookie;
use crate::core::error::{AppError, AppResult};
use crate::models::passkey::{PasskeyInfo, RenamePasskeyRequest};
use crate::models::user::{LoginResponse, User, UserResponse};
use crate::services::{passkey_service, session_service};

// ── Registration (authenticated user adds a passkey) ───────────────────

#[derive(Serialize)]
pub struct RegistrationBeginResponse {
    pub challenge_id: Uuid,
    pub options: CreationChallengeResponse,
}

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
        .start_passkey_registration(
            Uuid::from(user_id),
            user_email,
            user_email,
            Some(exclude),
        )
        .map_err(|e| AppError::Internal(format!("WebAuthn registration start failed: {}", e)))?;

    let state_json = serde_json::to_value(&reg_state)
        .map_err(|e| AppError::Internal(format!("Failed to serialize reg state: {}", e)))?;

    let challenge_id =
        passkey_service::store_challenge(&state.pool, Some(user_id), None, "registration", &state_json)
            .await?;

    Ok(Json(RegistrationBeginResponse {
        challenge_id,
        options: ccr,
    }))
}

#[derive(Deserialize)]
pub struct RegistrationCompleteRequest {
    pub challenge_id: Uuid,
    pub credential: RegisterPublicKeyCredential,
    pub name: Option<String>,
}

pub async fn registration_complete(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<RegistrationCompleteRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let state_json = passkey_service::get_and_delete_challenge(
        &state.pool,
        req.challenge_id,
        "registration",
    )
    .await?;

    let reg_state: PasskeyRegistration = serde_json::from_value(state_json)
        .map_err(|e| AppError::BadRequest(format!("Invalid registration state: {}", e)))?;

    let passkey = state
        .webauthn
        .finish_passkey_registration(&req.credential, &reg_state)
        .map_err(|e| AppError::BadRequest(format!("WebAuthn registration failed: {}", e)))?;

    let name = req.name.unwrap_or_else(|| "My Passkey".to_string());
    passkey_service::save_passkey(&state.pool, auth.0.sub, &name, &passkey).await?;

    Ok(Json(serde_json::json!({"message": "Passkey registered successfully"})))
}

// ── Authentication (unauthenticated user logs in with passkey) ─────────

#[derive(Deserialize)]
pub struct AuthBeginRequest {
    pub email: String,
}

#[derive(Serialize)]
pub struct AuthBeginResponse {
    pub challenge_id: Uuid,
    pub options: RequestChallengeResponse,
}

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

#[derive(Deserialize)]
pub struct AuthCompleteRequest {
    pub challenge_id: Uuid,
    pub credential: PublicKeyCredential,
}

pub async fn authentication_complete(
    State(state): State<AppState>,
    Json(req): Json<AuthCompleteRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Get the challenge (which includes user_id)
    let challenge_row: Option<(Option<Uuid>, serde_json::Value)> = sqlx::query_as(
        r#"DELETE FROM passkey_challenges
        WHERE id = $1 AND challenge_type = 'authentication' AND expires_at > NOW()
        RETURNING user_id, state_json"#,
    )
    .bind(req.challenge_id)
    .fetch_optional(&state.pool)
    .await?;

    let (user_id, state_json) = challenge_row
        .ok_or_else(|| AppError::BadRequest("Challenge expired or not found".into()))?;

    let user_id = user_id.ok_or_else(|| AppError::Internal("Missing user_id in challenge".into()))?;

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

    // Fetch user and issue tokens (same as password login)
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = $1 AND is_active = TRUE",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("User not found or inactive".into()))?;

    // Update last login
    sqlx::query("UPDATE users SET last_login = NOW() WHERE id = $1")
        .bind(user.id)
        .execute(&state.pool)
        .await?;

    let token = create_token(
        user.id,
        &user.email,
        &user.role,
        user.company_id,
        user.employee_id,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
    )?;

    let refresh_token = session_service::create_refresh_token(&state.pool, user.id).await?;

    let mut headers = HeaderMap::new();
    let (name, value) = cookie::set_refresh_cookie(&refresh_token, &state.config.frontend_url);
    headers.insert(name, value.parse().unwrap());

    let body = LoginResponse {
        token,
        refresh_token: None,
        user: UserResponse::from(user),
    };

    Ok((headers, Json(body)))
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
#[derive(Deserialize)]
pub struct CheckPasskeyRequest {
    pub email: String,
}

pub async fn check_passkey(
    State(state): State<AppState>,
    Json(req): Json<CheckPasskeyRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let has = passkey_service::get_user_id_by_email_with_passkeys(&state.pool, &req.email)
        .await?
        .is_some();
    Ok(Json(serde_json::json!({"has_passkey": has})))
}
