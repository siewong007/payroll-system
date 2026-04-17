use sqlx::PgPool;
use uuid::Uuid;
use webauthn_rs::prelude::*;

use crate::core::error::{AppError, AppResult};
use crate::models::passkey::{PasskeyCredential, PasskeyInfo};

// ── Credential CRUD ────────────────────────────────────────────────────

pub async fn list_passkeys(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<PasskeyInfo>> {
    let rows = sqlx::query_as::<_, PasskeyCredential>(
        "SELECT * FROM passkey_credentials WHERE user_id = $1 ORDER BY created_at",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

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
    let rows = sqlx::query_as::<_, PasskeyCredential>(
        "SELECT * FROM passkey_credentials WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

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

    sqlx::query(
        r#"INSERT INTO passkey_credentials (user_id, credential_name, credential_json)
        VALUES ($1, $2, $3)"#,
    )
    .bind(user_id)
    .bind(name)
    .bind(&json)
    .execute(pool)
    .await?;

    Ok(())
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

    sqlx::query(
        r#"UPDATE passkey_credentials
        SET credential_json = $3, last_used_at = NOW()
        WHERE user_id = $1 AND credential_json->'cred' ->> 'cred_id' = $2::jsonb ->> 0
        "#,
    )
    .bind(user_id)
    .bind(&cred_id)
    .bind(&json)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn rename_passkey(
    pool: &PgPool,
    user_id: Uuid,
    passkey_id: Uuid,
    name: &str,
) -> AppResult<()> {
    let result = sqlx::query(
        "UPDATE passkey_credentials SET credential_name = $3 WHERE id = $1 AND user_id = $2",
    )
    .bind(passkey_id)
    .bind(user_id)
    .bind(name)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Passkey not found".into()));
    }
    Ok(())
}

pub async fn delete_passkey(pool: &PgPool, user_id: Uuid, passkey_id: Uuid) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM passkey_credentials WHERE id = $1 AND user_id = $2")
        .bind(passkey_id)
        .bind(user_id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
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
    sqlx::query("DELETE FROM passkey_challenges WHERE expires_at < NOW()")
        .execute(pool)
        .await?;

    let id: (Uuid,) = sqlx::query_as(
        r#"INSERT INTO passkey_challenges (user_id, email, challenge_type, state_json)
        VALUES ($1, $2, $3, $4) RETURNING id"#,
    )
    .bind(user_id)
    .bind(email)
    .bind(challenge_type)
    .bind(state)
    .fetch_one(pool)
    .await?;

    Ok(id.0)
}

pub async fn get_and_delete_challenge(
    pool: &PgPool,
    challenge_id: Uuid,
    challenge_type: &str,
) -> AppResult<serde_json::Value> {
    let row: Option<(serde_json::Value,)> = sqlx::query_as(
        r#"DELETE FROM passkey_challenges
        WHERE id = $1 AND challenge_type = $2 AND expires_at > NOW()
        RETURNING state_json"#,
    )
    .bind(challenge_id)
    .bind(challenge_type)
    .fetch_optional(pool)
    .await?;

    row.map(|r| r.0)
        .ok_or_else(|| AppError::BadRequest("Challenge expired or not found".into()))
}

// ── Find users with passkeys by email ──────────────────────────────────

pub async fn get_user_id_by_email_with_passkeys(
    pool: &PgPool,
    email: &str,
) -> AppResult<Option<Uuid>> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"SELECT u.id FROM users u
        INNER JOIN passkey_credentials pc ON pc.user_id = u.id
        WHERE u.email = $1 AND u.is_active = TRUE
        LIMIT 1"#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.0))
}
