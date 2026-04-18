use chrono::{NaiveTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::attendance::{
    AttendanceExportQuery, AttendanceListQuery, AttendanceMethodResponse, AttendanceQrToken,
    AttendanceRecord, AttendanceRecordWithEmployee, AttendanceSummaryItem, AttendanceSummaryQuery,
    ManualAttendanceRequest, PaginatedAttendance, QrTokenResponse, UpdateAttendanceRecordRequest,
};
use crate::models::work_schedule::WorkSchedule;
use crate::services::geofence_service;

// ─── QR Token TTL ───
const QR_TOKEN_TTL_SECONDS: i64 = 300;
const DEFAULT_TIMEZONE: &str = "Asia/Kuala_Lumpur";

// ─── Platform Settings ───

pub async fn get_platform_attendance_method(pool: &PgPool) -> AppResult<String> {
    let method: Option<String> =
        sqlx::query_scalar("SELECT value FROM platform_settings WHERE key = 'attendance_method'")
            .fetch_optional(pool)
            .await?;
    Ok(method.unwrap_or_else(|| "qr_code".to_string()))
}

pub async fn get_platform_allow_override(pool: &PgPool) -> AppResult<bool> {
    let val: Option<String> = sqlx::query_scalar(
        "SELECT value FROM platform_settings WHERE key = 'allow_company_override'",
    )
    .fetch_optional(pool)
    .await?;
    Ok(val.map(|v| v == "true").unwrap_or(false))
}

pub async fn set_platform_attendance_method(
    pool: &PgPool,
    method: &str,
    allow_override: bool,
    updated_by: Uuid,
) -> AppResult<()> {
    if method != "qr_code" && method != "face_id" {
        return Err(AppError::BadRequest(
            "Method must be 'qr_code' or 'face_id'".into(),
        ));
    }

    sqlx::query(
        "INSERT INTO platform_settings (key, value, updated_at, updated_by)
         VALUES ('attendance_method', $1, NOW(), $2)
         ON CONFLICT (key) DO UPDATE SET value = $1, updated_at = NOW(), updated_by = $2",
    )
    .bind(method)
    .bind(updated_by)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO platform_settings (key, value, updated_at, updated_by)
         VALUES ('allow_company_override', $1, NOW(), $2)
         ON CONFLICT (key) DO UPDATE SET value = $1, updated_at = NOW(), updated_by = $2",
    )
    .bind(if allow_override { "true" } else { "false" })
    .bind(updated_by)
    .execute(pool)
    .await?;

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
    let company_method: Option<String> =
        sqlx::query_scalar("SELECT attendance_method FROM companies WHERE id = $1")
            .bind(company_id)
            .fetch_optional(pool)
            .await?
            .flatten();

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

    sqlx::query("UPDATE companies SET attendance_method = $1 WHERE id = $2")
        .bind(method)
        .bind(company_id)
        .execute(pool)
        .await?;

    Ok(())
}

// ─── QR Token Management ───

