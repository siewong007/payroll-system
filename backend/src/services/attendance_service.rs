use std::future::Future;
use std::time::Duration;

use chrono::{Datelike, Utc};
use chrono_tz::Tz;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;
use webauthn_rs::Webauthn;
use webauthn_rs::prelude::{PasskeyAuthentication, PublicKeyCredential};

use crate::core::error::{AppError, AppResult};
use crate::core::timezone;
use crate::models::attendance::{
    AttendanceExportQuery, AttendanceListQuery, AttendanceMethodResponse, AttendanceRecord,
    AttendanceRecordWithEmployee, AttendanceSummaryItem, AttendanceSummaryQuery,
    FaceIdBeginResponse, ManualAttendanceRequest, PaginatedAttendance, QrTokenResponse,
    UpdateAttendanceRecordRequest,
};
use crate::models::attendance_kiosk::KioskCredential;
use crate::repositories::reads::attendance as attendance_reads;
use crate::repositories::{
    attendance_kiosk_credentials, attendance_qr_tokens, attendance_records, audit_logs, clock,
    companies, company_work_schedules, employee_work_schedules, employees, passkey_challenges,
    platform_settings,
};
use crate::services::audit_service::{self, AuditRequestMeta};
use crate::services::{csv_helpers, geofence_service, passkey_service, settings_service};

// ─── QR Token TTL ───
const QR_TOKEN_TTL_SECONDS: i64 = 300;

fn normalize_absent_check_out(
    status: &str,
    check_in_at: chrono::DateTime<Utc>,
    check_out_at: Option<chrono::DateTime<Utc>>,
) -> Option<chrono::DateTime<Utc>> {
    if status == "absent" {
        Some(check_out_at.unwrap_or(check_in_at))
    } else {
        check_out_at
    }
}

// ─── Platform Settings ───

/// The company's effective attendance method, resolved from platform settings
/// and the optional company override. Lighter than the client-facing
/// `AttendanceMethodResponse` — the check-in hot path only needs these.
pub struct EffectiveMethod {
    pub method: String,
    pub allow_company_override: bool,
    pub is_company_override: bool,
}

pub async fn get_platform_attendance_method(pool: &PgPool) -> AppResult<String> {
    let method = platform_settings::get_attendance_method(pool).await?;
    Ok(method.unwrap_or_else(|| "qr_code".to_string()))
}

pub async fn get_platform_allow_override(pool: &PgPool) -> AppResult<bool> {
    let val = platform_settings::get_allow_override(pool).await?;
    Ok(val.map(|v| v == "true").unwrap_or(false))
}

pub async fn set_platform_attendance_method(
    pool: &PgPool,
    method: &str,
    allow_override: bool,
    updated_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    if method != "qr_code" && method != "face_id" {
        return Err(AppError::BadRequest(
            "Method must be 'qr_code' or 'face_id'".into(),
        ));
    }

    let old_method = get_platform_attendance_method(pool).await?;
    let old_allow_override = get_platform_allow_override(pool).await?;

    platform_settings::set_attendance_method(pool, method, updated_by).await?;
    platform_settings::set_allow_override(
        pool,
        if allow_override { "true" } else { "false" },
        updated_by,
    )
    .await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        None, // Platform-level setting, not scoped to a company
        Some(updated_by),
        "update",
        "platform_attendance_method",
        None,
        Some(serde_json::json!({
            "method": old_method,
            "allow_company_override": old_allow_override,
        })),
        Some(serde_json::json!({
            "method": method,
            "allow_company_override": allow_override,
        })),
        Some("Platform attendance method updated"),
        audit_meta,
    )
    .await;

    Ok(())
}

/// Get the effective attendance method for a company (company override > platform default)
pub async fn get_effective_method(pool: &PgPool, company_id: Uuid) -> AppResult<EffectiveMethod> {
    let (platform_method, allow_override_raw) =
        platform_settings::get_attendance_settings(pool).await?;
    let platform_method = platform_method.unwrap_or_else(|| "qr_code".to_string());
    let allow_override = allow_override_raw.map(|v| v == "true").unwrap_or(false);

    // Check if company has an override
    let company_method = companies::get_attendance_method(pool, company_id).await?;

    let (method, is_override) = if allow_override {
        if let Some(m) = company_method {
            (m, true)
        } else {
            (platform_method, false)
        }
    } else {
        (platform_method, false)
    };

    Ok(EffectiveMethod {
        method,
        allow_company_override: allow_override,
        is_company_override: is_override,
    })
}

/// Client-facing attendance bootstrap: the effective method plus the geofence
/// mode (clients skip the GPS wait when it is 'none') and the company
/// timezone (clients compute "today" on the company calendar).
pub async fn get_attendance_bootstrap(
    pool: &PgPool,
    company_id: Uuid,
) -> AppResult<AttendanceMethodResponse> {
    let effective = get_effective_method(pool, company_id).await?;
    let geofence_mode = geofence_service::get_geofence_mode(pool, company_id).await?;
    let timezone = get_company_timezone(pool, company_id).await;

    Ok(AttendanceMethodResponse {
        method: effective.method,
        allow_company_override: effective.allow_company_override,
        is_company_override: effective.is_company_override,
        geofence_mode,
        timezone,
    })
}

pub async fn set_company_attendance_method(
    pool: &PgPool,
    company_id: Uuid,
    method: Option<&str>,
    updated_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    // Verify overrides are allowed
    let allow_override = get_platform_allow_override(pool).await?;
    if !allow_override {
        return Err(AppError::Forbidden(
            "Company-level attendance method override is disabled by super admin".into(),
        ));
    }

    if let Some(m) = method
        && m != "qr_code"
        && m != "face_id"
    {
        return Err(AppError::BadRequest(
            "Method must be 'qr_code' or 'face_id'".into(),
        ));
    }

    let old_method = companies::get_attendance_method(pool, company_id).await?;

    companies::set_attendance_method(pool, company_id, method).await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(updated_by),
        "update",
        "company_attendance_method",
        Some(company_id),
        Some(serde_json::json!({ "method": old_method })),
        Some(serde_json::json!({ "method": method })),
        Some("Company attendance method updated"),
        audit_meta,
    )
    .await;

    Ok(())
}

// ─── QR Token Management ───

/// Issues a fresh QR token, retiring the previous one from the same display
/// surface. `kiosk_credential_id` is `None` for the admin console; each kiosk
/// retires only its own tokens, so kiosks on staggered refresh cycles no longer
/// revoke each other's still-displayed codes.
pub async fn generate_qr_token(
    pool: &PgPool,
    company_id: Uuid,
    frontend_url: &str,
    kiosk_credential_id: Option<Uuid>,
) -> AppResult<QrTokenResponse> {
    attendance_qr_tokens::revoke_unused_for_issuer(pool, company_id, kiosk_credential_id).await?;

    let token = Uuid::new_v4().to_string().replace('-', "");
    let expires_at = Utc::now() + chrono::Duration::seconds(QR_TOKEN_TTL_SECONDS);

    attendance_qr_tokens::insert(pool, company_id, &token, expires_at, kiosk_credential_id).await?;

    let scan_url = format!("{}/attendance/scan?token={}", frontend_url, token);

    Ok(QrTokenResponse {
        token,
        expires_at,
        scan_url,
        ttl_seconds: QR_TOKEN_TTL_SECONDS,
    })
}

