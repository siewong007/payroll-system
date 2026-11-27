use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::fmt::Write as _;
use totp_rs::{Algorithm, Builder, Secret, Totp};
use uuid::Uuid;

use crate::core::crypto;
use crate::core::error::{AppError, AppResult};
use crate::models::totp::TotpSetupResponse;
use crate::models::user::User;
use crate::repositories::{
    mfa_challenges, push_subscriptions, refresh_tokens, user_sessions, user_totp,
    user_totp_backup_codes,
};
use crate::services::audit_service::{self, AuditRequestMeta};
use crate::services::auth_service;

const ISSUER: &str = "PayrollMY";
const BACKUP_CODE_COUNT: usize = 10;
/// Lower than the password hash cost (12) since we hash up to 10 high-entropy
/// random codes synchronously per request; these aren't user-chosen secrets.
const BACKUP_CODE_BCRYPT_COST: u32 = 10;

fn build_totp(secret: Secret, account_name: &str) -> AppResult<Totp> {
    Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(6)
        .with_skew(1)
        .with_step_duration(30)
        .with_secret(secret)
        .with_issuer(Some(ISSUER))
        .with_account_name(account_name)
        .build()
        .map_err(|e| AppError::Internal(format!("Failed to build TOTP: {e}")))
}

fn parse_secret(secret_b32: &str) -> AppResult<Secret> {
    Secret::try_from_base32(secret_b32)
        .map_err(|e| AppError::Internal(format!("Invalid stored TOTP secret: {e}")))
}

fn generate_backup_codes() -> Vec<String> {
    (0..BACKUP_CODE_COUNT)
        .map(|_| {
            let b = *Uuid::new_v4().as_bytes();
            // Exclude UUIDv4 version/variant bytes (6 and 8) so all 8 random bytes
            // carry uniform OS CSPRNG entropy (64 bits per recovery code).
            format!(
                "{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}",
                b[0], b[1], b[2], b[3], b[4], b[5], b[7], b[9]
            )
        })
        .collect()
}

async fn hash_backup_codes(codes: Vec<String>) -> AppResult<Vec<String>> {
    tokio::task::spawn_blocking(move || {
        codes
            .iter()
            .map(|c| bcrypt::hash(c, BACKUP_CODE_BCRYPT_COST))
            .collect::<Result<Vec<_>, _>>()
    })
    .await
    .map_err(|e| AppError::Internal(format!("Backup code hashing task failed: {e}")))?
    .map_err(|_| AppError::Internal("Failed to hash backup codes".into()))
}

