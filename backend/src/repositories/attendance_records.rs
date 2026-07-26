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
                       -- Wrap past midnight: a night shift (e.g. 22:00->06:00)
                       -- has end_time < start_time, and a plain subtraction
                       -- yields -16h, which would turn an 8h shift into 24h of
                       -- overtime. Adding a day before the modulo maps that
                       -- back to 8h and leaves same-day shifts unchanged.
                       SELECT MOD(EXTRACT(EPOCH FROM (ws.end_time - ws.start_time))::numeric + 86400, 86400) / 3600.0
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
/// The employee's open record together with its local calendar date, so a caller
/// can tell a same-day double-tap from a session left open on an earlier day.
pub async fn find_open_with_local_date(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    tz: &str,
) -> AppResult<Option<(AttendanceRecord, chrono::NaiveDate, bool)>> {
    let row = sqlx::query!(
        r#"SELECT id, company_id, employee_id, check_in_at, check_out_at, method, status,
                  latitude, longitude, checkout_latitude, checkout_longitude, notes, qr_token_id,
                  created_by, hours_worked, overtime_hours, is_outside_geofence,
                  created_at, updated_at,
                  DATE(check_in_at AT TIME ZONE $2) AS "local_date!",
                  (DATE(check_in_at AT TIME ZONE $2) = DATE(NOW() AT TIME ZONE $2)) AS "is_today!"
           FROM attendance_records
           WHERE employee_id = $1 AND check_out_at IS NULL
           LIMIT 1"#,
        employee_id,
        tz,
    )
    .fetch_optional(executor)
    .await?;

    Ok(row.map(|r| {
        (
            AttendanceRecord {
                id: r.id,
                company_id: r.company_id,
                employee_id: r.employee_id,
                check_in_at: r.check_in_at,
                check_out_at: r.check_out_at,
                method: r.method,
                status: r.status,
                latitude: r.latitude,
                longitude: r.longitude,
                checkout_latitude: r.checkout_latitude,
                checkout_longitude: r.checkout_longitude,
                notes: r.notes,
                qr_token_id: r.qr_token_id,
                created_by: r.created_by,
                hours_worked: r.hours_worked,
                overtime_hours: r.overtime_hours,
                is_outside_geofence: r.is_outside_geofence,
                created_at: r.created_at,
                updated_at: r.updated_at,
            },
            r.local_date,
            r.is_today,
        )
    }))
}

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
        r#"UPDATE attendance_records ar
           SET check_in_at = $3, check_out_at = $4, status = $5, notes = $6,
               hours_worked = $7::float8,
               -- Recompute overtime from the corrected hours. Leaving it untouched
               -- kept the figure from the original check-out, so extending a
               -- checkout grew hours_worked while overtime stayed stale (and a
               -- manual record kept NULL). Same schedule expression as check_out,
               -- including the past-midnight wrap for night shifts.
               overtime_hours = CASE
                   WHEN $7::float8 IS NULL THEN NULL
                   ELSE GREATEST(0, $7::float8 - COALESCE((
                       SELECT MOD(EXTRACT(EPOCH FROM (ws.end_time - ws.start_time))::numeric + 86400, 86400) / 3600.0
                       FROM company_work_schedules ws
                       WHERE ws.company_id = ar.company_id AND ws.is_default = TRUE
                   ), 9))
               END,
               updated_at = NOW()
           WHERE ar.id = $1 AND ar.company_id = $2
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
           -- Only working days. LEFT JOIN, not JOIN: a company that never saved
           -- working_day_config would otherwise be skipped entirely and never get
           -- absent marking. Falls back to Mon-Fri, matching calendar_service.
           LEFT JOIN working_day_config wdc
               ON wdc.company_id = e.company_id
               AND wdc.day_of_week = EXTRACT(DOW FROM (NOW() AT TIME ZONE $1))::int
           WHERE e.is_active = TRUE
             AND e.deleted_at IS NULL
             AND CASE
                   WHEN EXISTS (
                       SELECT 1 FROM working_day_config w
                       WHERE w.company_id = e.company_id
                   ) THEN COALESCE(wdc.is_working_day, FALSE)
                   ELSE EXTRACT(DOW FROM (NOW() AT TIME ZONE $1))::int BETWEEN 1 AND 5
                 END
             -- No holiday today. A recurring holiday is stored once, on the year it
             -- was created, so matching only the exact date auto-marked everyone
             -- absent on every later occurrence.
             AND NOT EXISTS (
                 SELECT 1 FROM holidays h
                 WHERE h.company_id = e.company_id
                   AND (
                       h.date = DATE(NOW() AT TIME ZONE $1)
                       OR (
                           h.is_recurring
                           AND EXTRACT(MONTH FROM h.date)
                               = EXTRACT(MONTH FROM (NOW() AT TIME ZONE $1))
                           AND EXTRACT(DAY FROM h.date)
                               = EXTRACT(DAY FROM (NOW() AT TIME ZONE $1))
                       )
                   )
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