/// Validate a QR token without consuming it — multiple employees may check in with the
/// same active token during its TTL window. The `used` flag means admin-revoked (a new
/// token was generated), not employee-scanned.
pub async fn validate_qr_token(pool: &PgPool, token: &str, company_id: Uuid) -> AppResult<Uuid> {
    let row = attendance_qr_tokens::find_by_token(pool, token).await?;

    match row {
        None => Err(AppError::BadRequest(
            "Invalid QR code: token not found".into(),
        )),
        Some(t) if t.company_id != company_id => Err(AppError::BadRequest(
            "Invalid QR code: this code belongs to a different company".into(),
        )),
        Some(t) if t.used => Err(AppError::BadRequest(
            "This QR code has been revoked — please refresh the kiosk screen.".into(),
        )),
        Some(t) if t.expires_at < Utc::now() => Err(AppError::BadRequest(
            "QR code has expired — please refresh the kiosk screen.".into(),
        )),
        Some(t) => Ok(t.id),
    }
}

// ─── Face-ID Check-in Ceremony ───

/// Challenge type for face-id check-in ceremonies. Distinct from the login
/// ceremony types so a check-in challenge cannot complete a login or vice
/// versa; the stored row is additionally bound to the requesting user.
const FACE_ID_CHALLENGE_TYPE: &str = "attendance_face_id";

/// Start a WebAuthn assertion ceremony for a face-id check-in, scoped to the
/// authenticated user's registered passkeys.
pub async fn face_id_begin(
    pool: &PgPool,
    webauthn: &Webauthn,
    user_id: Uuid,
) -> AppResult<FaceIdBeginResponse> {
    let passkeys = passkey_service::get_passkeys_for_user(pool, user_id).await?;
    if passkeys.is_empty() {
        return Err(AppError::BadRequest(
            "No passkeys registered. Add a passkey in your profile to use Face ID check-in.".into(),
        ));
    }

    let (rcr, auth_state) = webauthn
        .start_passkey_authentication(&passkeys)
        .map_err(|e| AppError::Internal(format!("WebAuthn auth start failed: {}", e)))?;

    let state_json = serde_json::to_value(&auth_state)
        .map_err(|e| AppError::Internal(format!("Failed to serialize auth state: {}", e)))?;

    let challenge_id = passkey_service::store_challenge(
        pool,
        Some(user_id),
        None,
        FACE_ID_CHALLENGE_TYPE,
        &state_json,
    )
    .await?;

    Ok(FaceIdBeginResponse {
        challenge_id,
        options: rcr,
    })
}

/// Verify a face-id check-in assertion server-side. Consumes the challenge
/// (which must belong to this user) and verifies the assertion against the
/// user's registered passkeys — a face-id check-in without a live biometric
/// ceremony is rejected rather than trusted.
pub async fn face_id_verify(
    pool: &PgPool,
    webauthn: &Webauthn,
    user_id: Uuid,
    challenge_id: Uuid,
    credential: &PublicKeyCredential,
) -> AppResult<()> {
    let state_json =
        passkey_challenges::consume_for_user(pool, challenge_id, FACE_ID_CHALLENGE_TYPE, user_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("Face ID challenge expired or not found".into()))?;

    let auth_state: PasskeyAuthentication = serde_json::from_value(state_json)
        .map_err(|e| AppError::Internal(format!("Invalid stored auth state: {}", e)))?;

    let auth_result = webauthn
        .finish_passkey_authentication(credential, &auth_state)
        .map_err(|e| AppError::Unauthorized(format!("Face ID verification failed: {}", e)))?;

    // Persist the updated signature counter, mirroring the login flow.
    let mut passkeys = passkey_service::get_passkeys_for_user(pool, user_id).await?;
    for pk in passkeys.iter_mut() {
        if pk.cred_id() == auth_result.cred_id() {
            pk.update_credential(&auth_result);
            passkey_service::update_passkey_after_auth(pool, user_id, pk).await?;
            break;
        }
    }

    Ok(())
}

// ─── Kiosk Credentials (public-URL kiosk display) ───

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let out = hasher.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out.iter() {
        use std::fmt::Write;
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}

/// Mint a kiosk credential. Returns the model and the plaintext secret. The plaintext
/// is the only chance the caller has to learn the secret — the server stores only its
/// hash. Caller must surface it to the admin once and then drop it.
pub async fn create_kiosk_credential(
    pool: &PgPool,
    company_id: Uuid,
    label: &str,
    created_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<(KioskCredential, String)> {
    let label = label.trim();
    if label.is_empty() || label.len() > 100 {
        return Err(AppError::BadRequest(
            "Label must be 1–100 characters".into(),
        ));
    }

    // 64 hex chars = ~244 bits of entropy (2 × Uuid v4). Mirrors the existing token
    // shape used by `generate_qr_token`. Way beyond brute-forceable, especially with
    // the route-level rate limit.
    let secret = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let token_hash = sha256_hex(&secret);
    let token_prefix = secret[..8].to_string();

    let cred = attendance_kiosk_credentials::insert(
        pool,
        company_id,
        label,
        &token_hash,
        &token_prefix,
        created_by,
    )
    .await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(created_by),
        "create",
        "attendance_kiosk_credential",
        Some(cred.id),
        None,
        Some(serde_json::json!({
            "id": cred.id,
            "company_id": cred.company_id,
            "label": cred.label,
            "token_prefix": cred.token_prefix,
        })),
        Some("Attendance kiosk credential created"),
        audit_meta,
    )
    .await;

    Ok((cred, secret))
}

pub async fn list_kiosk_credentials(
    pool: &PgPool,
    company_id: Uuid,
) -> AppResult<Vec<KioskCredential>> {
    attendance_kiosk_credentials::list_for_company(pool, company_id).await
}

pub async fn revoke_kiosk_credential(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
    revoked_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let existing = attendance_kiosk_credentials::list_for_company(pool, company_id)
        .await?
        .into_iter()
        .find(|credential| credential.id == id && credential.revoked_at.is_none())
        .ok_or_else(|| {
            AppError::NotFound("Kiosk credential not found or already revoked".into())
        })?;

    let revoked = attendance_kiosk_credentials::revoke(pool, id, company_id).await?;
    if !revoked {
        return Err(AppError::NotFound(
            "Kiosk credential not found or already revoked".into(),
        ));
    }

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(revoked_by),
        "revoke",
        "attendance_kiosk_credential",
        Some(id),
        Some(serde_json::json!({
            "id": existing.id,
            "company_id": existing.company_id,
            "label": existing.label,
            "token_prefix": existing.token_prefix,
            "revoked_at": existing.revoked_at,
        })),
        Some(serde_json::json!({ "revoked": true })),
        Some("Attendance kiosk credential revoked"),
        audit_meta,
    )
    .await;

    Ok(())
}

