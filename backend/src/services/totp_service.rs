use sqlx::PgPool;
use totp_rs::{Algorithm, Secret, TOTP};
use uuid::Uuid;

use crate::core::crypto;
use crate::core::error::{AppError, AppResult};
use crate::models::totp::TotpSetupResponse;
use crate::models::user::User;
use crate::repositories::{user_totp, user_totp_backup_codes};
use crate::services::auth_service;

const ISSUER: &str = "PayrollMY";
const BACKUP_CODE_COUNT: usize = 10;
/// Lower than the password hash cost (12) since we hash up to 10 high-entropy
/// random codes synchronously per request; these aren't user-chosen secrets.
const BACKUP_CODE_BCRYPT_COST: u32 = 10;

fn build_totp(secret_bytes: Vec<u8>, account_name: &str) -> AppResult<TOTP> {
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret_bytes,
        Some(ISSUER.to_string()),
        account_name.to_string(),
    )
    .map_err(|e| AppError::Internal(format!("Failed to build TOTP: {e}")))
}

fn secret_to_bytes(secret_b32: &str) -> AppResult<Vec<u8>> {
    Secret::Encoded(secret_b32.to_string())
        .to_bytes()
        .map_err(|e| AppError::Internal(format!("Invalid stored TOTP secret: {e}")))
}

fn generate_backup_codes() -> Vec<String> {
    (0..BACKUP_CODE_COUNT)
        .map(|_| {
            let b = *Uuid::new_v4().as_bytes();
            format!("{:02X}{:02X}-{:02X}{:02X}", b[0], b[1], b[2], b[3])
        })
        .collect()
}

async fn store_backup_codes(pool: &PgPool, user_id: Uuid, codes: &[String]) -> AppResult<()> {
    let hashes: Vec<String> = codes
        .iter()
        .map(|c| bcrypt::hash(c, BACKUP_CODE_BCRYPT_COST))
        .collect::<Result<_, _>>()
        .map_err(|_| AppError::Internal("Failed to hash backup codes".into()))?;
    user_totp_backup_codes::insert_many(pool, user_id, &hashes).await
}

async fn verify_password(pool: &PgPool, user_id: Uuid, password: &str) -> AppResult<()> {
    let user = auth_service::get_user_by_id(pool, user_id).await?;
    let valid = bcrypt::verify(password, &user.password_hash)
        .map_err(|_| AppError::Internal("Password verification failed".into()))?;
    if !valid {
        return Err(AppError::BadRequest("Current password is incorrect".into()));
    }
    Ok(())
}

/// Starts (or restarts) enrollment: generates a fresh secret, stores it
/// encrypted as unconfirmed, and returns the QR/otpauth URL/raw secret. Any
/// prior unconfirmed attempt is overwritten; an already-enabled account must
/// disable first.
pub async fn begin_setup(
    pool: &PgPool,
    user: &User,
    jwt_secret: &str,
) -> AppResult<TotpSetupResponse> {
    if let Some(existing) = user_totp::find_by_user(pool, user.id).await?
        && existing.enabled
    {
        return Err(AppError::Conflict(
            "2FA is already enabled. Disable it before setting up again.".into(),
        ));
    }

    let secret = Secret::generate_secret();
    let secret_b32 = secret.to_encoded().to_string();
    let secret_bytes = secret
        .to_bytes()
        .map_err(|e| AppError::Internal(format!("Failed to generate secret: {e}")))?;

    let totp = build_totp(secret_bytes, &user.email)?;
    let otpauth_url = totp.get_url();
    let qr_code_base64 = totp.get_qr_base64().map_err(AppError::Internal)?;

    let encrypted = crypto::encrypt_secret(&secret_b32, jwt_secret)?;
    user_totp::upsert_pending(pool, user.id, &encrypted).await?;

    Ok(TotpSetupResponse {
        secret: secret_b32,
        otpauth_url,
        qr_code_base64,
    })
}

/// Confirms enrollment with the first code from the authenticator app,
/// enables 2FA, and returns a freshly generated set of backup codes (shown
/// once — only their bcrypt hashes are persisted).
pub async fn confirm_setup(
    pool: &PgPool,
    user_id: Uuid,
    code: &str,
    jwt_secret: &str,
) -> AppResult<Vec<String>> {
    let row = user_totp::find_by_user(pool, user_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("No 2FA setup in progress".into()))?;
    if row.enabled {
        return Err(AppError::Conflict("2FA is already enabled".into()));
    }

    let secret_b32 = crypto::decrypt_secret(&row.secret_encrypted, jwt_secret)?;
    let totp = build_totp(secret_to_bytes(&secret_b32)?, "")?;

    let valid = totp
        .check_current(code)
        .map_err(|e| AppError::Internal(format!("Failed to check code: {e}")))?;
    if !valid {
        return Err(AppError::Unauthorized("Invalid authentication code".into()));
    }

    user_totp::confirm(pool, user_id).await?;

    let backup_codes = generate_backup_codes();
    store_backup_codes(pool, user_id, &backup_codes).await?;

    Ok(backup_codes)
}

pub async fn is_enabled(pool: &PgPool, user_id: Uuid) -> AppResult<bool> {
    Ok(user_totp::find_by_user(pool, user_id)
        .await?
        .map(|r| r.enabled)
        .unwrap_or(false))
}

/// Verifies a login-time code: a current TOTP code, or (as a fallback) an
/// unused backup code, which is consumed on success. Used only at the
/// post-primary-auth MFA gate.
pub async fn verify_login_code(
    pool: &PgPool,
    user_id: Uuid,
    code: &str,
    jwt_secret: &str,
) -> AppResult<()> {
    let row = user_totp::find_by_user(pool, user_id)
        .await?
        .filter(|r| r.enabled)
        .ok_or_else(|| AppError::Unauthorized("2FA is not enabled".into()))?;

    let secret_b32 = crypto::decrypt_secret(&row.secret_encrypted, jwt_secret)?;
    let totp = build_totp(secret_to_bytes(&secret_b32)?, "")?;

    if totp.check_current(code).unwrap_or(false) {
        return Ok(());
    }

    for candidate in user_totp_backup_codes::find_unused(pool, user_id).await? {
        if bcrypt::verify(code, &candidate.code_hash).unwrap_or(false) {
            user_totp_backup_codes::mark_used(pool, candidate.id).await?;
            return Ok(());
        }
    }

    Err(AppError::Unauthorized("Invalid authentication code".into()))
}

/// Disables 2FA after re-verifying the current password.
pub async fn disable(pool: &PgPool, user_id: Uuid, password: &str) -> AppResult<()> {
    verify_password(pool, user_id, password).await?;

    user_totp_backup_codes::delete_for_user(pool, user_id).await?;
    let rows = user_totp::delete_for_user(pool, user_id).await?;
    if rows == 0 {
        return Err(AppError::NotFound("2FA is not enabled".into()));
    }
    Ok(())
}

/// Regenerates backup codes after re-verifying the current password.
/// Invalidates all previously issued codes.
pub async fn regenerate_backup_codes(
    pool: &PgPool,
    user_id: Uuid,
    password: &str,
) -> AppResult<Vec<String>> {
    verify_password(pool, user_id, password).await?;

    if !is_enabled(pool, user_id).await? {
        return Err(AppError::BadRequest("2FA is not enabled".into()));
    }

    user_totp_backup_codes::delete_for_user(pool, user_id).await?;
    let codes = generate_backup_codes();
    store_backup_codes(pool, user_id, &codes).await?;
    Ok(codes)
}
