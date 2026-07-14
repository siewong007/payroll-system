use std::time::Duration;

use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::attendance::{
    AttendanceExportQuery, AttendanceListQuery, AttendanceMethodResponse, AttendanceRecord,
    AttendanceRecordWithEmployee, AttendanceSummaryItem, AttendanceSummaryQuery,
    ManualAttendanceRequest, PaginatedAttendance, QrTokenResponse, UpdateAttendanceRecordRequest,
};
use crate::models::attendance_kiosk::KioskCredential;
use crate::repositories::reads::attendance as attendance_reads;
use crate::repositories::{
    attendance_kiosk_credentials, attendance_qr_tokens, attendance_records, audit_logs, clock,
    companies, company_work_schedules, employee_work_schedules, platform_settings,
};
use crate::services::audit_service::{self, AuditRequestMeta};
use crate::services::geofence_service;

// ─── QR Token TTL ───
const QR_TOKEN_TTL_SECONDS: i64 = 300;
const DEFAULT_TIMEZONE: &str = "Asia/Kuala_Lumpur";

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
pub async fn get_effective_method(
    pool: &PgPool,
    company_id: Uuid,
) -> AppResult<AttendanceMethodResponse> {
    let platform_method = get_platform_attendance_method(pool).await?;
    let allow_override = get_platform_allow_override(pool).await?;

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

    Ok(AttendanceMethodResponse {
        method,
        allow_company_override: allow_override,
        is_company_override: is_override,
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

pub async fn generate_qr_token(
    pool: &PgPool,
    company_id: Uuid,
    frontend_url: &str,
) -> AppResult<QrTokenResponse> {
    // Expire any existing unused tokens for this company
    attendance_qr_tokens::revoke_unused(pool, company_id).await?;

    let token = Uuid::new_v4().to_string().replace('-', "");
    let expires_at = Utc::now() + chrono::Duration::seconds(QR_TOKEN_TTL_SECONDS);

    attendance_qr_tokens::insert(pool, company_id, &token, expires_at).await?;

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

    let resp = generate_qr_token(pool, cred.company_id, frontend_url).await?;

    // Best-effort heartbeat; failure to record this should not block the kiosk.
    if let Err(e) = attendance_kiosk_credentials::mark_used(pool, cred.id, client_ip).await {
        tracing::warn!("Failed to update kiosk last_used: {}", e);
    }

    Ok((resp, cred.company_id))
}

// ─── Auto Late Detection ───

/// Determine attendance status based on the company's work schedule.
/// Returns "present" or "late".
pub(crate) async fn determine_checkin_status(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
) -> String {
    let tz = get_company_timezone(pool, company_id).await;

    // Day of week (0=Sunday, 6=Saturday) per the DB clock; default to Sunday on error.
    let dow = clock::dow_in_tz(pool, &tz).await.unwrap_or(0);

    // 1. Try employee-specific schedule, 2. fall back to company default.
    let timing = employee_work_schedules::find_timing_for_day(pool, employee_id, dow)
        .await
        .unwrap_or(None);

    let (start_time, grace_minutes) = if let Some(t) = timing {
        t
    } else {
        match company_work_schedules::find_default_timing(pool, company_id)
            .await
            .unwrap_or(None)
        {
            Some(t) => t,
            None => return "present".to_string(),
        }
    };

    let now_local = clock::local_time_in_tz(pool, &tz)
        .await
        .unwrap_or_else(|_| Utc::now().time());

    let cutoff = start_time + chrono::Duration::minutes(grace_minutes as i64);

    if now_local > cutoff {
        "late".to_string()
    } else {
        "present".to_string()
    }
}

/// Get the timezone for a company from its work schedule (fallback to default)
async fn get_company_timezone(pool: &PgPool, company_id: Uuid) -> String {
    company_work_schedules::find_default_timezone(pool, company_id)
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| DEFAULT_TIMEZONE.to_string())
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
    let status = determine_checkin_status(pool, employee_id, company_id).await;

    match attendance_records::insert_qr(
        pool,
        company_id,
        employee_id,
        &status,
        latitude,
        longitude,
        token_id,
        outside_geofence,
    )
    .await
    {
        Ok(record) => Ok(record),
        // Race condition: if already checked in, return the existing open record
        Err(AppError::Database(sqlx::Error::Database(db_err)))
            if db_err.code().as_deref() == Some("23505") =>
        {
            attendance_records::find_open_by_employee(pool, employee_id)
                .await?
                .ok_or_else(|| AppError::BadRequest("You already have an active check-in.".into()))
        }
        Err(e) => Err(e),
    }
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

    let status = determine_checkin_status(pool, employee_id, company_id).await;

    match attendance_records::insert_face(
        pool,
        company_id,
        employee_id,
        &status,
        latitude,
        longitude,
        outside_geofence,
    )
    .await
    {
        Ok(record) => Ok(record),
        // Race condition: if already checked in, return the existing open record
        Err(AppError::Database(sqlx::Error::Database(db_err)))
            if db_err.code().as_deref() == Some("23505") =>
        {
            attendance_records::find_open_by_employee(pool, employee_id)
                .await?
                .ok_or_else(|| AppError::BadRequest("You already have an active check-in.".into()))
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
    attendance_records::check_out(pool, employee_id, latitude, longitude, company_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(
                "No active check-in found. Please check in before checking out.".into(),
            )
        })
}

/// Prevent double check-in on the same calendar day (using company timezone)
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
    attendance_reads::list_with_employee(pool, company_id, q).await
}

pub async fn get_my_attendance(
    pool: &PgPool,
    employee_id: Uuid,
    q: &AttendanceListQuery,
) -> AppResult<PaginatedAttendance<AttendanceRecord>> {
    attendance_reads::list_for_employee(pool, employee_id, q).await
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
    let status = req.status.as_deref().unwrap_or("present");
    let check_out_at = normalize_absent_check_out(status, req.check_in_at, req.check_out_at);

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

pub async fn update_attendance_record(
    pool: &PgPool,
    company_id: Uuid,
    record_id: Uuid,
    req: &UpdateAttendanceRecordRequest,
    updated_by: Uuid,
) -> AppResult<AttendanceRecord> {
    // Fetch existing record
    let existing = attendance_records::get_by_id(pool, record_id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Attendance record not found".into()))?;

    let check_in = req.check_in_at.unwrap_or(existing.check_in_at);
    let status = req.status.as_deref().unwrap_or(&existing.status);
    let notes = req.notes.as_deref().or(existing.notes.as_deref());
    let check_out =
        normalize_absent_check_out(status, check_in, req.check_out_at.or(existing.check_out_at));

    // Validate status
    if !matches!(status, "present" | "late" | "absent" | "half_day") {
        return Err(AppError::BadRequest(
            "Status must be 'present', 'late', 'absent', or 'half_day'".into(),
        ));
    }

    // Recalculate hours_worked if both check_in and check_out are present
    let hours_worked = check_out.map(|co| {
        let diff = (co - check_in).num_seconds() as f64 / 3600.0;
        (diff * 100.0).round() / 100.0
    });

    let record = attendance_records::update(
        pool,
        record_id,
        company_id,
        check_in,
        check_out,
        status,
        notes,
        hours_worked,
    )
    .await?;

    // Log to audit trail
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
    });

    audit_logs::insert_attendance_correction(pool, updated_by, record_id, &old_vals, &new_vals)
        .await?;

    Ok(record)
}

// ─── Auto-Absent Marking ───

/// Mark all active employees in all companies as absent if they have no attendance record
/// for the given date. Respects working day config and holidays.
pub async fn mark_absent_for_date(pool: &PgPool, tz: &str) -> AppResult<i64> {
    Ok(attendance_records::mark_absent(pool, tz).await? as i64)
}

// ─── Attendance Summary ───

/// Per-employee aggregate for a date range. Employees with no records still appear (zero counts).
pub async fn get_attendance_summary(
    pool: &PgPool,
    company_id: Uuid,
    q: &AttendanceSummaryQuery,
) -> AppResult<Vec<AttendanceSummaryItem>> {
    attendance_reads::summary(pool, company_id, q).await
}

// ─── CSV Export ───

fn csv_field(s: &str) -> String {
    // Spreadsheet applications may execute cells beginning with these bytes as
    // formulas. Prefixing with an apostrophe forces the value to remain text.
    let formula_like = s
        .as_bytes()
        .first()
        .is_some_and(|first| matches!(*first, b'=' | b'+' | b'-' | b'@' | b'\t' | b'\r'));
    let value = if formula_like {
        format!("'{s}")
    } else {
        s.to_string()
    };

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

pub async fn export_attendance_csv(
    pool: &PgPool,
    company_id: Uuid,
    q: &AttendanceExportQuery,
) -> AppResult<String> {
    let records = attendance_reads::export_rows(pool, company_id, q).await?;

    let mut csv = String::from(
        "Date,Employee Number,Name,Department,Check In,Check Out,\
         Hours Worked,Overtime Hours,Method,Status,Outside Geofence,Notes\n",
    );

    for r in &records {
        let date = r.check_in_at.format("%Y-%m-%d");
        let check_in = r.check_in_at.format("%H:%M:%S");
        let check_out = r
            .check_out_at
            .map(|t| t.format("%H:%M:%S").to_string())
            .unwrap_or_default();
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
    use super::csv_field;

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
}