/// Validate a kiosk secret presented by an unauthenticated tablet, then mint a fresh
/// QR token for that kiosk's company. Reuses `generate_qr_token` so the QR rotation
/// behaviour stays identical across the admin-logged-in flow and the public flow.
///
/// SECURITY: never log the presented secret. On rejection, sleep briefly to flatten
/// any timing variance between "no such hash" and "found but revoked".
pub async fn generate_qr_via_kiosk(
    pool: &PgPool,
    presented_secret: &str,
    frontend_url: &str,
    client_ip: Option<&str>,
) -> AppResult<(QrTokenResponse, Uuid)> {
    let hash = sha256_hex(presented_secret);
    let cred = attendance_kiosk_credentials::find_active_by_hash(pool, &hash).await?;

    let cred = match cred {
        Some(c) => c,
        None => {
            tokio::time::sleep(Duration::from_millis(150)).await;
            return Err(AppError::Unauthorized("Invalid kiosk credential".into()));
        }
    };

    let resp = generate_qr_token(pool, cred.company_id, frontend_url, Some(cred.id)).await?;

    // Best-effort heartbeat; failure to record this should not block the kiosk.
    if let Err(e) = attendance_kiosk_credentials::mark_used(pool, cred.id, client_ip).await {
        tracing::warn!("Failed to update kiosk last_used: {}", e);
    }

    Ok((resp, cred.company_id))
}

// ─── Auto Late Detection ───

/// Determine attendance status based on the company's work schedule.
/// Returns "present" or "late".
///
/// Errors propagate: a transient DB failure used to silently degrade every
/// lookup (wrong day-of-week, UTC wall clock, schedule treated as absent),
/// recording a wrong status with no trace. Check-in fails visibly instead —
/// the employee retries with the still-valid QR.
pub(crate) async fn determine_checkin_status(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    tz: &str,
) -> AppResult<String> {
    // Day of week (0=Sunday, 6=Saturday) and local time per the DB clock, in
    // one round trip.
    let (dow, now_local) = clock::dow_and_time_in_tz(pool, tz).await?;

    // 1. Try employee-specific schedule, 2. fall back to company default.
    let timing = match employee_work_schedules::find_timing_for_day(pool, employee_id, dow).await? {
        Some(t) => Some(t),
        None => company_work_schedules::find_default_timing(pool, company_id).await?,
    };

    let Some((start_time, grace_minutes)) = timing else {
        // No schedule configured at all — nothing to be late against.
        return Ok("present".to_string());
    };

    let cutoff = start_time + chrono::Duration::minutes(grace_minutes as i64);

    Ok(if now_local > cutoff {
        "late".to_string()
    } else {
        "present".to_string()
    })
}

/// Get the timezone for a company from its work schedule (fallback to default).
///
/// Sanitized, not trusted: the column predates the write-side validator, so a
/// value already stored that no longer parses degrades to the default here
/// rather than reaching `AT TIME ZONE` and 500-ing every check-in, summary and
/// export for that tenant until an operator edits the row by hand.
async fn get_company_timezone(pool: &PgPool, company_id: Uuid) -> String {
    timezone::sanitize(
        company_work_schedules::find_default_timezone(pool, company_id)
            .await
            .unwrap_or(None),
    )
}

// ─── Check In / Check Out ───

