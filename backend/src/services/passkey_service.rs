use sqlx::PgPool;
use uuid::Uuid;
use webauthn_rs::prelude::*;

use crate::core::error::{AppError, AppResult};
use crate::models::passkey::PasskeyInfo;
use crate::repositories::reads::passkey as passkey_reads;
use crate::repositories::{passkey_challenges, passkey_credentials};

// ── Credential CRUD ────────────────────────────────────────────────────

pub async fn list_passkeys(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<PasskeyInfo>> {
    let rows = passkey_credentials::list_for_user(pool, user_id).await?;

    Ok(rows
        .into_iter()
        .map(|r| PasskeyInfo {
            id: r.id,
            credential_name: r.credential_name,
            created_at: r.created_at,
            last_used_at: r.last_used_at,
        })
        .collect())
}

pub async fn get_passkeys_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<Passkey>> {
    let rows = passkey_credentials::all_for_user(pool, user_id).await?;

    rows.into_iter()
        .map(|r| {
            serde_json::from_value::<Passkey>(r.credential_json)
                .map_err(|e| AppError::Internal(format!("Failed to deserialize passkey: {}", e)))
        })
        .collect()
}

pub async fn save_passkey(
    pool: &PgPool,
    user_id: Uuid,
    name: &str,
    passkey: &Passkey,
) -> AppResult<()> {
    let json = serde_json::to_value(passkey)
        .map_err(|e| AppError::Internal(format!("Failed to serialize passkey: {}", e)))?;

    passkey_credentials::insert(pool, user_id, name, &json).await
}

pub async fn update_passkey_after_auth(
    pool: &PgPool,
    user_id: Uuid,
    updated_passkey: &Passkey,
) -> AppResult<()> {
    // webauthn-rs may update the credential's counter; persist that
    let json = serde_json::to_value(updated_passkey)
        .map_err(|e| AppError::Internal(format!("Failed to serialize passkey: {}", e)))?;

    // Match by the credential ID inside the JSON
    let cred_id = serde_json::to_value(updated_passkey.cred_id())
        .map_err(|e| AppError::Internal(format!("Failed to serialize cred_id: {}", e)))?;

    passkey_credentials::update_credential_json(pool, user_id, &cred_id, &json).await
}

pub async fn rename_passkey(
    pool: &PgPool,
    user_id: Uuid,
    passkey_id: Uuid,
    name: &str,
) -> AppResult<()> {
    let rows = passkey_credentials::rename(pool, passkey_id, user_id, name).await?;

    if rows == 0 {
        return Err(AppError::NotFound("Passkey not found".into()));
    }
    Ok(())
}

pub async fn delete_passkey(pool: &PgPool, user_id: Uuid, passkey_id: Uuid) -> AppResult<()> {
    let rows = passkey_credentials::delete(pool, passkey_id, user_id).await?;

    if rows == 0 {
        return Err(AppError::NotFound("Passkey not found".into()));
    }
    Ok(())
}

// ── Challenge storage ──────────────────────────────────────────────────

pub async fn store_challenge(
    pool: &PgPool,
    user_id: Option<Uuid>,
    email: Option<&str>,
    challenge_type: &str,
    state: &serde_json::Value,
) -> AppResult<Uuid> {
    // Clean up expired challenges first
    passkey_challenges::delete_expired(pool).await?;
    passkey_challenges::insert(pool, user_id, email, challenge_type, state).await
}

pub async fn get_and_delete_challenge(
    pool: &PgPool,
    challenge_id: Uuid,
    challenge_type: &str,
) -> AppResult<serde_json::Value> {
    passkey_challenges::consume(pool, challenge_id, challenge_type)
        .await?
        .ok_or_else(|| AppError::BadRequest("Challenge expired or not found".into()))
}

/// Consume an authentication challenge, returning the targeted user id and WebAuthn state.
pub async fn take_authentication_challenge(
    pool: &PgPool,
    challenge_id: Uuid,
) -> AppResult<(Uuid, serde_json::Value)> {
    let consumed = passkey_challenges::consume_authentication(pool, challenge_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("Challenge expired or not found".into()))?;

    let user_id = consumed
        .user_id
        .ok_or_else(|| AppError::Internal("Missing user_id in challenge".into()))?;

    Ok((user_id, consumed.state_json))
}

/// Consume a discoverable-auth challenge, returning its WebAuthn state.
pub async fn take_discoverable_challenge(
    pool: &PgPool,
    challenge_id: Uuid,
) -> AppResult<serde_json::Value> {
    passkey_challenges::consume_discoverable(pool, challenge_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("Challenge expired or not found".into()))
}

// ── Find users with passkeys by email ──────────────────────────────────

pub async fn get_user_id_by_email_with_passkeys(
    pool: &PgPool,
    email: &str,
) -> AppResult<Option<Uuid>> {
    passkey_reads::user_id_by_email_with_passkeys(pool, email).await
}
