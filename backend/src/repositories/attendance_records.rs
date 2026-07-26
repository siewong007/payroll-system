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
    is_offsite_network: Option<bool>,
) -> AppResult<AttendanceRecord> {
    let record = sqlx::query_as!(
        AttendanceRecord,
        r#"INSERT INTO attendance_records
           (company_id, employee_id, method, status, latitude, longitude, qr_token_id, is_outside_geofence, is_offsite_network)
           VALUES ($1, $2, 'qr_code', $3, $4, $5, $6, $7, $8)
           RETURNING id, company_id, employee_id, check_in_at, check_out_at, method, status,
                     latitude, longitude, checkout_latitude, checkout_longitude, notes, qr_token_id,
                     created_by, hours_worked, overtime_hours, is_outside_geofence, is_offsite_network,
                     created_at, updated_at"#,
        company_id,
        employee_id,
        status,
        latitude,
        longitude,
        qr_token_id,
        is_outside_geofence,
        is_offsite_network,
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
    is_offsite_network: Option<bool>,
) -> AppResult<AttendanceRecord> {
    let record = sqlx::query_as!(
        AttendanceRecord,
        r#"INSERT INTO attendance_records
           (company_id, employee_id, method, status, latitude, longitude, is_outside_geofence, is_offsite_network)
           VALUES ($1, $2, 'face_id', $3, $4, $5, $6, $7)
           RETURNING id, company_id, employee_id, check_in_at, check_out_at, method, status,
                     latitude, longitude, checkout_latitude, checkout_longitude, notes, qr_token_id,
                     created_by, hours_worked, overtime_hours, is_outside_geofence, is_offsite_network,
                     created_at, updated_at"#,
        company_id,
        employee_id,
        status,
        latitude,
        longitude,
        is_outside_geofence,
        is_offsite_network,
    )
    .fetch_one(executor)
    .await?;
    Ok(record)
}

/// Close the most recent open check-in within 24h (handles overnight shifts), computing
/// hours worked and overtime against the company's default schedule.
/// `outside_geofence` ORs into the record's flag so an off-site checkout is
/// visible even when the check-in was on-site.
pub async fn check_out(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    latitude: Option<f64>,
    longitude: Option<f64>,
    company_id: Uuid,
    outside_geofence: bool,
    offsite_network: bool,
) -> AppResult<Option<AttendanceRecord>> {
    let record = sqlx::query_as!(
        AttendanceRecord,
        r#"UPDATE attendance_records ar
           SET check_out_at = NOW(),
               checkout_latitude = $2,
               checkout_longitude = $3,
               is_outside_geofence = (COALESCE(ar.is_outside_geofence, FALSE) OR $5),
               -- Left NULL when the check-in was never evaluated and this
               -- check-out was not either: "not checked" must stay
               -- distinguishable from "checked and on-network".
               is_offsite_network = CASE
                   WHEN $6 THEN TRUE
                   ELSE ar.is_offsite_network
               END,
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
                     ar.is_offsite_network, ar.created_at, ar.updated_at"#,
        employee_id,
        latitude,
        longitude,
        company_id,
        outside_geofence,
        offsite_network,
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
                  is_offsite_network, created_at, updated_at,
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
                is_offsite_network: r.is_offsite_network,
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
                created_by, hours_worked, overtime_hours, is_outside_geofence, is_offsite_network,
                created_at, updated_at
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
                created_by, hours_worked, overtime_hours, is_outside_geofence, is_offsite_network,
                created_at, updated_at
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
                     created_by, hours_worked, overtime_hours, is_outside_geofence, is_offsite_network,
                created_at, updated_at"#,
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

pub async fn update(
    executor: impl Executor<'_, Database = Postgres>,
    record_id: Uuid,
    company_id: Uuid,
    check_in_at: DateTime<Utc>,
    check_out_at: Option<DateTime<Utc>>,
    status: &str,
    notes: Option<&str>,
) -> AppResult<AttendanceRecord> {
    let record = sqlx::query_as!(
        AttendanceRecord,
        r#"UPDATE attendance_records ar
           SET check_in_at = $3, check_out_at = $4, status = $5, notes = $6,
               -- Recompute hours and overtime from the corrected timestamps, in
               -- numeric — the correction path must not round-trip money-feeding
               -- figures through float. Same schedule expression as check_out,
               -- including the past-midnight wrap for night shifts. Clearing the
               -- check-out clears both derived figures.
               hours_worked = CASE
                   WHEN $4::timestamptz IS NULL THEN NULL
                   ELSE ROUND(EXTRACT(EPOCH FROM ($4::timestamptz - $3::timestamptz)) / 3600.0, 2)
               END,
               overtime_hours = CASE
                   WHEN $4::timestamptz IS NULL THEN NULL
                   ELSE GREATEST(0,
                       ROUND(EXTRACT(EPOCH FROM ($4::timestamptz - $3::timestamptz)) / 3600.0, 2)
                       - COALESCE((
                           SELECT MOD(EXTRACT(EPOCH FROM (ws.end_time - ws.start_time))::numeric + 86400, 86400) / 3600.0
                           FROM company_work_schedules ws
                           WHERE ws.company_id = ar.company_id AND ws.is_default = TRUE
                       ), 9))
               END,
               updated_at = NOW()
           WHERE ar.id = $1 AND ar.company_id = $2
           RETURNING id, company_id, employee_id, check_in_at, check_out_at, method, status,
                     latitude, longitude, checkout_latitude, checkout_longitude, notes, qr_token_id,
                     created_by, hours_worked, overtime_hours, is_outside_geofence, is_offsite_network,
                created_at, updated_at"#,
        record_id,
        company_id,
        check_in_at,
        check_out_at,
        status,
        notes,
    )
    .fetch_one(executor)
    .await?;
    Ok(record)
}

/// Remove today's auto-absent placeholder so a real (late) check-in
/// supersedes it. Matches only rows the cron itself wrote: system-created
/// (`created_by IS NULL`), method 'manual', status 'absent', with the cron's
/// marker note — an HR-touched row no longer matches and is preserved.
/// Returns the number of rows removed (0 or 1 in practice).
pub async fn delete_auto_absent_today(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    tz: &str,
) -> AppResult<u64> {
    let result = sqlx::query!(
        r#"DELETE FROM attendance_records
           WHERE employee_id = $1
             AND status = 'absent'
             AND method = 'manual'
             AND created_by IS NULL
             AND notes = 'Auto-marked absent (no check-in recorded)'
             AND DATE(check_in_at AT TIME ZONE $2) = DATE(NOW() AT TIME ZONE $2)"#,
        employee_id,
        tz,
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

/// Auto-mark absent for the given local calendar date in `tz`, skipping
/// holidays, approved leave, and employees who already have a record on that
/// date. Idempotent — the target date is a parameter (not NOW()) so missed
/// runs can be backfilled and tests can pin a date. `company_id` limits the
/// run to one tenant (admin backfill); `None` covers all companies (the daily
/// job). Returns rows inserted.
pub async fn mark_absent(
    executor: impl Executor<'_, Database = Postgres>,
    tz: &str,
    target_date: chrono::NaiveDate,
    company_id: Option<Uuid>,
) -> AppResult<u64> {
    let result = sqlx::query!(
        r#"INSERT INTO attendance_records
           (company_id, employee_id, check_in_at, check_out_at, method, status, notes)
           SELECT
               e.company_id,
               e.id,
               ($2::date)::timestamp AT TIME ZONE $1,
               ($2::date)::timestamp AT TIME ZONE $1,
               'manual',
               'absent',
               'Auto-marked absent (no check-in recorded)'
           FROM employees e
           -- Only working days. LEFT JOIN, not JOIN: a company that never saved
           -- working_day_config would otherwise be skipped entirely and never get
           -- absent marking. Falls back to Mon-Fri, matching calendar_service.
           LEFT JOIN working_day_config wdc
               ON wdc.company_id = e.company_id
               AND wdc.day_of_week = EXTRACT(DOW FROM $2::date)::int
           WHERE e.is_active = TRUE
             AND e.deleted_at IS NULL
             AND ($3::uuid IS NULL OR e.company_id = $3)
             -- Only within the employment window. When this query was pinned to
             -- NOW() an active employee had necessarily already joined, so the
             -- guard was unreachable; now that the date is a parameter (catch-up
             -- and admin backfill both target past dates) its absence would
             -- invent absences from before a hire or after a resignation.
             -- Mirrors the payroll eligibility filter in employees.rs.
             AND e.date_joined <= $2::date
             AND (e.date_resigned IS NULL OR e.date_resigned >= $2::date)
             AND CASE
                   WHEN EXISTS (
                       SELECT 1 FROM working_day_config w
                       WHERE w.company_id = e.company_id
                   ) THEN COALESCE(wdc.is_working_day, FALSE)
                   ELSE EXTRACT(DOW FROM $2::date)::int BETWEEN 1 AND 5
                 END
             -- No holiday that day. A recurring holiday is stored once, on the year
             -- it was created, so matching only the exact date auto-marked everyone
             -- absent on every later occurrence.
             AND NOT EXISTS (
                 SELECT 1 FROM holidays h
                 WHERE h.company_id = e.company_id
                   AND (
                       h.date = $2::date
                       OR (
                           h.is_recurring
                           AND EXTRACT(MONTH FROM h.date) = EXTRACT(MONTH FROM $2::date)
                           AND EXTRACT(DAY FROM h.date) = EXTRACT(DAY FROM $2::date)
                       )
                   )
             )
             -- Not on approved leave that day
             AND NOT EXISTS (
                 SELECT 1 FROM leave_requests lr
                 WHERE lr.employee_id = e.id
                   AND lr.status = 'approved'
                   AND $2::date BETWEEN lr.start_date AND lr.end_date
             )
             -- No attendance record on that local date. Sargable range on the raw
             -- timestamptz so the (employee_id, check_in_at) index serves it.
             AND NOT EXISTS (
                 SELECT 1 FROM attendance_records ar
                 WHERE ar.employee_id = e.id
                   AND ar.check_in_at >= ($2::date)::timestamp AT TIME ZONE $1
                   AND ar.check_in_at < ($2::date + 1)::timestamp AT TIME ZONE $1
             )"#,
        tz,
        target_date,
        company_id,
    )
    .execute(executor)
    .await?;

    Ok(result.rows_affected())
}