pub async fn check_in_qr(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    token: &str,
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> AppResult<AttendanceRecord> {
    let tz = get_company_timezone(pool, company_id).await;
    ensure_no_active_checkin(pool, employee_id, &tz).await?;

    // Geofence check (may reject in enforce mode)
    let outside_geofence =
        geofence_service::validate_geofence(pool, company_id, latitude, longitude).await?;

    let token_id = validate_qr_token(pool, token, company_id).await?;
    let status = determine_checkin_status(pool, employee_id, company_id, &tz).await?;

    insert_checkin(pool, employee_id, &tz, || async {
        let mut tx = pool.begin().await?;
        let record = attendance_records::insert_qr(
            &mut *tx,
            company_id,
            employee_id,
            &status,
            latitude,
            longitude,
            token_id,
            outside_geofence,
        )
        .await?;
        // A post-cron check-in supersedes the day's auto-absent placeholder —
        // atomically, so the day never counts as both absent and late.
        attendance_records::delete_auto_absent_today(&mut *tx, employee_id, &tz).await?;
        tx.commit().await?;
        Ok(record)
    })
    .await
}

pub async fn check_in_face_id(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> AppResult<AttendanceRecord> {
    let tz = get_company_timezone(pool, company_id).await;
    ensure_no_active_checkin(pool, employee_id, &tz).await?;

    // Geofence check
    let outside_geofence =
        geofence_service::validate_geofence(pool, company_id, latitude, longitude).await?;

    let status = determine_checkin_status(pool, employee_id, company_id, &tz).await?;

    insert_checkin(pool, employee_id, &tz, || async {
        let mut tx = pool.begin().await?;
        let record = attendance_records::insert_face(
            &mut *tx,
            company_id,
            employee_id,
            &status,
            latitude,
            longitude,
            outside_geofence,
        )
        .await?;
        attendance_records::delete_auto_absent_today(&mut *tx, employee_id, &tz).await?;
        tx.commit().await?;
        Ok(record)
    })
    .await
}

/// Run a check-in insert closure, translating the one-open-session unique
/// violation into the shared conflict resolution.
async fn insert_checkin<F, Fut>(
    pool: &PgPool,
    employee_id: Uuid,
    tz: &str,
    insert: F,
) -> AppResult<AttendanceRecord>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = AppResult<AttendanceRecord>>,
{
    match insert().await {
        Ok(record) => Ok(record),
        Err(AppError::Database(sqlx::Error::Database(db_err)))
            if db_err.code().as_deref() == Some("23505") =>
        {
            resolve_open_checkin_conflict(pool, employee_id, tz).await
        }
        Err(e) => Err(e),
    }
}

pub async fn check_out(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> AppResult<AttendanceRecord> {
    // Never blocks: an off-site (or GPS-less) check-out is flagged for admin
    // review instead of refused, so employees cannot be trapped in an open
    // session that only an admin correction can close.
    let outside_geofence =
        geofence_service::flag_geofence_for_checkout(pool, company_id, latitude, longitude).await?;

    // Above this many derived overtime hours the figure is treated as a
    // forgotten check-out and left unrated for HR rather than paid. The decision
    // is per-tenant configuration, so it is read here; the repository only
    // applies the bound it is given.
    let max_overtime_hours = settings_service::overtime_settings(pool, company_id)
        .await
        .max_overtime_hours_per_day;

    if let Some(record) = attendance_records::check_out(
        pool,
        employee_id,
        latitude,
        longitude,
        company_id,
        outside_geofence,
        max_overtime_hours,
    )
    .await?
    {
        return Ok(record);
    }

    // Nothing matched the 24h window. If an older session in this company is
    // still open, say so — the old "please check in first" advice bounced the
    // employee into the one-open-session conflict and straight back here.
    // Open sessions in *other* companies are not surfaced.
    let tz = get_company_timezone(pool, company_id).await;
    match attendance_records::find_open_with_local_date(pool, employee_id, &tz).await? {
        Some((record, local_date, _)) if record.company_id == company_id => {
            Err(AppError::BadRequest(format!(
                "Your session from {local_date} is more than 24 hours old and can no longer be closed here. Ask an administrator to correct it."
            )))
        }
        _ => Err(AppError::BadRequest(
            "No active check-in found. Please check in before checking out.".into(),
        )),
    }
}

/// Prevent double check-in on the same calendar day (using company timezone)
/// Resolves a unique-index violation on `attendance_one_open_per_employee`.
///
/// A same-day double-tap is a genuine race, so returning the existing record is
/// right. An open record left over from an earlier day is not: the pre-check
/// `ensure_no_active_checkin` only looks at *today*, so that stale row silently
/// blocks today's INSERT. Returning it would report success while no record
/// exists for today — the auto-absent cron then marks the employee absent, and
/// check-out fails too once the row is more than 24 hours old. Surface it
/// instead so it can be closed or corrected.
async fn resolve_open_checkin_conflict(
    pool: &PgPool,
    employee_id: Uuid,
    tz: &str,
) -> AppResult<AttendanceRecord> {
    let (record, local_date, is_today) =
        attendance_records::find_open_with_local_date(pool, employee_id, tz)
            .await?
            .ok_or_else(|| AppError::BadRequest("You already have an active check-in.".into()))?;

    if is_today {
        return Ok(record);
    }

    Err(AppError::BadRequest(format!(
        "You have a check-in from {local_date} that was never checked out. Check out from that session, or ask an administrator to correct it, before checking in today."
    )))
}

async fn ensure_no_active_checkin(pool: &PgPool, employee_id: Uuid, tz: &str) -> AppResult<()> {
    if attendance_records::exists_active_checkin_today(pool, employee_id, tz).await? {
        return Err(AppError::BadRequest(
            "You have already checked in today. Please check out first.".into(),
        ));
    }
    Ok(())
}

// ─── List / Query ───

pub async fn list_attendance(
    pool: &PgPool,
    company_id: Uuid,
    q: &AttendanceListQuery,
) -> AppResult<PaginatedAttendance<AttendanceRecordWithEmployee>> {
    let tz = get_company_timezone(pool, company_id).await;
    attendance_reads::list_with_employee(pool, company_id, &tz, q).await
}

pub async fn get_my_attendance(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    q: &AttendanceListQuery,
) -> AppResult<PaginatedAttendance<AttendanceRecord>> {
    let tz = get_company_timezone(pool, company_id).await;
    attendance_reads::list_for_employee(pool, employee_id, &tz, q).await
}

/// Get today's check-in for the current employee (if any)
pub async fn get_today_checkin(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<AttendanceRecord>> {
    let tz = get_company_timezone(pool, company_id).await;
    attendance_records::get_today(pool, employee_id, &tz).await
}

pub async fn manual_attendance(
    pool: &PgPool,
    company_id: Uuid,
    req: ManualAttendanceRequest,
    created_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<AttendanceRecord> {
    // The employee must belong to the caller's company. The composite tenant
    // FK also blocks a cross-tenant insert, but as an opaque 500 — and a
    // cross-tenant *open* record would occupy the victim's one-open-session
    // slot. Reject explicitly with an actionable error instead.
    if !employees::exists_in_company(pool, req.employee_id, company_id).await? {
        return Err(AppError::NotFound(
            "Employee not found in your company".into(),
        ));
    }

    let status = req.status.as_deref().unwrap_or("present");
    if !matches!(status, "present" | "late" | "absent" | "half_day") {
        return Err(AppError::BadRequest(
            "Status must be 'present', 'late', 'absent', or 'half_day'".into(),
        ));
    }
    let check_out_at = normalize_absent_check_out(status, req.check_in_at, req.check_out_at);
    if check_out_at.is_some_and(|check_out| check_out < req.check_in_at) {
        return Err(AppError::BadRequest(
            "Check-out time must not be before check-in time".into(),
        ));
    }

    let record = attendance_records::insert_manual(
        pool,
        company_id,
        req.employee_id,
        req.check_in_at,
        check_out_at,
        status,
        req.notes.as_deref(),
        created_by,
    )
    .await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(created_by),
        "create",
        "attendance_record",
        Some(record.id),
        None,
        Some(serde_json::to_value(&record).unwrap_or_default()),
        Some("Manual attendance record created"),
        audit_meta,
    )
    .await;

    Ok(record)
}

// ─── Attendance Correction ───

/// Reject a correction whose two timestamps describe a session longer than one
/// day. Pure, so the boundary is testable without a database.
///
/// This does change one workflow: HR closing a three-week-old open session must
/// now correct the check-in as well, rather than only supplying a check-out.
/// That is the right outcome — recording 500 hours worked was never correct —
/// so the message names both remedies.
fn validate_correction_span(
    check_in: chrono::DateTime<Utc>,
    check_out: Option<chrono::DateTime<Utc>>,
) -> AppResult<()> {
    let Some(check_out) = check_out else {
        return Ok(());
    };
    if check_out - check_in > chrono::Duration::hours(MAX_CORRECTION_SPAN_HOURS) {
        return Err(AppError::BadRequest(format!(
            "A single attendance session cannot span more than {MAX_CORRECTION_SPAN_HOURS} hours. \
             Correct the check-in time as well, or clear the check-out to leave the session open."
        )));
    }
    Ok(())
}

pub async fn update_attendance_record(
    pool: &PgPool,
    company_id: Uuid,
    record_id: Uuid,
    req: &UpdateAttendanceRecordRequest,
    updated_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<AttendanceRecord> {
    let reason = req.reason.trim();
    if reason.is_empty() {
        return Err(AppError::BadRequest(
            "A reason for the correction is required".into(),
        ));
    }
    // The reason is concatenated into audit_logs.description, which is
    // varchar(500). Reject over-long input with an actionable 400 rather than
    // letting the INSERT raise 22001 and roll back the correction as a 500.
    if reason.chars().count() > MAX_CORRECTION_REASON_CHARS {
        return Err(AppError::BadRequest(format!(
            "Reason must be {MAX_CORRECTION_REASON_CHARS} characters or fewer"
        )));
    }

    // Fetch existing record
    let existing = attendance_records::get_by_id(pool, record_id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Attendance record not found".into()))?;

    let check_in = req.check_in_at.unwrap_or(existing.check_in_at);
    let status = req.status.as_deref().unwrap_or(&existing.status);

    // Tri-state per field: value = set, clear flag = clear, neither = keep.
    let notes = if req.clear_notes.unwrap_or(false) {
        None
    } else {
        req.notes.as_deref().or(existing.notes.as_deref())
    };
    let check_out = if req.clear_check_out.unwrap_or(false) {
        // Reopening the session; normalize_absent_check_out below re-closes
        // absent rows, which must never be open.
        None
    } else {
        req.check_out_at.or(existing.check_out_at)
    };
    let check_out = normalize_absent_check_out(status, check_in, check_out);

    // Validate status
    if !matches!(status, "present" | "late" | "absent" | "half_day") {
        return Err(AppError::BadRequest(
            "Status must be 'present', 'late', 'absent', or 'half_day'".into(),
        ));
    }

    if check_out.is_some_and(|value| value < check_in) {
        return Err(AppError::BadRequest(
            "Check-out time must not be before check-in time".into(),
        ));
    }

    validate_correction_span(check_in, check_out)?;

    // Update and audit atomically: a correction to a payroll-feeding record
    // must never exist without its audit row, and vice versa.
    let mut tx = pool.begin().await?;

    let record = match attendance_records::update(
        &mut *tx, record_id, company_id, check_in, check_out, status, notes,
    )
    .await
    {
        Ok(record) => record,
        Err(AppError::Database(sqlx::Error::Database(db_err))) => {
            // Owned, so the fall-through arm can still hand the original error
            // back rather than borrowing it for the length of the match.
            let code = db_err.code().map(|code| code.into_owned());
            // The derived-hours expressions can only fail in ways an operator
            // can act on, so none of them may surface as a bare 500 that also
            // rolls the correction back with no explanation.
            return Err(match code.as_deref() {
                // Reopening a session (clearing its check-out) collides with
                // the one-open-session-per-employee index when the employee
                // already has another open record.
                Some("23505") => AppError::Conflict(
                    "This employee already has another open session. Close that one before reopening this record.".into(),
                ),
                // numeric_value_out_of_range: hours_worked/overtime_hours are
                // numeric(5,2), so a span past ~41.7 days overflows. Unreachable
                // behind the 24 h guard above; kept so no future arithmetic can
                // reintroduce the 500.
                Some("22003") => AppError::BadRequest(
                    "The corrected times produce an implausible number of hours worked. Check the check-in and check-out dates.".into(),
                ),
                // check_violation: attendance_records_hours_check and the
                // overtime-within-hours constraint added by 1012.
                Some("23514") => AppError::BadRequest(
                    "The corrected times fail an attendance consistency rule (hours worked and overtime must both be plausible). Check the check-in and check-out dates.".into(),
                ),
                _ => AppError::Database(sqlx::Error::Database(db_err)),
            });
        }
        Err(e) => return Err(e),
    };

    let old_vals = serde_json::json!({
        "check_in_at": existing.check_in_at,
        "check_out_at": existing.check_out_at,
        "status": existing.status,
        "notes": existing.notes,
    });
    let new_vals = serde_json::json!({
        "check_in_at": record.check_in_at,
        "check_out_at": record.check_out_at,
        "status": record.status,
        "notes": record.notes,
        "reason": reason,
    });

    audit_logs::insert(
        &mut *tx,
        Some(company_id),
        Some(updated_by),
        "update",
        "attendance_record",
        Some(record_id),
        Some(old_vals),
        Some(new_vals),
        Some(&format!("Attendance record corrected: {reason}")),
        audit_meta.and_then(|meta| meta.ip_address.as_deref()),
        audit_meta.and_then(|meta| meta.user_agent.as_deref()),
    )
    .await?;

    tx.commit().await?;

    Ok(record)
}

// ─── Auto-Absent Marking ───

/// Never backfill further than this; on a first run (no bookmark) only the
/// current day is considered. Per company, so one wedged tenant can no longer
/// burn the window for everybody else.
const AUTO_ABSENT_MAX_BACKFILL_DAYS: i64 = 14;
/// Daily cutoff, in each company's *own* local time. The catch-up only treats
/// *today* as due after this — marking absences at 09:00 would flag everyone
/// who simply hasn't arrived yet.
const AUTO_ABSENT_CUTOFF: (u32, u32) = (12, 30);

/// Bound on the correction reason. It is prefixed and stored in
/// `audit_logs.description` (varchar(500)); leaving room for the prefix keeps
/// a long reason a 400, not a failed transaction.
const MAX_CORRECTION_REASON_CHARS: usize = 400;

/// Longest span a single corrected session may cover.
///
/// Not an arbitrary number: it is the invariant the rest of the system already
/// enforces. Check-out only matches an open record inside 24 hours, and an
/// employee with an older open session is told to ask an administrator — "a
/// session is at most one day" is the documented model, and the correction path
/// was the one place that had escaped it. Unbounded, it wrote `hours_worked`
/// straight past `numeric(5,2)` (a 500 that also rolled the correction back),
/// and — worse, because it is silent — a 40-day "session" just under that
/// ceiling committed cleanly with ~951 payable hours in `overtime_hours`, which
/// feeds payroll.
const MAX_CORRECTION_SPAN_HOURS: i64 = 24;

/// How far back an admin may run the absence backfill. Bounds an accidental
/// (or hostile) request for an arbitrary historical date.
const MAX_ABSENT_BACKFILL_DAYS: i64 = 90;

/// Mark active employees as absent for one local calendar date if they have
/// no attendance record on it. Respects working day config, holidays, and
/// approved leave. Idempotent per date. `company_id` scopes an admin-run
/// backfill to that tenant; `None` (the daily job) covers all companies.
pub async fn mark_absent_for_date(
    pool: &PgPool,
    tz: &str,
    date: chrono::NaiveDate,
    company_id: Option<Uuid>,
) -> AppResult<i64> {
    Ok(attendance_records::mark_absent(pool, tz, date, company_id).await? as i64)
}

/// Admin backfill for one company and one past local date. Resolves the
/// company's own timezone so the placeholder lands on the same calendar the
/// reads (and `delete_auto_absent_today`) bucket by — hardcoding MYT here
/// would write the row on a different day than the one being corrected.
pub async fn mark_absent_for_company_date(
    pool: &PgPool,
    company_id: Uuid,
    date: chrono::NaiveDate,
) -> AppResult<i64> {
    let tz = get_company_timezone(pool, company_id).await;
    let (today, _) = clock::date_and_time_in_tz(pool, &tz).await?;
    if date > today {
        return Err(AppError::BadRequest(
            "Cannot mark absences for a future date".into(),
        ));
    }
    if (today - date).num_days() > MAX_ABSENT_BACKFILL_DAYS {
        return Err(AppError::BadRequest(format!(
            "Absences can only be backfilled up to {MAX_ABSENT_BACKFILL_DAYS} days in the past"
        )));
    }
    mark_absent_for_date(pool, &tz, date, Some(company_id)).await
}

/// The inclusive range of local dates the auto-absent job still owes, given
/// the current local date/time and the last date it completed.
///
/// Pure so the arithmetic is testable without a database or a fake clock —
/// this is where an off-by-one silently marks a whole workforce absent (or
/// skips a day forever), and neither outcome is visible in a log line.
/// Returns `None` when nothing is due.
fn auto_absent_due_range(
    today: chrono::NaiveDate,
    now_local: chrono::NaiveTime,
    last_run: Option<chrono::NaiveDate>,
) -> Option<(chrono::NaiveDate, chrono::NaiveDate)> {
    let cutoff = chrono::NaiveTime::from_hms_opt(AUTO_ABSENT_CUTOFF.0, AUTO_ABSENT_CUTOFF.1, 0)
        .expect("valid cutoff time");
    // Today only becomes due once the daily cutoff has passed — marking at
    // 09:00 would flag everyone who simply has not arrived yet.
    let last_due = if now_local >= cutoff {
        today
    } else {
        today - chrono::Duration::days(1)
    };

    let backfill_floor = last_due - chrono::Duration::days(AUTO_ABSENT_MAX_BACKFILL_DAYS);
    let start = match last_run {
        // Never let a long outage (or a hand-edited bookmark) turn into an
        // unbounded backfill.
        Some(done) => (done + chrono::Duration::days(1)).max(backfill_floor),
        // First run ever: no history to reconstruct, start with the current due day.
        None => last_due,
    };

    (start <= last_due).then_some((start, last_due))
}

/// Run auto-absent marking for every local date that is due but not yet done:
/// each date after that company's last successful run (bounded by the backfill
/// cap) up to yesterday, plus today once past the daily cutoff. Called at
/// startup and by the daily scheduler; safe to call repeatedly.
///
/// Every tenant is evaluated on *its own* calendar and against *its own*
/// bookmark. The scheduler's daily tick only paces the job: deciding "today"
/// once, on one zone, marked a `Pacific/Honolulu` workforce absent for a date
/// that had not started there yet — a future-dated placeholder the admin
/// backfill endpoint explicitly refuses to create.
///
/// One tenant also can no longer stop the rest. A stored zone that does not
/// parse, or a failed write, is logged against its company id and that company
/// alone is left behind; previously the `?` aborted the whole catch-up before
/// the shared bookmark was written, so the next tick re-failed on the same date
/// for every tenant until the owed days aged past the backfill cap.
pub async fn run_auto_absent_catchup(pool: &PgPool) -> AppResult<i64> {
    // One clock read for the whole run; each company's local date is derived
    // from this instant in Rust. Converting per company in SQL would put every
    // tenant back in one failure domain — a single unparseable stored zone
    // aborts the query for all of them.
    let (utc_date, utc_time) = clock::date_and_time_in_tz(pool, "UTC").await?;
    let now_utc =
        chrono::DateTime::<Utc>::from_naive_utc_and_offset(utc_date.and_time(utc_time), Utc);

    let targets = attendance_reads::auto_absent_targets(pool).await?;
    let mut total = 0i64;

    for target in &targets {
        let Some(tz) = timezone::parse(&target.timezone) else {
            tracing::error!(
                company_id = %target.company_id,
                timezone = %target.timezone,
                "auto-absent: skipping company with an unrecognised IANA timezone"
            );
            continue;
        };

        let local = now_utc.with_timezone(&tz);
        let Some((start, last_due)) =
            auto_absent_due_range(local.date_naive(), local.time(), target.last_run_date)
        else {
            continue;
        };

        let mut date = start;
        while date <= last_due {
            // Pass the canonical zone name, not the raw column value.
            let outcome =
                attendance_records::mark_absent(pool, tz.name(), date, Some(target.company_id))
                    .await;
            let marked = match outcome {
                Ok(marked) => marked as i64,
                Err(e) => {
                    tracing::error!(
                        company_id = %target.company_id,
                        timezone = %target.timezone,
                        date = %date,
                        error = %e,
                        "auto-absent: company run failed; bookmark not advanced"
                    );
                    // This tenant only, and stop at the first failed date: the
                    // bookmark means "everything up to here is done", so dates
                    // must complete in order. Retried on the next tick.
                    break;
                }
            };

            // Advance the bookmark per date, so a failure part-way through does
            // not re-run the dates already completed on the next attempt.
            if let Err(e) =
                companies::set_auto_absent_last_run_date(pool, target.company_id, date).await
            {
                tracing::error!(
                    company_id = %target.company_id,
                    date = %date,
                    error = %e,
                    "auto-absent: bookmark write failed; this date will be re-run"
                );
                break;
            }

            if marked > 0 {
                tracing::info!(
                    company_id = %target.company_id,
                    date = %date,
                    marked,
                    "auto-absent: marked absentees"
                );
            }
            total += marked;
            date += chrono::Duration::days(1);
        }
    }

    tracing::info!(
        companies = targets.len(),
        marked = total,
        "auto-absent: catch-up complete"
    );
    Ok(total)
}

// ─── Attendance Summary ───

/// Per-employee aggregate for a date range. Employees with no records still appear (zero counts).
pub async fn get_attendance_summary(
    pool: &PgPool,
    company_id: Uuid,
    q: &AttendanceSummaryQuery,
) -> AppResult<Vec<AttendanceSummaryItem>> {
    let tz = get_company_timezone(pool, company_id).await;
    attendance_reads::summary(pool, company_id, &tz, q).await
}

// ─── CSV Export ───

fn csv_field(s: &str) -> String {
    // Formula neutralisation is shared with the statutory exports — the same
    // vector reaches a spreadsheet from either file. Quoting is this writer's
    // own concern and composes on top.
    let value = csv_helpers::neutralize_formula(s);
    let formula_like = value != s;

    if formula_like
        || value.contains(',')
        || value.contains('"')
        || value.contains('\n')
        || value.contains('\r')
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

/// The zone the CSV renders in. `get_company_timezone` already sanitizes, so
/// the fallback arm is unreachable in practice; it is here so the export cannot
/// take a panic path on a value that somehow slipped past the sanitizer.
/// `zone_default_matches_the_platform_fallback` pins the two to each other.
fn render_zone(tz: &str) -> Tz {
    timezone::parse(tz).unwrap_or(Tz::Asia__Kuala_Lumpur)
}

/// One record's local calendar date and wall-clock times, as `(date, check-in,
/// check-out)`; the check-out is empty for an open session.
///
/// The invariant: the printed date must be the *same* local calendar day the
/// SQL range filtered on. That holds only when both sides consult the tz
/// database. A hardcoded UTC+8 offset put a Jakarta tenant's 23:30 check-in on
/// the following day — a row dated outside the range they asked for. Resolving
/// a `FixedOffset` once per export would fail the same way across a DST
/// transition inside the range, and both Europe/London and Australia/Sydney
/// ship in the settings picker; `with_timezone(&Tz)` resolves per instant.
///
/// Pure and zone-parameterised so the boundary cases are unit-testable without
/// a database.
fn render_local(
    check_in: chrono::DateTime<Utc>,
    check_out: Option<chrono::DateTime<Utc>>,
    zone: Tz,
) -> (String, String, String) {
    let local_in = check_in.with_timezone(&zone);
    (
        local_in.format("%Y-%m-%d").to_string(),
        local_in.format("%H:%M:%S").to_string(),
        check_out
            .map(|t| t.with_timezone(&zone).format("%H:%M:%S").to_string())
            .unwrap_or_default(),
    )
}

/// Longest span one export may cover. A full statutory year plus a leap day is
/// a real year-end need; anything past that is a mistake or an attack.
pub(crate) const MAX_EXPORT_RANGE_DAYS: i64 = 366;

/// Hard row ceiling, as the backstop the span cap cannot provide: a
/// 2,000-employee tenant exporting a legitimate year is ~700k rows, and the
/// whole file is buffered before any of it ships. Failing is deliberate —
/// silently truncating hands an admin a short CSV they cannot distinguish from
/// a complete one.
const MAX_EXPORT_ROWS: usize = 100_000;

pub async fn export_attendance_csv(
    pool: &PgPool,
    company_id: Uuid,
    q: &AttendanceExportQuery,
) -> AppResult<String> {
    export_attendance_csv_bounded(pool, company_id, q, MAX_EXPORT_ROWS).await
}

/// `export_attendance_csv` with the row ceiling as a parameter, so the
/// truncation behaviour can be tested against three seeded rows instead of a
/// hundred thousand.
pub(crate) async fn export_attendance_csv_bounded(
    pool: &PgPool,
    company_id: Uuid,
    q: &AttendanceExportQuery,
    max_rows: usize,
) -> AppResult<String> {
    let tz = get_company_timezone(pool, company_id).await;

    // Default each bound *independently*. Requiring both to be absent meant a
    // lone `date_from` skipped the month default entirely and the reads layer
    // emitted only the one bound it was given — an open-ended scan of the
    // tenant's whole history.
    let (date_from, date_to) = match (q.date_from, q.date_to) {
        (Some(from), Some(to)) => (from, to),
        _ => {
            let (today, _) = clock::date_and_time_in_tz(pool, &tz).await?;
            let to = q.date_to.unwrap_or(today);
            // Anchored on the resolved `to`, not on today: a lone
            // `date_to=2024-01-15` then means 2024-01-01..2024-01-15 — the
            // month the admin actually asked about — rather than a range that
            // ends before it starts.
            let from = q.date_from.unwrap_or_else(|| to.with_day(1).unwrap_or(to));
            (from, to)
        }
    };

    // Both of these silently returned an empty (or enormous) 200 before.
    if date_from > date_to {
        return Err(AppError::BadRequest(format!(
            "The start date ({date_from}) must not be after the end date ({date_to})"
        )));
    }
    if (date_to - date_from).num_days() > MAX_EXPORT_RANGE_DAYS {
        return Err(AppError::BadRequest(format!(
            "An export may cover at most {MAX_EXPORT_RANGE_DAYS} days. Narrow the date range."
        )));
    }

    let q = AttendanceExportQuery {
        date_from: Some(date_from),
        date_to: Some(date_to),
        employee_id: q.employee_id,
        status: q.status.clone(),
        method: q.method.clone(),
    };

    // The read asks for one row more than the ceiling; its presence is the
    // "truncated" signal, with no second COUNT round trip.
    let limit = i64::try_from(max_rows).unwrap_or(i64::MAX);
    let records = attendance_reads::export_rows(pool, company_id, &tz, &q, limit).await?;
    if records.len() > max_rows {
        return Err(AppError::PayloadTooLarge(format!(
            "This export covers more than {max_rows} records. Narrow the date range or filter by employee."
        )));
    }

    let zone = render_zone(&tz);

    let mut csv = String::from(
        "Date,Employee Number,Name,Department,Check In,Check Out,\
         Hours Worked,Overtime Hours,Method,Status,Outside Geofence,Notes\n",
    );

    for r in &records {
        let (date, check_in, check_out) = render_local(r.check_in_at, r.check_out_at, zone);
        let hours = r.hours_worked.map(|h| h.to_string()).unwrap_or_default();
        let ot = r.overtime_hours.map(|h| h.to_string()).unwrap_or_default();
        let outside = r
            .is_outside_geofence
            .map(|b| if b { "Yes" } else { "No" })
            .unwrap_or("No");
        let notes = csv_field(r.notes.as_deref().unwrap_or(""));
        let dept = csv_field(r.department.as_deref().unwrap_or(""));
        let name = csv_field(&r.full_name);
        let employee_number = csv_field(&r.employee_number);
        let method = csv_field(&r.method);
        let status = csv_field(&r.status);

        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\n",
            date,
            employee_number,
            name,
            dept,
            check_in,
            check_out,
            hours,
            ot,
            method,
            status,
            outside,
            notes
        ));
    }

    Ok(csv)
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_CORRECTION_SPAN_HOURS, auto_absent_due_range, csv_field, render_local, render_zone,
        validate_correction_span,
    };
    use crate::core::error::AppError;
    use crate::core::timezone::DEFAULT_TIMEZONE;
    use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
    use chrono_tz::Tz;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }
    fn time(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).expect("valid time")
    }
    fn instant(s: &str) -> DateTime<Utc> {
        s.parse().expect("valid RFC 3339 instant")
    }

    #[test]
    fn today_is_not_due_before_the_daily_cutoff() {
        // 09:00: employees may still be arriving — marking now would flag them all.
        let due = auto_absent_due_range(date(2026, 6, 10), time(9, 0), Some(date(2026, 6, 9)));
        assert_eq!(due, None, "nothing is owed yet at 09:00");
    }

    #[test]
    fn today_becomes_due_at_the_cutoff() {
        let due = auto_absent_due_range(date(2026, 6, 10), time(12, 30), Some(date(2026, 6, 9)));
        assert_eq!(due, Some((date(2026, 6, 10), date(2026, 6, 10))));
    }

    #[test]
    fn a_missed_window_is_backfilled_rather_than_skipped_forever() {
        // Last completed run was the 5th; it is now the 10th, past cutoff.
        let due = auto_absent_due_range(date(2026, 6, 10), time(13, 0), Some(date(2026, 6, 5)));
        assert_eq!(
            due,
            Some((date(2026, 6, 6), date(2026, 6, 10))),
            "every date the job owes must be replayed"
        );
    }

    #[test]
    fn backfill_is_capped_so_a_long_outage_cannot_run_away() {
        let due = auto_absent_due_range(date(2026, 6, 10), time(13, 0), Some(date(2020, 1, 1)));
        let (start, end) = due.expect("a range is due");
        assert_eq!(end, date(2026, 6, 10));
        assert_eq!(
            (end - start).num_days(),
            super::AUTO_ABSENT_MAX_BACKFILL_DAYS,
            "the window must stay bounded by the backfill cap"
        );
    }

    #[test]
    fn first_run_only_considers_the_current_due_day() {
        // No bookmark: there is no history to reconstruct, so do not invent one.
        let due = auto_absent_due_range(date(2026, 6, 10), time(13, 0), None);
        assert_eq!(due, Some((date(2026, 6, 10), date(2026, 6, 10))));
    }

    #[test]
    fn nothing_is_due_when_the_bookmark_is_already_current_or_ahead() {
        assert_eq!(
            auto_absent_due_range(date(2026, 6, 10), time(13, 0), Some(date(2026, 6, 10))),
            None,
            "today already done"
        );
        // A clock skew or hand-edited bookmark must be a no-op, not a panic or
        // a backwards loop.
        assert_eq!(
            auto_absent_due_range(date(2026, 6, 10), time(13, 0), Some(date(2026, 7, 1))),
            None,
            "a future bookmark owes nothing"
        );
    }

    #[test]
    fn before_cutoff_yesterday_is_still_owed() {
        let due = auto_absent_due_range(date(2026, 6, 10), time(8, 0), Some(date(2026, 6, 8)));
        assert_eq!(
            due,
            Some((date(2026, 6, 9), date(2026, 6, 9))),
            "yesterday is due even though today is not yet"
        );
    }

    #[test]
    fn csv_field_quotes_delimiters_and_doubles_quotes() {
        assert_eq!(csv_field("plain text"), "plain text");
        assert_eq!(csv_field("Doe, Jane"), "\"Doe, Jane\"");
        assert_eq!(csv_field("said \"hello\""), "\"said \"\"hello\"\"\"");
        assert_eq!(csv_field("line one\nline two"), "\"line one\nline two\"");
    }

    #[test]
    fn csv_field_neutralizes_spreadsheet_formula_prefixes() {
        for value in [
            "=1+1",
            "+cmd|' /C calc'!A0",
            "-2+3",
            "@SUM(1,2)",
            "\t=1+1",
            "\r=1+1",
        ] {
            let escaped = csv_field(value);
            assert!(escaped.starts_with("\"'"), "not neutralized: {escaped}");
            assert!(escaped.ends_with('"'), "not quoted: {escaped}");
        }
    }

    // ─── CSV rendering zone ───

    #[test]
    fn zone_default_matches_the_platform_fallback() {
        // `render_zone` names the fallback as a `Tz` variant because the
        // constant is a string; if the two ever diverge, an unparseable stored
        // zone would render on a different calendar than every SQL bucket.
        let fallback = Tz::Asia__Kuala_Lumpur;
        assert_eq!(fallback.name(), DEFAULT_TIMEZONE);
        assert_eq!(render_zone("Asia/Jakarta"), Tz::Asia__Jakarta);
        assert_eq!(render_zone("Asia/Kuala_Lumpr"), fallback, "a typo degrades");
    }

    #[test]
    fn a_non_myt_tenant_renders_on_its_own_calendar() {
        // The reported case. Jakarta is UTC+7: this instant is 23:30 on the
        // 31st there, inside a `date_to=2026-07-31` export. Rendered at a fixed
        // UTC+8 it came out as 2026-08-01 00:30:00 — a row dated outside the
        // range the SQL had selected it for.
        let at = instant("2026-07-31T16:30:00Z");
        let (date, check_in, _) = render_local(at, None, render_zone("Asia/Jakarta"));
        assert_eq!(date, "2026-07-31");
        assert_eq!(check_in, "23:30:00");
    }

    #[test]
    fn the_default_myt_tenant_is_unchanged() {
        // Regression guard: the same instant is genuinely the 1st in Malaysia,
        // so the fix must not move the calendar for the default tenant.
        let at = instant("2026-07-31T16:30:00Z");
        let (date, check_in, _) = render_local(at, None, render_zone(DEFAULT_TIMEZONE));
        assert_eq!(date, "2026-08-01");
        assert_eq!(check_in, "00:30:00");
    }

    #[test]
    fn a_dst_transition_inside_the_range_is_resolved_per_instant() {
        // London moves to BST at 01:00 UTC on 2026-03-29. No single offset
        // resolved once per export can render both of these correctly, which is
        // why the renderer takes a `Tz` and not a `FixedOffset`.
        let zone = render_zone("Europe/London");
        let gmt = render_local(instant("2026-03-29T00:30:00Z"), None, zone);
        let bst = render_local(instant("2026-03-29T01:30:00Z"), None, zone);
        assert_eq!(gmt.0, "2026-03-29");
        assert_eq!(gmt.1, "00:30:00", "still GMT");
        assert_eq!(bst.0, "2026-03-29");
        assert_eq!(bst.1, "02:30:00", "the clocks have gone forward");
    }

    #[test]
    fn a_western_tenant_lands_on_the_correct_local_date() {
        // Los Angeles is 15-16 h from UTC+8, so a fixed offset was wrong on
        // nearly every row: this 08:15 local check-in was printed as the next
        // calendar day.
        let zone = render_zone("America/Los_Angeles");
        let out = Some(instant("2026-07-16T01:00:00Z"));
        let (date, check_in, check_out) = render_local(instant("2026-07-15T15:15:00Z"), out, zone);
        assert_eq!(date, "2026-07-15");
        assert_eq!(check_in, "08:15:00");
        assert_eq!(check_out, "18:00:00");
    }

    #[test]
    fn an_open_session_renders_an_empty_check_out() {
        let at = instant("2026-07-15T02:00:00Z");
        let (_, _, check_out) = render_local(at, None, render_zone(DEFAULT_TIMEZONE));
        assert_eq!(check_out, "");
    }

    // ─── Correction span ───

    #[test]
    fn a_correction_may_span_exactly_the_cap() {
        // The bound is inclusive: a full MAX_CORRECTION_SPAN_HOURS session is a
        // legitimate correction, not an error.
        let start = instant("2026-07-15T00:00:00Z");
        let end = instant("2026-07-16T00:00:00Z");
        assert_eq!(MAX_CORRECTION_SPAN_HOURS, 24);
        assert!(validate_correction_span(start, Some(end)).is_ok());
    }

    #[test]
    fn a_correction_one_second_past_the_cap_is_rejected() {
        let start = instant("2026-07-15T00:00:00Z");
        let end = instant("2026-07-16T00:00:01Z");
        let err = validate_correction_span(start, Some(end))
            .expect_err("a session one second past the cap must be a 400");
        match err {
            AppError::BadRequest(msg) => assert!(
                msg.contains("clear the check-out"),
                "the message must name the remedy: {msg}"
            ),
            other => panic!("expected BadRequest, got: {other:?}"),
        }
    }

    #[test]
    fn a_night_shift_is_well_inside_the_cap() {
        // 16 h overnight — the cap must not be set so tight that real work
        // becomes uncorrectable.
        let start = instant("2026-07-15T14:00:00Z");
        let end = instant("2026-07-16T06:00:00Z");
        assert!(validate_correction_span(start, Some(end)).is_ok());
    }

    #[test]
    fn the_absent_placeholder_shape_passes() {
        // `normalize_absent_check_out` closes an absent row at its own check-in.
        let at = instant("2026-07-15T00:00:00Z");
        assert!(validate_correction_span(at, Some(at)).is_ok());
        assert!(validate_correction_span(at, None).is_ok(), "open session");
    }
}
