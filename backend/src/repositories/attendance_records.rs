//! Data access for the `attendance_records` table.
//!
//! NOTE: several query strings carry indentation matched to the byte-exact SQL in the
//! offline `.sqlx` cache (hashing is whitespace-sensitive). Do not reflow them.

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::attendance::AttendanceRecord;

#[allow(clippy::too_many_arguments)]
pub async fn insert_qr(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Uuid,
    status: &str,
    latitude: Option<f64>,
    longitude: Option<f64>,
    qr_token_id: Uuid,
    is_outside_geofence: bool,
) -> AppResult<AttendanceRecord> {
    let record = sqlx::query_as!(
        AttendanceRecord,
        r#"INSERT INTO attendance_records
           (company_id, employee_id, method, status, latitude, longitude, qr_token_id, is_outside_geofence)
           VALUES ($1, $2, 'qr_code', $3, $4, $5, $6, $7)
           RETURNING id, company_id, employee_id, check_in_at, check_out_at, method, status,
                     latitude, longitude, checkout_latitude, checkout_longitude, notes, qr_token_id,
                     created_by, hours_worked, overtime_hours, is_outside_geofence, created_at, updated_at"#,
        company_id,
        employee_id,
        status,
        latitude,
        longitude,
        qr_token_id,
        is_outside_geofence,
    )
    .fetch_one(executor)
    .await?;
    Ok(record)
}

pub async fn insert_face(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Uuid,
    status: &str,
    latitude: Option<f64>,
    longitude: Option<f64>,
    is_outside_geofence: bool,
) -> AppResult<AttendanceRecord> {
    let record = sqlx::query_as!(
        AttendanceRecord,
        r#"INSERT INTO attendance_records
           (company_id, employee_id, method, status, latitude, longitude, is_outside_geofence)
           VALUES ($1, $2, 'face_id', $3, $4, $5, $6)
           RETURNING id, company_id, employee_id, check_in_at, check_out_at, method, status,
                     latitude, longitude, checkout_latitude, checkout_longitude, notes, qr_token_id,
                     created_by, hours_worked, overtime_hours, is_outside_geofence, created_at, updated_at"#,
        company_id,
        employee_id,
        status,
        latitude,
        longitude,
        is_outside_geofence,
    )
    .fetch_one(executor)
    .await?;
    Ok(record)
}