fn backup_code_lookup_hash(encryption_key: &str, user_id: Uuid, normalized_code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"totp-backup-lookup-v1:");
    hasher.update(encryption_key.as_bytes());
    hasher.update(b":");
    hasher.update(user_id.to_string().as_bytes());
    hasher.update(b":");
    hasher.update(normalized_code.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        let _ = write!(&mut hex, "{b:02x}");
    }
    hex
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Verifies the current password and returns the user row, so callers that
/// need the account's company scope for auditing do not have to re-fetch it.
async fn verify_password(
    pool: &PgPool,
    user_id: Uuid,
    password: &str,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<User> {
    auth_service::verify_user_password(pool, user_id, password, audit_meta).await
}

/// Starts (or restarts) enrollment: generates a fresh secret, stores it
/// encrypted as unconfirmed, and returns the QR/otpauth URL/raw secret. Any
/// prior unconfirmed attempt is overwritten; an already-enabled account must
/// disable first.
pub async fn begin_setup(
    pool: &PgPool,
    user: &User,
    encryption_key: &str,
) -> AppResult<TotpSetupResponse> {
    if let Some(existing) = user_totp::find_by_user(pool, user.id).await?
        && existing.enabled
    {
        return Err(AppError::Conflict(
            "2FA is already enabled. Disable it before setting up again.".into(),
        ));
    }

    let secret = Secret::generate();
    let secret_b32 = secret.to_base32();

    let totp = build_totp(secret, &user.email)?;
    let otpauth_url = totp
        .to_url()
        .map_err(|e| AppError::Internal(format!("Failed to build TOTP URL: {e}")))?;
    let qr_code_base64 = totp
        .to_qr_base64()
        .map_err(|e| AppError::Internal(format!("Failed to render TOTP QR code: {e}")))?;

    let encrypted = crypto::encrypt_secret_for_user(&secret_b32, encryption_key, user.id)?;
    if user_totp::upsert_pending(pool, user.id, &encrypted)
        .await?
        .is_none()
    {
        return Err(AppError::Conflict(
            "2FA is already enabled. Disable it before setting up again.".into(),
        ));
    }

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
    encryption_key: &str,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Vec<String>> {
    confirm_setup_with_session(pool, user_id, None, None, code, encryption_key, audit_meta).await
}

/// Session-aware variant used by HTTP setup confirmation so current password is
/// re-verified and other active sessions are revoked upon 2FA enrollment.
pub async fn confirm_setup_with_session(
    pool: &PgPool,
    user_id: Uuid,
    current_session_id: Option<Uuid>,
    password: Option<&str>,
    code: &str,
    encryption_key: &str,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Vec<String>> {
    if let Some(pw) = password {
        auth_service::verify_user_password_for_session(
            pool,
            user_id,
            current_session_id,
            pw,
            audit_meta,
        )
        .await?;
    }

    let row = user_totp::find_by_user(pool, user_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("No 2FA setup in progress".into()))?;
    if row.enabled {
        return Err(AppError::Conflict("2FA is already enabled".into()));
    }

    let secret_b32 =
        crypto::decrypt_secret_for_user(&row.secret_encrypted, encryption_key, user_id)?;
    let totp = build_totp(parse_secret(&secret_b32)?, "")?;

    let trimmed = code.trim();
    let now_step = chrono::Utc::now().timestamp() / 30;
    let mut matched_step: Option<i64> = None;
    for offset in [-1i64, 0, 1] {
        let step = now_step + offset;
        if step >= 0 {
            let expected = totp.generate((step as u64) * 30).to_string();
            if constant_time_eq(trimmed.as_bytes(), expected.as_bytes()) {
                matched_step = Some(step);
            }
        }
    }
    let matched_step =
        matched_step.ok_or_else(|| AppError::Unauthorized("Invalid authentication code".into()))?;

    let backup_codes = generate_backup_codes();
    let lookup_hashes: Vec<String> = backup_codes
        .iter()
        .map(|code| backup_code_lookup_hash(encryption_key, user_id, code))
        .collect();
    let hashes = hash_backup_codes(backup_codes.clone()).await?;

    let mut tx = pool.begin().await?;
    if !user_totp::confirm(&mut *tx, user_id, matched_step, &row.secret_encrypted).await? {
        return Err(AppError::Conflict(
            "2FA setup state changed while confirming. Restart 2FA setup and try again.".into(),
        ));
    }
    user_totp_backup_codes::delete_for_user(&mut *tx, user_id).await?;
    user_totp_backup_codes::insert_many(&mut *tx, user_id, &hashes, &lookup_hashes).await?;
    mfa_challenges::revoke_for_user(&mut *tx, user_id).await?;
    if let Some(sid) = current_session_id {
        user_sessions::revoke_others(&mut *tx, user_id, sid).await?;
        refresh_tokens::revoke_other_sessions(&mut *tx, user_id, sid).await?;
        push_subscriptions::revoke_for_revoked_sessions(&mut *tx, user_id, sid).await?;
    }
    tx.commit().await?;

    // Best-effort: the enrolment succeeded; failing the request over its
    // trail row would leave the account in an ambiguous state. The company
    // scope is what makes the event visible in the tenant's audit view.
    if let Ok(user) = auth_service::get_user_by_id(pool, user_id).await {
        let _ = audit_service::log_action_with_metadata(
            pool,
            user.company_id,
            Some(user_id),
            "totp_enabled",
            "auth",
            Some(user_id),
            None::<serde_json::Value>,
            Some(serde_json::json!({ "method": "totp" })),
            Some("Two-factor authentication enabled"),
            audit_meta,
        )
        .await;
    }

    Ok(backup_codes)
}

pub async fn is_enabled<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    user_id: Uuid,
) -> AppResult<bool> {
    Ok(user_totp::find_by_user(executor, user_id)
        .await?
        .map(|r| r.enabled)
        .unwrap_or(false))
}

const MAX_2FA_ATTEMPTS: i32 = 5;
const LOCK_2FA_MINUTES: i64 = 15;

/// Verifies a login-time code against a server-tracked single-use MFA challenge
/// (`jti`), locking both `mfa_challenges` and `user_totp` rows so concurrent
/// brute-force attempts or token replays serialize cleanly.
pub async fn verify_login_code_for_challenge(
    pool: &PgPool,
    user_id: Uuid,
    jti: Uuid,
    code: &str,
    encryption_key: &str,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    let Some(challenge) = mfa_challenges::find_for_update(&mut *tx, jti, user_id).await? else {
        return Err(AppError::Unauthorized(
            "Invalid or expired MFA session".into(),
        ));
    };
    if challenge.consumed_at.is_some()
        || challenge.expires_at <= chrono::Utc::now()
        || challenge.attempts >= MAX_2FA_ATTEMPTS
    {
        return Err(AppError::Unauthorized(
            "Invalid or expired MFA session".into(),
        ));
    }

    let row = user_totp::find_by_user_for_update(&mut *tx, user_id)
        .await?
        .filter(|r| r.enabled)
        .ok_or_else(|| AppError::Unauthorized("2FA is not enabled".into()))?;

    if row
        .locked_until
        .is_some_and(|until| until > chrono::Utc::now())
    {
        return Err(AppError::TooManyRequests(
            "Too many failed 2FA attempts. Please try again later.".into(),
        ));
    }

    let trimmed = code.trim();

    if let Ok(totp) =
        crypto::decrypt_secret_for_user(&row.secret_encrypted, encryption_key, user_id)
            .and_then(|secret_b32| build_totp(parse_secret(&secret_b32)?, ""))
    {
        let now_step = chrono::Utc::now().timestamp() / 30;
        let mut matched_step: Option<i64> = None;
        for offset in [-1i64, 0, 1] {
            let step = now_step + offset;
            if step >= 0 {
                let expected = totp.generate((step as u64) * 30).to_string();
                if constant_time_eq(trimmed.as_bytes(), expected.as_bytes()) {
                    matched_step = Some(step);
                }
            }
        }
        if let Some(step) = matched_step {
            if user_totp::claim_totp_step(&mut *tx, user_id, step).await?
                && mfa_challenges::consume(&mut *tx, jti, user_id, MAX_2FA_ATTEMPTS).await?
            {
                tx.commit().await?;
                return Ok(());
            }
            mfa_challenges::record_failed_attempt(&mut *tx, jti, user_id).await?;
            user_totp::record_failure(&mut *tx, user_id, MAX_2FA_ATTEMPTS, LOCK_2FA_MINUTES)
                .await?;
            tx.commit().await?;
            return Err(AppError::Unauthorized("Invalid authentication code".into()));
        }
    }

    let normalized = trimmed.to_ascii_uppercase();
    let lookup_hash = backup_code_lookup_hash(encryption_key, user_id, &normalized);

    let lookup_candidate =
        user_totp_backup_codes::find_unused_by_lookup(&mut *tx, user_id, &lookup_hash).await?;
    let candidates: Vec<(Uuid, String)> = if let Some(candidate) = lookup_candidate {
        vec![(candidate.id, candidate.code_hash)]
    } else {
        user_totp_backup_codes::find_all_unused_for_user(&mut *tx, user_id)
            .await?
            .into_iter()
            .map(|c| (c.id, c.code_hash))
            .collect()
    };

    if candidates.is_empty() {
        mfa_challenges::record_failed_attempt(&mut *tx, jti, user_id).await?;
        user_totp::record_failure(&mut *tx, user_id, MAX_2FA_ATTEMPTS, LOCK_2FA_MINUTES).await?;
        tx.commit().await?;
        return Err(AppError::Unauthorized("Invalid authentication code".into()));
    }

    // Release row locks and the DB connection before CPU-intensive bcrypt verification
    // so concurrent login/2FA traffic does not hold DB connections across bcrypt waits.
    tx.rollback().await?;

    let matched_id = tokio::task::spawn_blocking(move || {
        candidates.into_iter().find_map(|(id, hash)| {
            if bcrypt::verify(&normalized, &hash).unwrap_or(false) {
                Some(id)
            } else {
                None
            }
        })
    })
    .await
    .map_err(|e| AppError::Internal(format!("Backup code verification task failed: {e}")))?;

    let mut tx = pool.begin().await?;
    let Some(challenge) = mfa_challenges::find_for_update(&mut *tx, jti, user_id).await? else {
        return Err(AppError::Unauthorized(
            "Invalid or expired MFA session".into(),
        ));
    };
    if challenge.consumed_at.is_some()
        || challenge.expires_at <= chrono::Utc::now()
        || challenge.attempts >= MAX_2FA_ATTEMPTS
    {
        return Err(AppError::Unauthorized(
            "Invalid or expired MFA session".into(),
        ));
    }
    let row = user_totp::find_by_user_for_update(&mut *tx, user_id)
        .await?
        .filter(|r| r.enabled)
        .ok_or_else(|| AppError::Unauthorized("2FA is not enabled".into()))?;
    if row
        .locked_until
        .is_some_and(|until| until > chrono::Utc::now())
    {
        return Err(AppError::TooManyRequests(
            "Too many failed 2FA attempts. Please try again later.".into(),
        ));
    }

    if let Some(id) = matched_id
        && user_totp_backup_codes::mark_used(&mut *tx, id).await?
        && mfa_challenges::consume(&mut *tx, jti, user_id, MAX_2FA_ATTEMPTS).await?
    {
        user_totp::reset_failures(&mut *tx, user_id).await?;
        tx.commit().await?;
        return Ok(());
    }

    mfa_challenges::record_failed_attempt(&mut *tx, jti, user_id).await?;
    user_totp::record_failure(&mut *tx, user_id, MAX_2FA_ATTEMPTS, LOCK_2FA_MINUTES).await?;
    tx.commit().await?;
    Err(AppError::Unauthorized("Invalid authentication code".into()))
}

/// Verifies a login-time code: a current TOTP code, or (as a fallback) an
/// unused backup code, which is consumed on success. Used only at the
/// post-primary-auth MFA gate.
pub async fn verify_login_code(
    pool: &PgPool,
    user_id: Uuid,
    code: &str,
    encryption_key: &str,
) -> AppResult<()> {
    verify_code_inner(pool, user_id, code, encryption_key).await
}

/// Passwordless `/auth/login/code` variant: verifies the code against its own
/// isolated lockout bucket (`code_login_failed_attempts` /
/// `code_login_locked_until`) so unauthenticated attackers are rate-limited
/// without locking out the user's password + 2FA login flow.
pub async fn verify_code_login(
    pool: &PgPool,
    user_id: Uuid,
    code: &str,
    encryption_key: &str,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    let row = user_totp::find_by_user_for_update(&mut *tx, user_id)
        .await?
        .filter(|r| r.enabled)
        .ok_or_else(|| AppError::Unauthorized("2FA is not enabled".into()))?;

    if row
        .code_login_locked_until
        .is_some_and(|until| until > chrono::Utc::now())
    {
        return Err(AppError::TooManyRequests(
            "Too many failed 2FA attempts. Please try again later.".into(),
        ));
    }

    let trimmed = code.trim();

    if let Ok(totp) =
        crypto::decrypt_secret_for_user(&row.secret_encrypted, encryption_key, user_id)
            .and_then(|secret_b32| build_totp(parse_secret(&secret_b32)?, ""))
    {
        let now_step = chrono::Utc::now().timestamp() / 30;
        let mut matched_step: Option<i64> = None;
        for offset in [-1i64, 0, 1] {
            let step = now_step + offset;
            if step >= 0 {
                let expected = totp.generate((step as u64) * 30).to_string();
                if constant_time_eq(trimmed.as_bytes(), expected.as_bytes()) {
                    matched_step = Some(step);
                }
            }
        }
        if let Some(step) = matched_step {
            if user_totp::claim_code_login_step(&mut *tx, user_id, step).await? {
                tx.commit().await?;
                return Ok(());
            }
            user_totp::record_code_login_failure(
                &mut *tx,
                user_id,
                MAX_2FA_ATTEMPTS,
                LOCK_2FA_MINUTES,
            )
            .await?;
            tx.commit().await?;
            return Err(AppError::Unauthorized("Invalid authentication code".into()));
        }
    }

    let normalized = trimmed.to_ascii_uppercase();
    let lookup_hash = backup_code_lookup_hash(encryption_key, user_id, &normalized);

    let lookup_candidate =
        user_totp_backup_codes::find_unused_by_lookup(&mut *tx, user_id, &lookup_hash).await?;
    let candidates: Vec<(Uuid, String)> = if let Some(candidate) = lookup_candidate {
        vec![(candidate.id, candidate.code_hash)]
    } else {
        user_totp_backup_codes::find_all_unused_for_user(&mut *tx, user_id)
            .await?
            .into_iter()
            .map(|c| (c.id, c.code_hash))
            .collect()
    };

    if candidates.is_empty() {
        user_totp::record_code_login_failure(&mut *tx, user_id, MAX_2FA_ATTEMPTS, LOCK_2FA_MINUTES)
            .await?;
        tx.commit().await?;
        return Err(AppError::Unauthorized("Invalid authentication code".into()));
    }

    // Release row lock and DB connection before bcrypt verification.
    tx.rollback().await?;

    let matched_id = tokio::task::spawn_blocking(move || {
        candidates.into_iter().find_map(|(id, hash)| {
            if bcrypt::verify(&normalized, &hash).unwrap_or(false) {
                Some(id)
            } else {
                None
            }
        })
    })
    .await
    .map_err(|e| AppError::Internal(format!("Backup code verification task failed: {e}")))?;

    let mut tx = pool.begin().await?;
    let row = user_totp::find_by_user_for_update(&mut *tx, user_id)
        .await?
        .filter(|r| r.enabled)
        .ok_or_else(|| AppError::Unauthorized("2FA is not enabled".into()))?;
    if row
        .code_login_locked_until
        .is_some_and(|until| until > chrono::Utc::now())
    {
        return Err(AppError::TooManyRequests(
            "Too many failed 2FA attempts. Please try again later.".into(),
        ));
    }

    if let Some(id) = matched_id
        && user_totp_backup_codes::mark_used(&mut *tx, id).await?
    {
        user_totp::reset_code_login_failures(&mut *tx, user_id).await?;
        tx.commit().await?;
        return Ok(());
    }

    user_totp::record_code_login_failure(&mut *tx, user_id, MAX_2FA_ATTEMPTS, LOCK_2FA_MINUTES)
        .await?;
    tx.commit().await?;
    Err(AppError::Unauthorized("Invalid authentication code".into()))
}

async fn verify_code_inner(
    pool: &PgPool,
    user_id: Uuid,
    code: &str,
    encryption_key: &str,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    let row = user_totp::find_by_user_for_update(&mut *tx, user_id)
        .await?
        .filter(|r| r.enabled)
        .ok_or_else(|| AppError::Unauthorized("2FA is not enabled".into()))?;

    if row
        .locked_until
        .is_some_and(|until| until > chrono::Utc::now())
    {
        return Err(AppError::TooManyRequests(
            "Too many failed 2FA attempts. Please try again later.".into(),
        ));
    }

    let trimmed = code.trim();

    match crypto::decrypt_secret_for_user(&row.secret_encrypted, encryption_key, user_id)
        .and_then(|secret_b32| build_totp(parse_secret(&secret_b32)?, ""))
    {
        Ok(totp) => {
            let now_step = chrono::Utc::now().timestamp() / 30;
            let mut matched_step: Option<i64> = None;
            for offset in [-1i64, 0, 1] {
                let step = now_step + offset;
                if step >= 0 {
                    let expected = totp.generate((step as u64) * 30).to_string();
                    if constant_time_eq(trimmed.as_bytes(), expected.as_bytes()) {
                        matched_step = Some(step);
                    }
                }
            }
            if let Some(step) = matched_step {
                if user_totp::claim_totp_step(&mut *tx, user_id, step).await? {
                    tx.commit().await?;
                    return Ok(());
                }
                user_totp::record_failure(&mut *tx, user_id, MAX_2FA_ATTEMPTS, LOCK_2FA_MINUTES)
                    .await?;
                tx.commit().await?;
                return Err(AppError::Unauthorized("Invalid authentication code".into()));
            }
        }
        Err(error) => {
            tracing::warn!(
                error = %error,
                user_id = %user_id,
                "TOTP verification errored; falling back to backup codes"
            );
        }
    }

    let normalized = trimmed.to_ascii_uppercase();
    let lookup_hash = backup_code_lookup_hash(encryption_key, user_id, &normalized);

    let lookup_candidate =
        user_totp_backup_codes::find_unused_by_lookup(&mut *tx, user_id, &lookup_hash).await?;
    let candidates: Vec<(Uuid, String)> = if let Some(candidate) = lookup_candidate {
        vec![(candidate.id, candidate.code_hash)]
    } else {
        // Fallback scans all unused codes for the user (<= 10 rows) so both
        // pre-lookup legacy rows AND rows indexed under an older rotated key remain usable.
        user_totp_backup_codes::find_all_unused_for_user(&mut *tx, user_id)
            .await?
            .into_iter()
            .map(|c| (c.id, c.code_hash))
            .collect()
    };

    if candidates.is_empty() {
        user_totp::record_failure(&mut *tx, user_id, MAX_2FA_ATTEMPTS, LOCK_2FA_MINUTES).await?;
        tx.commit().await?;
        return Err(AppError::Unauthorized("Invalid authentication code".into()));
    }

    // Release row locks before bcrypt verification so pool connections aren't held idle.
    tx.rollback().await?;

    let matched_id = tokio::task::spawn_blocking(move || {
        candidates.into_iter().find_map(|(id, hash)| {
            if bcrypt::verify(&normalized, &hash).unwrap_or(false) {
                Some(id)
            } else {
                None
            }
        })
    })
    .await
    .map_err(|e| AppError::Internal(format!("Backup code verification task failed: {e}")))?;

    let mut tx = pool.begin().await?;
    let row = user_totp::find_by_user_for_update(&mut *tx, user_id)
        .await?
        .filter(|r| r.enabled)
        .ok_or_else(|| AppError::Unauthorized("2FA is not enabled".into()))?;
    if row
        .locked_until
        .is_some_and(|until| until > chrono::Utc::now())
    {
        return Err(AppError::TooManyRequests(
            "Too many failed 2FA attempts. Please try again later.".into(),
        ));
    }

    if let Some(id) = matched_id
        && user_totp_backup_codes::mark_used(&mut *tx, id).await?
    {
        user_totp::reset_failures(&mut *tx, user_id).await?;
        tx.commit().await?;
        return Ok(());
    }

    user_totp::record_failure(&mut *tx, user_id, MAX_2FA_ATTEMPTS, LOCK_2FA_MINUTES).await?;
    tx.commit().await?;
    Err(AppError::Unauthorized("Invalid authentication code".into()))
}

/// Step-up verification for authenticated sensitive mutations: when 2FA is enabled
/// on the caller's account, requires a valid TOTP or backup code. Returns `400 BadRequest`
/// (or `429 TooManyRequests`) — never `401` — so Axios's 401 interceptor does not
/// sign out the user on a mistyped confirmation code.
pub async fn verify_step_up_if_enabled(
    pool: &PgPool,
    user_id: Uuid,
    totp_code: Option<&str>,
    encryption_key: &str,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    if !is_enabled(pool, user_id).await? {
        return Ok(());
    }

    let trimmed = totp_code
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| AppError::BadRequest("Two-factor authentication code is required".into()))?;

    match verify_code_inner(pool, user_id, trimmed, encryption_key).await {
        Ok(()) => Ok(()),
        Err(AppError::TooManyRequests(msg)) => Err(AppError::TooManyRequests(msg)),
        Err(AppError::Unauthorized(_)) => {
            if let Ok(user) = auth_service::get_user_by_id(pool, user_id).await {
                let _ = audit_service::log_action_with_metadata(
                    pool,
                    user.company_id,
                    Some(user_id),
                    "step_up_totp_failed",
                    "auth",
                    Some(user_id),
                    None::<serde_json::Value>,
                    Some(serde_json::json!({ "reason": "invalid_totp_code" })),
                    Some("Step-up 2FA verification failed"),
                    audit_meta,
                )
                .await;
            }
            Err(AppError::BadRequest("Invalid authentication code".into()))
        }
        Err(other) => Err(other),
    }
}

/// Disables 2FA after re-verifying the current password.
pub async fn disable(
    pool: &PgPool,
    user_id: Uuid,
    password: &str,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    disable_with_session(pool, user_id, None, password, audit_meta).await
}

/// Session-aware variant used by HTTP 2FA removal when password step-up has not yet run.
pub async fn disable_with_session(
    pool: &PgPool,
    user_id: Uuid,
    current_session_id: Option<Uuid>,
    password: &str,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let user = auth_service::verify_user_password_for_session(
        pool,
        user_id,
        current_session_id,
        password,
        audit_meta,
    )
    .await?;
    disable_with_session_verified(pool, &user, current_session_id, audit_meta).await
}

/// Disables 2FA after step-up credentials have already been verified by the caller.
pub async fn disable_with_session_verified(
    pool: &PgPool,
    user: &User,
    current_session_id: Option<Uuid>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let user_id = user.id;
    auth_service::require_two_factor_not_enforced(pool, user).await?;

    let mut tx = pool.begin().await?;
    user_totp_backup_codes::delete_for_user(&mut *tx, user_id).await?;
    let rows = user_totp::delete_for_user(&mut *tx, user_id).await?;
    if rows == 0 {
        return Err(AppError::NotFound("2FA is not enabled".into()));
    }
    mfa_challenges::revoke_for_user(&mut *tx, user_id).await?;
    if let Some(sid) = current_session_id {
        user_sessions::revoke_others(&mut *tx, user_id, sid).await?;
        refresh_tokens::revoke_other_sessions(&mut *tx, user_id, sid).await?;
        push_subscriptions::revoke_for_revoked_sessions(&mut *tx, user_id, sid).await?;
    }
    tx.commit().await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        user.company_id,
        Some(user_id),
        "totp_disabled",
        "auth",
        Some(user_id),
        None::<serde_json::Value>,
        Some(serde_json::json!({ "method": "totp" })),
        Some("Two-factor authentication disabled"),
        audit_meta,
    )
    .await;
    Ok(())
}

