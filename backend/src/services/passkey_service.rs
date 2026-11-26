use sqlx::PgPool;
use uuid::Uuid;
use webauthn_rs::prelude::*;

use crate::core::error::{AppError, AppResult};
use crate::models::audit::AuditRequestMeta;
use crate::models::passkey::PasskeyInfo;
use crate::repositories::reads::passkey as passkey_reads;
use crate::repositories::{passkey_challenges, passkey_credentials, users};
use crate::services::audit_service;

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

pub async fn get_passkey_records_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> AppResult<Vec<(Uuid, Passkey)>> {
    let rows = passkey_credentials::all_for_user(pool, user_id).await?;

    rows.into_iter()
        .map(|r| {
            let pk = serde_json::from_value::<Passkey>(r.credential_json)
                .map_err(|e| AppError::Internal(format!("Failed to deserialize passkey: {e}")))?;
            Ok((r.id, pk))
        })
        .collect()
}

pub async fn get_passkeys_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<Passkey>> {
    Ok(get_passkey_records_for_user(pool, user_id)
        .await?
        .into_iter()
        .map(|(_, pk)| pk)
        .collect())
}

pub async fn save_passkey(
    pool: &PgPool,
    user_id: Uuid,
    name: &str,
    passkey: &Passkey,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let json = serde_json::to_value(passkey)
        .map_err(|e| AppError::Internal(format!("Failed to serialize passkey: {}", e)))?;

    passkey_credentials::insert(pool, user_id, name, &json).await?;

    audit_credential_event(pool, user_id, "passkey_registered", Some(name), audit_meta).await;
    Ok(())
}

pub async fn update_passkey_after_auth(
    pool: &PgPool,
    passkey_id: Uuid,
    user_id: Uuid,
    auth_result: &webauthn_rs::prelude::AuthenticationResult,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    let row = passkey_credentials::get_by_id_for_update(&mut *tx, passkey_id, user_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Passkey credential was removed".into()))?;

    let mut passkey: Passkey = serde_json::from_value(row.credential_json.clone())
        .map_err(|e| AppError::Internal(format!("Failed to deserialize passkey: {e}")))?;

    let stored_counter = row
        .credential_json
        .pointer("/cred/counter")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let new_counter = u64::from(auth_result.counter());
    // WebAuthn §7.2 step 17: if either counter is non-zero, the new signature
    // counter must strictly exceed the locked database value. Re-checking after
    // `FOR UPDATE` blocks concurrent assertions using the same counter value.
    if (new_counter > 0 || stored_counter > 0) && new_counter <= stored_counter {
        tracing::warn!(
            %user_id,
            %passkey_id,
            stored_counter,
            new_counter,
            "passkey counter regression or concurrent replay detected"
        );
        return Err(AppError::Unauthorized(
            "Authenticator counter regression detected".into(),
        ));
    }

    passkey
        .update_credential(auth_result)
        .ok_or_else(|| AppError::Unauthorized("Credential mismatch during update".into()))?;

    let json = serde_json::to_value(&passkey)
        .map_err(|e| AppError::Internal(format!("Failed to serialize passkey: {e}")))?;

    let affected =
        passkey_credentials::update_credential_json_by_id(&mut *tx, passkey_id, user_id, &json)
            .await?;
    if affected != 1 {
        return Err(AppError::Unauthorized(
            "Passkey credential update failed".into(),
        ));
    }
    tx.commit().await?;
    Ok(())
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

pub async fn delete_passkey(
    pool: &PgPool,
    user_id: Uuid,
    passkey_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let rows = passkey_credentials::delete(pool, passkey_id, user_id).await?;

    if rows == 0 {
        return Err(AppError::NotFound("Passkey not found".into()));
    }

    audit_credential_event(pool, user_id, "passkey_deleted", None, audit_meta).await;
    Ok(())
}

/// Writes a best-effort credential-lifecycle row, scoped to the user's active
/// company so it appears in that tenant's audit view.
async fn audit_credential_event(
    pool: &PgPool,
    user_id: Uuid,
    action: &str,
    label: Option<&str>,
    audit_meta: Option<&AuditRequestMeta>,
) {
    let company_id = users::get_by_id(pool, user_id)
        .await
        .ok()
        .flatten()
        .and_then(|u| u.company_id);

    let _ = audit_service::log_action_with_metadata(
        pool,
        company_id,
        Some(user_id),
        action,
        "auth",
        Some(user_id),
        None::<serde_json::Value>,
        Some(serde_json::json!({
            "credential": label,
        })),
        Some("Passkey credential change"),
        audit_meta,
    )
    .await;
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

pub async fn consume_registration_challenge(
    pool: &PgPool,
    challenge_id: Uuid,
    user_id: Uuid,
) -> AppResult<serde_json::Value> {
    passkey_challenges::consume_for_user(pool, challenge_id, "registration", user_id)
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