/// The employee's most recent open check-in (used to recover from a check-in race).
pub async fn find_open_by_employee(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
) -> AppResult<Option<AttendanceRecord>> {
    let record = sqlx::query_as!(
        AttendanceRecord,
        "SELECT id, company_id, employee_id, check_in_at, check_out_at, method, status,
                        latitude, longitude, checkout_latitude, checkout_longitude, notes, qr_token_id,
                        created_by, hours_worked, overtime_hours, is_outside_geofence, created_at, updated_at
                 FROM attendance_records WHERE employee_id = $1 AND check_out_at IS NULL LIMIT 1",
        employee_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(record)
}

/// Close the most recent open check-in within 24h (handles overnight shifts), computing
/// hours worked and overtime against the company's default schedule.
pub async fn check_out(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    latitude: Option<f64>,
    longitude: Option<f64>,
    company_id: Uuid,
) -> AppResult<Option<AttendanceRecord>> {
    let record = sqlx::query_as!(
        AttendanceRecord,
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
           RETURNING ar.id, ar.company_id, ar.employee_id, ar.check_in_at, ar.check_out_at,
                     ar.method, ar.status, ar.latitude, ar.longitude, ar.checkout_latitude,
                     ar.checkout_longitude, ar.notes, ar.qr_token_id, ar.created_by,
                     ar.hours_worked, ar.overtime_hours, ar.is_outside_geofence,
                     ar.created_at, ar.updated_at"#,
        employee_id,
        latitude,
        longitude,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(record)
}

/// Whether the employee has an open check-in for the current local day in `tz`.
pub async fn exists_active_checkin_today(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    tz: &str,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(
            SELECT 1 FROM attendance_records
            WHERE employee_id = $1
              AND DATE(check_in_at AT TIME ZONE $2) = DATE(NOW() AT TIME ZONE $2)
              AND check_out_at IS NULL
        ) AS "exists!""#,
        employee_id,
        tz,
    )
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

pub async fn get_today(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    tz: &str,
) -> AppResult<Option<AttendanceRecord>> {
    let record = sqlx::query_as!(
        AttendanceRecord,
        "SELECT id, company_id, employee_id, check_in_at, check_out_at, method, status,
                latitude, longitude, checkout_latitude, checkout_longitude, notes, qr_token_id,
                created_by, hours_worked, overtime_hours, is_outside_geofence, created_at, updated_at
         FROM attendance_records
         WHERE employee_id = $1
           AND DATE(check_in_at AT TIME ZONE $2) = DATE(NOW() AT TIME ZONE $2)
         ORDER BY check_in_at DESC
         LIMIT 1",
        employee_id,
        tz,
    )
    .fetch_optional(executor)
    .await?;
    Ok(record)
}

pub async fn get_by_id(
    executor: impl Executor<'_, Database = Postgres>,
    record_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<AttendanceRecord>> {
    let record = sqlx::query_as!(
        AttendanceRecord,
        "SELECT id, company_id, employee_id, check_in_at, check_out_at, method, status,
                latitude, longitude, checkout_latitude, checkout_longitude, notes, qr_token_id,
                created_by, hours_worked, overtime_hours, is_outside_geofence, created_at, updated_at
         FROM attendance_records WHERE id = $1 AND company_id = $2",
        record_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(record)
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_manual(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Uuid,
    check_in_at: DateTime<Utc>,
    check_out_at: Option<DateTime<Utc>>,
    status: &str,
    notes: Option<&str>,
    created_by: Uuid,
) -> AppResult<AttendanceRecord> {
    let record = sqlx::query_as!(
        AttendanceRecord,
        r#"INSERT INTO attendance_records
           (company_id, employee_id, check_in_at, check_out_at, method, status, notes, created_by)
           VALUES ($1, $2, $3, $4, 'manual', $5, $6, $7)
           RETURNING id, company_id, employee_id, check_in_at, check_out_at, method, status,
                     latitude, longitude, checkout_latitude, checkout_longitude, notes, qr_token_id,
                     created_by, hours_worked, overtime_hours, is_outside_geofence, created_at, updated_at"#,
        company_id,
        employee_id,
        check_in_at,
        check_out_at,
        status,
        notes,
        created_by,
    )
    .fetch_one(executor)
    .await?;
    Ok(record)
}

#[allow(clippy::too_many_arguments)]
pub async fn update(
    executor: impl Executor<'_, Database = Postgres>,
    record_id: Uuid,
    company_id: Uuid,
    check_in_at: DateTime<Utc>,
    check_out_at: Option<DateTime<Utc>>,
    status: &str,
    notes: Option<&str>,
    hours_worked: Option<f64>,
) -> AppResult<AttendanceRecord> {
    let record = sqlx::query_as!(
        AttendanceRecord,
        r#"UPDATE attendance_records
           SET check_in_at = $3, check_out_at = $4, status = $5, notes = $6,
               hours_worked = $7::float8, updated_at = NOW()
           WHERE id = $1 AND company_id = $2
           RETURNING id, company_id, employee_id, check_in_at, check_out_at, method, status,
                     latitude, longitude, checkout_latitude, checkout_longitude, notes, qr_token_id,
                     created_by, hours_worked, overtime_hours, is_outside_geofence, created_at, updated_at"#,
        record_id,
        company_id,
        check_in_at,
        check_out_at,
        status,
        notes,
        hours_worked,
    )
    .fetch_one(executor)
    .await?;
    Ok(record)
}

/// Auto-mark absent for the given date (`tz`), skipping holidays, approved leave, and
/// employees who already have a record. Returns the number of rows inserted.
pub async fn mark_absent(
    executor: impl Executor<'_, Database = Postgres>,
    tz: &str,
) -> AppResult<u64> {
    let result = sqlx::query!(
        r#"INSERT INTO attendance_records
           (company_id, employee_id, check_in_at, check_out_at, method, status, notes)
           SELECT
               e.company_id,
               e.id,
               DATE(NOW() AT TIME ZONE $1) + TIME '00:00',
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
        tz,
    )
    .execute(executor)
    .await?;

    Ok(result.rows_affected())
}