/// Regenerates backup codes after re-verifying the current password.
/// Invalidates all previously issued codes.
pub async fn regenerate_backup_codes(
    pool: &PgPool,
    user_id: Uuid,
    password: &str,
    encryption_key: &str,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Vec<String>> {
    regenerate_backup_codes_with_session(pool, user_id, None, password, encryption_key, audit_meta)
        .await
}

/// Session-aware variant used when password step-up has not yet run.
pub async fn regenerate_backup_codes_with_session(
    pool: &PgPool,
    user_id: Uuid,
    current_session_id: Option<Uuid>,
    password: &str,
    encryption_key: &str,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Vec<String>> {
    let user = auth_service::verify_user_password_for_session(
        pool,
        user_id,
        current_session_id,
        password,
        audit_meta,
    )
    .await?;
    regenerate_backup_codes_with_session_verified(
        pool,
        &user,
        current_session_id,
        encryption_key,
        audit_meta,
    )
    .await
}

/// Regenerates backup codes after step-up credentials have already been verified.
pub async fn regenerate_backup_codes_with_session_verified(
    pool: &PgPool,
    user: &User,
    current_session_id: Option<Uuid>,
    encryption_key: &str,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Vec<String>> {
    let user_id = user.id;

    if !is_enabled(pool, user_id).await? {
        return Err(AppError::BadRequest("2FA is not enabled".into()));
    }

    let codes = generate_backup_codes();
    let lookup_hashes: Vec<String> = codes
        .iter()
        .map(|code| backup_code_lookup_hash(encryption_key, user_id, code))
        .collect();
    let hashes = hash_backup_codes(codes.clone()).await?;

    let mut tx = pool.begin().await?;
    let row = user_totp::find_by_user_for_update(&mut *tx, user_id).await?;
    if !row.is_some_and(|r| r.enabled) {
        return Err(AppError::BadRequest("2FA is not enabled".into()));
    }
    user_totp_backup_codes::delete_for_user(&mut *tx, user_id).await?;
    user_totp_backup_codes::insert_many(&mut *tx, user_id, &hashes, &lookup_hashes).await?;
    mfa_challenges::revoke_for_user(&mut *tx, user_id).await?;
    if let Some(sid) = current_session_id {
        user_sessions::revoke_others(&mut *tx, user_id, sid).await?;
        refresh_tokens::revoke_other_sessions(&mut *tx, user_id, sid).await?;
        push_subscriptions::revoke_for_revoked_sessions(&mut *tx, user_id, sid).await?;
    }
    tx.commit().await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        user.company_id,
        Some(user_id),
        "backup_codes_regenerated",
        "auth",
        Some(user_id),
        None::<serde_json::Value>,
        Some(serde_json::json!({
            "count": codes.len(),
            "prior_codes_invalidated": true,
        })),
        Some("Backup codes regenerated"),
        audit_meta,
    )
    .await;
    Ok(codes)
}

/// Startup migration: re-encrypts every stored TOTP secret under
/// `TOTP_ENCRYPTION_KEY`. Rows already under the dedicated key are skipped,
/// rows still under the old `JWT_SECRET`-derived key are migrated, and a row
/// that decrypts under neither is left untouched with a warning — deleting it
/// would silently strip 2FA from an account, and the login path now falls back
/// to backup codes when decryption fails. Idempotent and safe to run on every
/// boot; each row commits independently so partial progress survives a crash.
pub async fn reencrypt_all(
    pool: &PgPool,
    encryption_key: &str,
    legacy_jwt_secret: &str,
) -> AppResult<()> {
    let rows = user_totp::list_all(pool).await?;
    let mut migrated = 0usize;
    let mut unreadable = 0usize;

    for row in rows {
        if row.secret_encrypted.starts_with("v3:")
            && crypto::decrypt_secret_for_user(&row.secret_encrypted, encryption_key, row.user_id)
                .is_ok()
        {
            continue;
        }

        let plaintext =
            crypto::decrypt_secret(&row.secret_encrypted, encryption_key).or_else(|_| {
                crypto::decrypt_with_legacy_jwt_derivation(&row.secret_encrypted, legacy_jwt_secret)
            });

        match plaintext {
            Ok(plaintext) => {
                let encrypted =
                    crypto::encrypt_secret_for_user(&plaintext, encryption_key, row.user_id)?;
                user_totp::update_secret(pool, row.id, &encrypted).await?;
                migrated += 1;
            }
            Err(error) => {
                unreadable += 1;
                tracing::warn!(
                    error = %error,
                    user_id = %row.user_id,
                    "TOTP secret decrypts under neither TOTP_ENCRYPTION_KEY nor the legacy JWT derivation; left as-is"
                );
            }
        }
    }

    if migrated > 0 || unreadable > 0 {
        tracing::info!(
            migrated,
            unreadable,
            "TOTP secret re-encryption pass complete"
        );
    }
    Ok(())
}

/// Break-glass reset: removes a user's TOTP enrolment and backup codes so they
/// can sign in with their password alone, and revokes every live session and
/// refresh token — the caller is asserting the second factor is lost or
/// compromised, so anything it was guarding must die with it.
///
/// The acting super_admin re-presents their own password here so a hijacked
/// super_admin session cannot strip 2FA off other accounts on its own.
/// The whole operation, including its audit row, is one transaction: either
/// the account is unlocked *and* the trail records who did it, or neither
/// happens.
pub async fn admin_reset(
    pool: &PgPool,
    target_user_id: Uuid,
    actor_user_id: Uuid,
    actor_password: &str,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    verify_password(pool, actor_user_id, actor_password, audit_meta).await?;
    admin_reset_verified(pool, target_user_id, actor_user_id, audit_meta).await
}

/// Break-glass reset after the acting `super_admin`'s step-up credentials have
/// already been verified in the handler.
pub async fn admin_reset_verified(
    pool: &PgPool,
    target_user_id: Uuid,
    actor_user_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    if target_user_id == actor_user_id {
        // Not a safety property (the normal disable flow still requires only
        // the user's own credentials) but a reset aimed at yourself is always
        // a mistake — that is what `POST /auth/2fa/disable` is for.
        return Err(AppError::BadRequest(
            "Use 2FA settings to disable your own 2FA".into(),
        ));
    }

    let mut tx = pool.begin().await?;

    let existing = user_totp::find_by_user(&mut *tx, target_user_id).await?;
    let was_enabled = existing.as_ref().map(|r| r.enabled).unwrap_or(false);
    if existing.is_none() {
        return Err(AppError::NotFound(
            "2FA is not enabled for this user".into(),
        ));
    }

    user_totp_backup_codes::delete_for_user(&mut *tx, target_user_id).await?;
    user_totp::delete_for_user(&mut *tx, target_user_id).await?;
    user_sessions::revoke_all_for_user(&mut *tx, target_user_id).await?;
    refresh_tokens::revoke_all_for_user(&mut *tx, target_user_id).await?;
    mfa_challenges::revoke_for_user(&mut *tx, target_user_id).await?;
    push_subscriptions::revoke_all_for_user(&mut *tx, target_user_id).await?;

    // Scoped to the target's company so the reset shows up in that tenant's
    // own audit view, not just in a platform-wide query.
    let target_company = crate::repositories::users::get_by_id(&mut *tx, target_user_id)
        .await?
        .and_then(|u| u.company_id);

    // Deliberately not best-effort-and-dropped: this call site writes inside
    // the transaction, so a failure here rolls the unlock back rather than
    // leaving an untraceable credential change behind.
    audit_service::log_action_with_metadata(
        &mut *tx,
        target_company,
        Some(actor_user_id),
        "reset",
        "user_totp",
        Some(target_user_id),
        Some(serde_json::json!({ "enabled": was_enabled })),
        None::<serde_json::Value>,
        Some("2FA break-glass reset by super_admin; sessions revoked"),
        audit_meta,
    )
    .await?;

    tx.commit().await?;
    Ok(())
}