pub async fn generate_qr_token(
    pool: &PgPool,
    company_id: Uuid,
    frontend_url: &str,
) -> AppResult<QrTokenResponse> {
    // Expire any existing unused tokens for this company
    sqlx::query(
        "UPDATE attendance_qr_tokens SET used = TRUE
         WHERE company_id = $1 AND used = FALSE",
    )
    .bind(company_id)
    .execute(pool)
    .await?;

    let token = Uuid::new_v4().to_string().replace('-', "");
    let expires_at = Utc::now() + chrono::Duration::seconds(QR_TOKEN_TTL_SECONDS);

    sqlx::query(
        "INSERT INTO attendance_qr_tokens (company_id, token, expires_at)
         VALUES ($1, $2, $3)",
    )
    .bind(company_id)
    .bind(&token)
    .bind(expires_at)
    .execute(pool)
    .await?;

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
    let row: Option<AttendanceQrToken> =
        sqlx::query_as("SELECT * FROM attendance_qr_tokens WHERE token = $1")
            .bind(token)
            .fetch_optional(pool)
            .await?;

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

// ─── Auto Late Detection ───

/// Determine attendance status based on the company's work schedule.
/// Returns "present" or "late".
async fn determine_checkin_status(pool: &PgPool, employee_id: Uuid, company_id: Uuid) -> String {
    let tz = get_company_timezone(pool, company_id).await;

    // Get current day of week (0=Sunday, 6=Saturday)
    let dow: i16 = sqlx::query_scalar("SELECT EXTRACT(DOW FROM (NOW() AT TIME ZONE $1))::int16")
        .bind(&tz)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    // 1. Try employee-specific schedule
    let emp_schedule: Option<(NaiveTime, i32)> = sqlx::query_as(
        "SELECT start_time, grace_minutes FROM employee_work_schedules
         WHERE employee_id = $1 AND day_of_week = $2 AND is_active = TRUE",
    )
    .bind(employee_id)
    .bind(dow)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    let (start_time, grace_minutes) = if let Some(s) = emp_schedule {
        s
    } else {
        // 2. Fallback to company default
        let company_schedule: Option<(NaiveTime, i32)> = sqlx::query_as(
            "SELECT start_time, grace_minutes FROM company_work_schedules WHERE company_id = $1 AND is_default = TRUE",
        )
        .bind(company_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        match company_schedule {
            Some(s) => s,
            None => return "present".to_string(),
        }
    };

    let now_local: NaiveTime = sqlx::query_scalar("SELECT (NOW() AT TIME ZONE $1)::time")
        .bind(&tz)
        .fetch_one(pool)
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
    let tz: Option<String> = sqlx::query_scalar(
        "SELECT timezone FROM company_work_schedules WHERE company_id = $1 AND is_default = TRUE",
    )
    .bind(company_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    tz.unwrap_or_else(|| DEFAULT_TIMEZONE.to_string())
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

    let result = sqlx::query_as::<_, AttendanceRecord>(
        r#"INSERT INTO attendance_records
           (company_id, employee_id, method, status, latitude, longitude, qr_token_id, is_outside_geofence)
           VALUES ($1, $2, 'qr_code', $3, $4, $5, $6, $7)
           RETURNING *"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .bind(&status)
    .bind(latitude)
    .bind(longitude)
    .bind(token_id)
    .bind(outside_geofence)
    .fetch_one(pool)
    .await;

    match result {
        Ok(record) => Ok(record),
        Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("23505") => {
            // Handle race condition: if already checked in, return the existing open record
            let record = sqlx::query_as::<_, AttendanceRecord>(
                "SELECT * FROM attendance_records WHERE employee_id = $1 AND check_out_at IS NULL LIMIT 1"
            )
            .bind(employee_id)
            .fetch_optional(pool)
            .await?;
            
            record.ok_or_else(|| AppError::BadRequest("You already have an active check-in.".into()))
        }
        Err(e) => Err(e.into()),
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

    let result = sqlx::query_as::<_, AttendanceRecord>(
        r#"INSERT INTO attendance_records
           (company_id, employee_id, method, status, latitude, longitude, is_outside_geofence)
           VALUES ($1, $2, 'face_id', $3, $4, $5, $6)
           RETURNING *"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .bind(&status)
    .bind(latitude)
    .bind(longitude)
    .bind(outside_geofence)
    .fetch_one(pool)
    .await;

    match result {
        Ok(record) => Ok(record),
        Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("23505") => {
            // Handle race condition: if already checked in, return the existing open record
            let record = sqlx::query_as::<_, AttendanceRecord>(
                "SELECT * FROM attendance_records WHERE employee_id = $1 AND check_out_at IS NULL LIMIT 1"
            )
            .bind(employee_id)
            .fetch_optional(pool)
            .await?;
            
            record.ok_or_else(|| AppError::BadRequest("You already have an active check-in.".into()))
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn check_out(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> AppResult<AttendanceRecord> {
    // Use a subquery to find the most recent open check-in within 24 hours.
    // The 24-hour window (instead of same-calendar-day) handles overnight shifts
    // where check-in and check-out span midnight.
    let record = sqlx::query_as::<_, AttendanceRecord>(
        r#"UPDATE attendance_records ar
           SET check_out_at = NOW(),
               checkout_latitude = $2,
               checkout_longitude = $3,
               hours_worked = ROUND(EXTRACT(EPOCH FROM (NOW() - ar.check_in_at)) / 3600.0, 2),
               overtime_hours = GREATEST(0,
                   ROUND(EXTRACT(EPOCH FROM (NOW() - ar.check_in_at)) / 3600.0, 2)
                   - COALESCE((
                       SELECT EXTRACT(EPOCH FROM (ws.end_time - ws.start_time)) / 3600.0
                       FROM company_work_schedules ws
                       WHERE ws.company_id = ar.company_id AND ws.is_default = TRUE
                   ), 9)
               ),
               updated_at = NOW()
           WHERE ar.id = (
               SELECT id FROM attendance_records
               WHERE employee_id = $1
                 AND company_id = $4
                 AND check_out_at IS NULL
                 AND check_in_at > NOW() - INTERVAL '24 hours'
               ORDER BY check_in_at DESC
               LIMIT 1
           )
           RETURNING ar.*"#,
    )
    .bind(employee_id)
    .bind(latitude)
    .bind(longitude)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest(
            "No active check-in found. Please check in before checking out.".into(),
        )
    })?;

    Ok(record)
}

/// Prevent double check-in on the same calendar day (using company timezone)
async fn ensure_no_active_checkin(pool: &PgPool, employee_id: Uuid, tz: &str) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
            SELECT 1 FROM attendance_records
            WHERE employee_id = $1
              AND DATE(check_in_at AT TIME ZONE $2) = DATE(NOW() AT TIME ZONE $2)
              AND check_out_at IS NULL
        )"#,
    )
    .bind(employee_id)
    .bind(tz)
    .fetch_one(pool)
    .await?;

    if exists {
        return Err(AppError::BadRequest(
            "You have already checked in today. Please check out first.".into(),
        ));
    }
    Ok(())
}

// ─── Pagination Helpers ───

fn resolve_pagination(q: &AttendanceListQuery) -> (i64, i64, i64) {
    let per_page = q.per_page.unwrap_or(50).clamp(1, 200);
    let page = q.page.unwrap_or(1).max(1);
    let offset = (page - 1) * per_page;
    (page, per_page, offset)
}

// ─── List / Query ───

pub async fn list_attendance(
    pool: &PgPool,
    company_id: Uuid,
    q: &AttendanceListQuery,
) -> AppResult<PaginatedAttendance<AttendanceRecordWithEmployee>> {
    let (page, per_page, offset) = resolve_pagination(q);

    // Build WHERE clause (shared between count + data queries)
    let mut where_clause = String::from("ar.company_id = $1");
    let mut param_idx = 2usize;

    if q.employee_id.is_some() {
        where_clause.push_str(&format!(" AND ar.employee_id = ${}", param_idx));
        param_idx += 1;
    }
    if q.date_from.is_some() {
        where_clause.push_str(&format!(" AND ar.check_in_at >= ${}::date", param_idx));
        param_idx += 1;
    }
    if q.date_to.is_some() {
        where_clause.push_str(&format!(
            " AND ar.check_in_at < (${}::date + INTERVAL '1 day')",
            param_idx
        ));
        param_idx += 1;
    }
    if q.status.is_some() {
        where_clause.push_str(&format!(" AND ar.status = ${}", param_idx));
        param_idx += 1;
    }
    if q.method.is_some() {
        where_clause.push_str(&format!(" AND ar.method = ${}", param_idx));
        param_idx += 1;
    }

    // Count query
    let count_sql = format!(
        "SELECT COUNT(*) FROM attendance_records ar WHERE {}",
        where_clause
    );
    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql).bind(company_id);
    if let Some(eid) = q.employee_id {
        count_query = count_query.bind(eid);
    }
    if let Some(ref df) = q.date_from {
        count_query = count_query.bind(df);
    }
    if let Some(ref dt) = q.date_to {
        count_query = count_query.bind(dt);
    }
    if let Some(ref st) = q.status {
        count_query = count_query.bind(st);
    }
    if let Some(ref m) = q.method {
        count_query = count_query.bind(m);
    }
    let total = count_query.fetch_one(pool).await?;

    // Data query
    let data_sql = format!(
        r#"SELECT
            ar.id, ar.company_id, ar.employee_id,
            e.employee_number, e.full_name, e.department,
            ar.check_in_at, ar.check_out_at,
            ar.method, ar.status,
            ar.latitude, ar.longitude,
            ar.checkout_latitude, ar.checkout_longitude,
            ar.notes,
            ar.hours_worked, ar.overtime_hours, ar.is_outside_geofence,
            ar.created_at
           FROM attendance_records ar
           JOIN employees e ON ar.employee_id = e.id
           WHERE {}
           ORDER BY ar.check_in_at DESC
           LIMIT ${} OFFSET ${}"#,
        where_clause,
        param_idx,
        param_idx + 1
    );

    let mut data_query =
        sqlx::query_as::<_, AttendanceRecordWithEmployee>(&data_sql).bind(company_id);
    if let Some(eid) = q.employee_id {
        data_query = data_query.bind(eid);
    }
    if let Some(ref df) = q.date_from {
        data_query = data_query.bind(df);
    }
    if let Some(ref dt) = q.date_to {
        data_query = data_query.bind(dt);
    }
    if let Some(ref st) = q.status {
        data_query = data_query.bind(st);
    }
    if let Some(ref m) = q.method {
        data_query = data_query.bind(m);
    }
    let data = data_query
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    let total_pages = (total + per_page - 1) / per_page;

    Ok(PaginatedAttendance {
        data,
        total,
        page,
        per_page,
        total_pages,
    })
}

pub async fn get_my_attendance(
    pool: &PgPool,
    employee_id: Uuid,
    q: &AttendanceListQuery,
) -> AppResult<PaginatedAttendance<AttendanceRecord>> {
    let (page, per_page, offset) = resolve_pagination(q);

    let mut where_clause = String::from("employee_id = $1");
    let mut param_idx = 2usize;

    if q.date_from.is_some() {
        where_clause.push_str(&format!(" AND check_in_at >= ${}::date", param_idx));
        param_idx += 1;
    }
    if q.date_to.is_some() {
        where_clause.push_str(&format!(
            " AND check_in_at < (${}::date + INTERVAL '1 day')",
            param_idx
        ));
        param_idx += 1;
    }

    // Count
    let count_sql = format!(
        "SELECT COUNT(*) FROM attendance_records WHERE {}",
        where_clause
    );
    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql).bind(employee_id);
    if let Some(ref df) = q.date_from {
        count_query = count_query.bind(df);
    }
    if let Some(ref dt) = q.date_to {
        count_query = count_query.bind(dt);
    }
    let total = count_query.fetch_one(pool).await?;

    // Data
    let data_sql = format!(
        "SELECT * FROM attendance_records WHERE {} ORDER BY check_in_at DESC LIMIT ${} OFFSET ${}",
        where_clause,
        param_idx,
        param_idx + 1
    );
    let mut data_query = sqlx::query_as::<_, AttendanceRecord>(&data_sql).bind(employee_id);
    if let Some(ref df) = q.date_from {
        data_query = data_query.bind(df);
    }
    if let Some(ref dt) = q.date_to {
        data_query = data_query.bind(dt);
    }
    let data = data_query
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    let total_pages = (total + per_page - 1) / per_page;

    Ok(PaginatedAttendance {
        data,
        total,
        page,
        per_page,
        total_pages,
    })
}

/// Get today's check-in for the current employee (if any)
pub async fn get_today_checkin(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<AttendanceRecord>> {
    let tz = get_company_timezone(pool, company_id).await;

    let record = sqlx::query_as::<_, AttendanceRecord>(
        "SELECT * FROM attendance_records
         WHERE employee_id = $1
           AND DATE(check_in_at AT TIME ZONE $2) = DATE(NOW() AT TIME ZONE $2)
         ORDER BY check_in_at DESC
         LIMIT 1",
    )
    .bind(employee_id)
    .bind(&tz)
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

pub async fn manual_attendance(
    pool: &PgPool,
    company_id: Uuid,
    req: ManualAttendanceRequest,
    created_by: Uuid,
) -> AppResult<AttendanceRecord> {
    let status = req.status.as_deref().unwrap_or("present");

    let record = sqlx::query_as::<_, AttendanceRecord>(
        r#"INSERT INTO attendance_records
           (company_id, employee_id, check_in_at, check_out_at, method, status, notes, created_by)
           VALUES ($1, $2, $3, $4, 'manual', $5, $6, $7)
           RETURNING *"#,
    )
    .bind(company_id)
    .bind(req.employee_id)
    .bind(req.check_in_at)
    .bind(req.check_out_at)
    .bind(status)
    .bind(req.notes)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

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
    let existing = sqlx::query_as::<_, AttendanceRecord>(
        "SELECT * FROM attendance_records WHERE id = $1 AND company_id = $2",
    )
    .bind(record_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Attendance record not found".into()))?;

    let check_in = req.check_in_at.unwrap_or(existing.check_in_at);
    let check_out = req.check_out_at.or(existing.check_out_at);
    let status = req.status.as_deref().unwrap_or(&existing.status);
    let notes = req.notes.as_deref().or(existing.notes.as_deref());

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

    let record = sqlx::query_as::<_, AttendanceRecord>(
        r#"UPDATE attendance_records
           SET check_in_at = $3, check_out_at = $4, status = $5, notes = $6,
               hours_worked = $7, updated_at = NOW()
           WHERE id = $1 AND company_id = $2
           RETURNING *"#,
    )
    .bind(record_id)
    .bind(company_id)
    .bind(check_in)
    .bind(check_out)
    .bind(status)
    .bind(notes)
    .bind(hours_worked)
    .fetch_one(pool)
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

    sqlx::query(
        r#"INSERT INTO audit_logs (user_id, action, entity_type, entity_id, old_values, new_values, description)
           VALUES ($1, 'update', 'attendance_record', $2, $3, $4, 'Attendance record corrected')"#,
    )
    .bind(updated_by)
    .bind(record_id)
    .bind(&old_vals)
    .bind(&new_vals)
    .execute(pool)
    .await?;

    Ok(record)
}

// ─── Auto-Absent Marking ───

/// Mark all active employees in all companies as absent if they have no attendance record
/// for the given date. Respects working day config and holidays.
pub async fn mark_absent_for_date(pool: &PgPool, tz: &str) -> AppResult<i64> {
    // Insert absent records for employees who:
    // 1. Are active and not deleted
    // 2. Work on this day of week (working_day_config)
    // 3. Don't have a holiday today
    // 4. Don't already have an attendance record today
    let result = sqlx::query(
        r#"INSERT INTO attendance_records (company_id, employee_id, check_in_at, method, status, notes)
           SELECT
               e.company_id,
               e.id,
               DATE(NOW() AT TIME ZONE $1) + TIME '00:00',
               'manual',
               'absent',
               'Auto-marked absent (no check-in recorded)'
           FROM employees e
           -- Only working days
           JOIN working_day_config wdc
               ON wdc.company_id = e.company_id
               AND wdc.day_of_week = EXTRACT(DOW FROM (NOW() AT TIME ZONE $1))::int
               AND wdc.is_working_day = TRUE
           WHERE e.is_active = TRUE
             AND e.deleted_at IS NULL
             -- No holiday today
             AND NOT EXISTS (
                 SELECT 1 FROM holidays h
                 WHERE h.company_id = e.company_id
                   AND h.date = DATE(NOW() AT TIME ZONE $1)
             )
             -- Not on approved leave today
             AND NOT EXISTS (
                 SELECT 1 FROM leave_requests lr
                 WHERE lr.employee_id = e.id
                   AND lr.status = 'approved'
                   AND DATE(NOW() AT TIME ZONE $1) BETWEEN lr.start_date AND lr.end_date
             )
             -- No attendance record today
             AND NOT EXISTS (
                 SELECT 1 FROM attendance_records ar
                 WHERE ar.employee_id = e.id
                   AND DATE(ar.check_in_at AT TIME ZONE $1) = DATE(NOW() AT TIME ZONE $1)
             )"#,
    )
    .bind(tz)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() as i64)
}

// ─── Attendance Summary ───

/// Per-employee aggregate for a date range. Employees with no records still appear (zero counts).
pub async fn get_attendance_summary(
    pool: &PgPool,
    company_id: Uuid,
    q: &AttendanceSummaryQuery,
) -> AppResult<Vec<AttendanceSummaryItem>> {
    let mut extra_where = String::new();
    let mut param_idx = 4usize;

    if q.employee_id.is_some() {
        extra_where.push_str(&format!(" AND e.id = ${}", param_idx));
        param_idx += 1;
    }
    if q.department.is_some() {
        extra_where.push_str(&format!(" AND e.department = ${}", param_idx));
        param_idx += 1;
    }
    let _ = param_idx; // suppress unused warning

    let sql = format!(
        r#"SELECT
               e.id              AS employee_id,
               e.employee_number,
               e.full_name,
               e.department,
               COUNT(*) FILTER (WHERE ar.status = 'present')  AS present_days,
               COUNT(*) FILTER (WHERE ar.status = 'late')     AS late_days,
               COUNT(*) FILTER (WHERE ar.status = 'absent')   AS absent_days,
               COUNT(*) FILTER (WHERE ar.status = 'half_day') AS half_days,
               COALESCE(SUM(ar.hours_worked),    0)::NUMERIC(10,2) AS total_hours,
               COALESCE(SUM(ar.overtime_hours),  0)::NUMERIC(10,2) AS overtime_hours,
               COUNT(*) FILTER (
                   WHERE ar.check_out_at IS NULL AND ar.status NOT IN ('absent')
               ) AS unchecked_out_days
           FROM employees e
           LEFT JOIN attendance_records ar
               ON  ar.employee_id = e.id
               AND ar.check_in_at >= $2::date
               AND ar.check_in_at <  ($3::date + INTERVAL '1 day')
           WHERE e.company_id   = $1
             AND e.is_active    = TRUE
             AND e.deleted_at   IS NULL
             {}
           GROUP BY e.id, e.employee_number, e.full_name, e.department
           ORDER BY e.full_name"#,
        extra_where
    );

    let mut query = sqlx::query_as::<_, AttendanceSummaryItem>(&sql)
        .bind(company_id)
        .bind(&q.date_from)
        .bind(&q.date_to);

    if let Some(eid) = q.employee_id {
        query = query.bind(eid);
    }
    if let Some(ref d) = q.department {
        query = query.bind(d);
    }

    Ok(query.fetch_all(pool).await?)
}

// ─── CSV Export ───

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub async fn export_attendance_csv(
    pool: &PgPool,
    company_id: Uuid,
    q: &AttendanceExportQuery,
) -> AppResult<String> {
    let mut where_clause = String::from("ar.company_id = $1");
    let mut param_idx = 2usize;

    if q.employee_id.is_some() {
        where_clause.push_str(&format!(" AND ar.employee_id = ${}", param_idx));
        param_idx += 1;
    }
    if q.date_from.is_some() {
        where_clause.push_str(&format!(" AND ar.check_in_at >= ${}::date", param_idx));
        param_idx += 1;
    }
    if q.date_to.is_some() {
        where_clause.push_str(&format!(
            " AND ar.check_in_at < (${}::date + INTERVAL '1 day')",
            param_idx
        ));
        param_idx += 1;
    }
    if q.status.is_some() {
        where_clause.push_str(&format!(" AND ar.status = ${}", param_idx));
        param_idx += 1;
    }
    let _ = param_idx;

    let sql = format!(
        r#"SELECT
               ar.id, ar.company_id, ar.employee_id,
               e.employee_number, e.full_name, e.department,
               ar.check_in_at, ar.check_out_at,
               ar.method, ar.status,
               ar.latitude, ar.longitude,
               ar.checkout_latitude, ar.checkout_longitude,
               ar.notes, ar.hours_worked, ar.overtime_hours, ar.is_outside_geofence,
               ar.created_at
           FROM attendance_records ar
           JOIN employees e ON ar.employee_id = e.id
           WHERE {}
           ORDER BY ar.check_in_at DESC"#,
        where_clause
    );

    let mut dq = sqlx::query_as::<_, AttendanceRecordWithEmployee>(&sql).bind(company_id);
    if let Some(eid) = q.employee_id {
        dq = dq.bind(eid);
    }
    if let Some(ref f) = q.date_from {
        dq = dq.bind(f);
    }
    if let Some(ref t) = q.date_to {
        dq = dq.bind(t);
    }
    if let Some(ref s) = q.status {
        dq = dq.bind(s);
    }

    let records = dq.fetch_all(pool).await?;

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

        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\n",
            date,
            r.employee_number,
            name,
            dept,
            check_in,
            check_out,
            hours,
            ot,
            r.method,
            r.status,
            outside,
            notes
        ));
    }

    Ok(csv)
}
