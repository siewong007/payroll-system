//! Data access for the `attendance_records` table.
//!
//! NOTE: several query strings carry indentation matched to the byte-exact SQL in the
//! offline `.sqlx` cache (hashing is whitespace-sensitive). Do not reflow them.

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::attendance::AttendanceRecord;

pub struct CheckInInsertParams<'a> {
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub method: &'a str,
    pub status: &'a str,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub qr_token_id: Option<Uuid>,
    pub is_outside_geofence: bool,
    pub is_offsite_network: Option<bool>,
    pub session_id: Option<Uuid>,
    pub device_id: Option<Uuid>,
    pub cookie_id: Option<Uuid>,
    pub passkey_id: Option<Uuid>,
    pub user_agent: Option<&'a str>,
    pub is_untrusted_device: Option<bool>,
    pub anomaly_flags: &'a [String],
    pub photo_url: Option<&'a str>,
}

pub async fn insert_checked_in(
    executor: impl Executor<'_, Database = Postgres>,
    params: &CheckInInsertParams<'_>,
) -> AppResult<AttendanceRecord> {
    let record = sqlx::query_as!(
        AttendanceRecord,
        r#"INSERT INTO attendance_records
           (company_id, employee_id, method, status, latitude, longitude, qr_token_id,
            is_outside_geofence, is_offsite_network,
            check_in_session_id, check_in_device_id, check_in_cookie_id, check_in_passkey_id,
            check_in_user_agent, is_untrusted_device, anomaly_flags, check_in_photo_url)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
           RETURNING id, company_id, employee_id, check_in_at, check_out_at, method, status,
                     latitude, longitude, checkout_latitude, checkout_longitude, notes, qr_token_id,
                     created_by, hours_worked, overtime_hours, is_outside_geofence, is_offsite_network,
                     check_in_session_id, check_out_session_id, check_in_device_id, check_out_device_id,
                     check_in_cookie_id, check_out_cookie_id, check_in_passkey_id,
                     check_in_user_agent, check_out_user_agent, is_untrusted_device, anomaly_flags,
                     check_in_photo_url, check_out_photo_url, check_in_photo_purged_at, check_out_photo_purged_at,
                     created_at, updated_at"#,
        params.company_id,
        params.employee_id,
        params.method,
        params.status,
        params.latitude,
        params.longitude,
        params.qr_token_id,
        params.is_outside_geofence,
        params.is_offsite_network,
        params.session_id,
        params.device_id,
        params.cookie_id,
        params.passkey_id,
        params.user_agent,
        params.is_untrusted_device,
        params.anomaly_flags,
        params.photo_url,
    )
    .fetch_one(executor)
    .await?;
    Ok(record)
}

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
    insert_checked_in(
        executor,
        &CheckInInsertParams {
            company_id,
            employee_id,
            method: "qr_code",
            status,
            latitude,
            longitude,
            qr_token_id: Some(qr_token_id),
            is_outside_geofence,
            is_offsite_network,
            session_id: None,
            device_id: None,
            cookie_id: None,
            passkey_id: None,
            user_agent: None,
            is_untrusted_device: None,
            anomaly_flags: &[],
            photo_url: None,
        },
    )
    .await
}

#[allow(clippy::too_many_arguments)]
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
    insert_checked_in(
        executor,
        &CheckInInsertParams {
            company_id,
            employee_id,
            method: "face_id",
            status,
            latitude,
            longitude,
            qr_token_id: None,
            is_outside_geofence,
            is_offsite_network,
            session_id: None,
            device_id: None,
            cookie_id: None,
            passkey_id: None,
            user_agent: None,
            is_untrusted_device: None,
            anomaly_flags: &[],
            photo_url: None,
        },
    )
    .await
}

pub struct CheckOutUpdateParams<'a> {
    pub employee_id: Uuid,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub company_id: Uuid,
    pub outside_geofence: bool,
    pub offsite_network: bool,
    pub max_overtime_hours: rust_decimal::Decimal,
    pub session_id: Option<Uuid>,
    pub device_id: Option<Uuid>,
    pub cookie_id: Option<Uuid>,
    pub user_agent: Option<&'a str>,
    pub untrusted_device: bool,
    pub anomaly_flags: &'a [String],
    pub photo_url: Option<&'a str>,
}

/// Close the most recent open check-in within 24h (handles overnight shifts), computing
/// hours worked and overtime against the company's default schedule.
#[allow(clippy::too_many_arguments)]
pub async fn check_out(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    latitude: Option<f64>,
    longitude: Option<f64>,
    company_id: Uuid,
    outside_geofence: bool,
    offsite_network: bool,
    max_overtime_hours: rust_decimal::Decimal,
) -> AppResult<Option<AttendanceRecord>> {
    check_out_with_context(
        executor,
        &CheckOutUpdateParams {
            employee_id,
            latitude,
            longitude,
            company_id,
            outside_geofence,
            offsite_network,
            max_overtime_hours,
            session_id: None,
            device_id: None,
            cookie_id: None,
            user_agent: None,
            untrusted_device: false,
            anomaly_flags: &[],
            photo_url: None,
        },
    )
    .await
}

pub async fn check_out_with_context(
    executor: impl Executor<'_, Database = Postgres>,
    params: &CheckOutUpdateParams<'_>,
) -> AppResult<Option<AttendanceRecord>> {
    let record = sqlx::query_as!(
        AttendanceRecord,
        r#"UPDATE attendance_records ar
           SET check_out_at = NOW(),
               checkout_latitude = $2,
               checkout_longitude = $3,
               is_outside_geofence = (COALESCE(ar.is_outside_geofence, FALSE) OR $5),
               is_offsite_network = CASE
                   WHEN $6 THEN TRUE
                   ELSE ar.is_offsite_network
               END,
               check_out_session_id = $8,
               check_out_device_id = $9,
               check_out_cookie_id = $10,
               check_out_user_agent = $11,
               is_untrusted_device = CASE
                   WHEN $12 THEN TRUE
                   ELSE ar.is_untrusted_device
               END,
               anomaly_flags = ARRAY(
                   SELECT DISTINCT f
                   FROM unnest(ar.anomaly_flags || $13::text[]) AS f
                   ORDER BY f
               ),
               check_out_photo_url = COALESCE($14, ar.check_out_photo_url),
               hours_worked = src.elapsed_hours,
               overtime_hours = CASE
                   WHEN (COALESCE(ar.is_outside_geofence, FALSE) OR $5)
                     OR (COALESCE(ar.is_offsite_network, FALSE) OR $6)
                     OR src.overtime_hours > $7 THEN NULL
                   ELSE src.overtime_hours
               END,
               updated_at = NOW()
           FROM (
               SELECT open_record.id,
                      open_record.elapsed_hours,
                      GREATEST(0,
                          GREATEST(0, open_record.elapsed_hours - COALESCE(sched.break_hours, 0))
                          - GREATEST(0, COALESCE(sched.shift_span_hours, 9) - COALESCE(sched.break_hours, 0))
                      ) AS overtime_hours
               FROM (
                   SELECT id, company_id,
                          ROUND(EXTRACT(EPOCH FROM (NOW() - check_in_at)) / 3600.0, 2) AS elapsed_hours
                   FROM attendance_records
                   WHERE employee_id = $1
                     AND company_id = $4
                     AND check_out_at IS NULL
                     AND check_in_at > NOW() - INTERVAL '24 hours'
                     AND check_in_at <= NOW()
                   ORDER BY check_in_at DESC
                   LIMIT 1
               ) open_record
               LEFT JOIN LATERAL (
                   SELECT ws.unpaid_break_minutes / 60.0 AS break_hours,
                          MOD(EXTRACT(EPOCH FROM (ws.end_time - ws.start_time))::numeric + 86400, 86400) / 3600.0 AS shift_span_hours
                   FROM company_work_schedules ws
                   WHERE ws.company_id = open_record.company_id AND ws.is_default = TRUE
                   LIMIT 1
               ) sched ON TRUE
           ) src
           WHERE ar.id = src.id AND ar.check_out_at IS NULL
           RETURNING ar.id, ar.company_id, ar.employee_id, ar.check_in_at, ar.check_out_at,
                     ar.method, ar.status, ar.latitude, ar.longitude, ar.checkout_latitude,
                     ar.checkout_longitude, ar.notes, ar.qr_token_id, ar.created_by,
                     ar.hours_worked, ar.overtime_hours, ar.is_outside_geofence,
                     ar.is_offsite_network,
                     ar.check_in_session_id, ar.check_out_session_id, ar.check_in_device_id, ar.check_out_device_id,
                     ar.check_in_cookie_id, ar.check_out_cookie_id, ar.check_in_passkey_id,
                     ar.check_in_user_agent, ar.check_out_user_agent, ar.is_untrusted_device, ar.anomaly_flags,
                     ar.check_in_photo_url, ar.check_out_photo_url, ar.check_in_photo_purged_at, ar.check_out_photo_purged_at,
                     ar.created_at, ar.updated_at"#,
        params.employee_id,
        params.latitude,
        params.longitude,
        params.company_id,
        params.outside_geofence,
        params.offsite_network,
        params.max_overtime_hours,
        params.session_id,
        params.device_id,
        params.cookie_id,
        params.user_agent,
        params.untrusted_device,
        params.anomaly_flags,
        params.photo_url,
    )
    .fetch_optional(executor)
    .await?;
    Ok(record)
}

/// Find the employee's most recent open check-in within the last 24 hours (used
/// before check-out to evaluate device mismatch).
pub async fn find_latest_open_24h(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<AttendanceRecord>> {
    let record = sqlx::query_as!(
        AttendanceRecord,
        r#"SELECT id, company_id, employee_id, check_in_at, check_out_at, method, status,
                  latitude, longitude, checkout_latitude, checkout_longitude, notes, qr_token_id,
                  created_by, hours_worked, overtime_hours, is_outside_geofence, is_offsite_network,
                  check_in_session_id, check_out_session_id, check_in_device_id, check_out_device_id,
                  check_in_cookie_id, check_out_cookie_id, check_in_passkey_id,
                  check_in_user_agent, check_out_user_agent, is_untrusted_device, anomaly_flags,
                  check_in_photo_url, check_out_photo_url, check_in_photo_purged_at, check_out_photo_purged_at,
                  created_at, updated_at
           FROM attendance_records
           WHERE employee_id = $1
             AND company_id = $2
             AND check_out_at IS NULL
             AND check_in_at > NOW() - INTERVAL '24 hours'
             AND check_in_at <= NOW()
           ORDER BY check_in_at DESC
           LIMIT 1"#,
        employee_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(record)
}

/// Find records belonging to other employees in the same company that used
/// `cookie_id` within the last 24 hours (shared-device detection).
pub async fn find_other_employee_records_by_cookie_24h(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    exclude_employee_id: Uuid,
    cookie_id: Uuid,
) -> AppResult<Vec<Uuid>> {
    let ids = sqlx::query_scalar!(
        r#"SELECT id
           FROM attendance_records
           WHERE company_id = $1
             AND employee_id <> $2
             AND (
                 (check_in_cookie_id = $3 AND check_in_at > NOW() - INTERVAL '24 hours')
                 OR (check_out_cookie_id = $3 AND check_out_at > NOW() - INTERVAL '24 hours')
             )"#,
        company_id,
        exclude_employee_id,
        cookie_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(ids)
}

/// Append a single anomaly flag onto existing record(s) without duplicating.
pub async fn append_anomaly_flag(
    executor: impl Executor<'_, Database = Postgres>,
    record_ids: &[Uuid],
    flag: &str,
) -> AppResult<()> {
    if record_ids.is_empty() {
        return Ok(());
    }
    sqlx::query!(
        r#"UPDATE attendance_records
           SET anomaly_flags = CASE
                   WHEN $2 = ANY(anomaly_flags) THEN anomaly_flags
                   ELSE array_append(anomaly_flags, $2)
               END,
               updated_at = NOW()
           WHERE id = ANY($1)"#,
        record_ids,
        flag,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Clear expired check-in / check-out attendance photos per each company's
/// `attendance_photo_retention_days` policy, remove matching `uploaded_files`
/// ownership rows, and return stored file names to unlink from disk.
pub async fn purge_expired_photos(
    executor: impl Executor<'_, Database = Postgres>,
) -> AppResult<Vec<String>> {
    let names = sqlx::query_scalar!(
        r#"WITH expired_in AS (
               SELECT ar.id, substring(ar.check_in_photo_url FROM 14) AS stored_name
               FROM attendance_records ar
               JOIN companies c ON c.id = ar.company_id
               WHERE ar.check_in_photo_url IS NOT NULL
                 AND ar.check_in_at < NOW() - make_interval(days => c.attendance_photo_retention_days)
           ),
           expired_out AS (
               SELECT ar.id, substring(ar.check_out_photo_url FROM 14) AS stored_name
               FROM attendance_records ar
               JOIN companies c ON c.id = ar.company_id
               WHERE ar.check_out_photo_url IS NOT NULL
                 AND COALESCE(ar.check_out_at, ar.check_in_at) < NOW() - make_interval(days => c.attendance_photo_retention_days)
           ),
           updated AS (
               UPDATE attendance_records ar
               SET check_in_photo_url = CASE
                       WHEN ar.id IN (SELECT id FROM expired_in) THEN NULL
                       ELSE ar.check_in_photo_url
                   END,
                   check_in_photo_purged_at = CASE
                       WHEN ar.id IN (SELECT id FROM expired_in) THEN NOW()
                       ELSE ar.check_in_photo_purged_at
                   END,
                   check_out_photo_url = CASE
                       WHEN ar.id IN (SELECT id FROM expired_out) THEN NULL
                       ELSE ar.check_out_photo_url
                   END,
                   check_out_photo_purged_at = CASE
                       WHEN ar.id IN (SELECT id FROM expired_out) THEN NOW()
                       ELSE ar.check_out_photo_purged_at
                   END,
                   updated_at = NOW()
               WHERE ar.id IN (SELECT id FROM expired_in UNION SELECT id FROM expired_out)
               RETURNING ar.id
           ),
           to_remove AS (
               SELECT stored_name FROM expired_in WHERE stored_name IS NOT NULL
               UNION
               SELECT stored_name FROM expired_out WHERE stored_name IS NOT NULL
           ),
           deleted_uploads AS (
               DELETE FROM uploaded_files uf
               WHERE uf.stored_name IN (SELECT stored_name FROM to_remove)
               RETURNING uf.stored_name
           )
           SELECT stored_name AS "stored_name!"
           FROM to_remove
           WHERE EXISTS (SELECT 1 FROM updated LIMIT 1)
              OR EXISTS (SELECT 1 FROM deleted_uploads LIMIT 1)"#,
    )
    .fetch_all(executor)
    .await?;
    Ok(names)
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
                  is_offsite_network,
                  check_in_session_id, check_out_session_id, check_in_device_id, check_out_device_id,
                  check_in_cookie_id, check_out_cookie_id, check_in_passkey_id,
                  check_in_user_agent, check_out_user_agent, is_untrusted_device, anomaly_flags,
                  check_in_photo_url, check_out_photo_url, check_in_photo_purged_at, check_out_photo_purged_at,
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
                is_offsite_network: r.is_offsite_network,
                check_in_session_id: r.check_in_session_id,
                check_out_session_id: r.check_out_session_id,
                check_in_device_id: r.check_in_device_id,
                check_out_device_id: r.check_out_device_id,
                check_in_cookie_id: r.check_in_cookie_id,
                check_out_cookie_id: r.check_out_cookie_id,
                check_in_passkey_id: r.check_in_passkey_id,
                check_in_user_agent: r.check_in_user_agent,
                check_out_user_agent: r.check_out_user_agent,
                is_untrusted_device: r.is_untrusted_device,
                anomaly_flags: r.anomaly_flags,
                check_in_photo_url: r.check_in_photo_url,
                check_out_photo_url: r.check_out_photo_url,
                check_in_photo_purged_at: r.check_in_photo_purged_at,
                check_out_photo_purged_at: r.check_out_photo_purged_at,
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
              AND check_in_at >= date_trunc('day', NOW() AT TIME ZONE $2) AT TIME ZONE $2
              AND check_in_at < (date_trunc('day', NOW() AT TIME ZONE $2) + INTERVAL '1 day') AT TIME ZONE $2
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
                check_in_session_id, check_out_session_id, check_in_device_id, check_out_device_id,
                check_in_cookie_id, check_out_cookie_id, check_in_passkey_id,
                check_in_user_agent, check_out_user_agent, is_untrusted_device, anomaly_flags,
                check_in_photo_url, check_out_photo_url, check_in_photo_purged_at, check_out_photo_purged_at,
                created_at, updated_at
         FROM attendance_records
         WHERE employee_id = $1
           AND check_in_at >= date_trunc('day', NOW() AT TIME ZONE $2) AT TIME ZONE $2
           AND check_in_at < (date_trunc('day', NOW() AT TIME ZONE $2) + INTERVAL '1 day') AT TIME ZONE $2
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
                check_in_session_id, check_out_session_id, check_in_device_id, check_out_device_id,
                check_in_cookie_id, check_out_cookie_id, check_in_passkey_id,
                check_in_user_agent, check_out_user_agent, is_untrusted_device, anomaly_flags,
                check_in_photo_url, check_out_photo_url, check_in_photo_purged_at, check_out_photo_purged_at,
                created_at, updated_at
         FROM attendance_records WHERE id = $1 AND company_id = $2",
        record_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(record)
}

pub async fn get_by_id_locked(
    executor: impl Executor<'_, Database = Postgres>,
    record_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<AttendanceRecord>> {
    let record = sqlx::query_as!(
        AttendanceRecord,
        "SELECT id, company_id, employee_id, check_in_at, check_out_at, method, status,
                latitude, longitude, checkout_latitude, checkout_longitude, notes, qr_token_id,
                created_by, hours_worked, overtime_hours, is_outside_geofence, is_offsite_network,
                check_in_session_id, check_out_session_id, check_in_device_id, check_out_device_id,
                check_in_cookie_id, check_out_cookie_id, check_in_passkey_id,
                check_in_user_agent, check_out_user_agent, is_untrusted_device, anomaly_flags,
                check_in_photo_url, check_out_photo_url, check_in_photo_purged_at, check_out_photo_purged_at,
                created_at, updated_at
         FROM attendance_records WHERE id = $1 AND company_id = $2
         FOR UPDATE",
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
           (company_id, employee_id, check_in_at, check_out_at, method, status, notes, created_by,
            hours_worked, overtime_hours)
           SELECT
               $1, $2, $3, $4, 'manual', $5::varchar, $6, $7,
               CASE
                   WHEN $4::timestamptz IS NULL OR $5::varchar = 'absent' THEN NULL
                   ELSE ROUND(EXTRACT(EPOCH FROM ($4::timestamptz - $3::timestamptz)) / 3600.0, 2)
               END,
               CASE
                   WHEN $4::timestamptz IS NULL THEN NULL
                   WHEN $5::varchar = 'absent' THEN 0
                   ELSE GREATEST(0,
                       GREATEST(0,
                           ROUND(EXTRACT(EPOCH FROM ($4::timestamptz - $3::timestamptz)) / 3600.0, 2)
                           - COALESCE(ws.unpaid_break_minutes, 0) / 60.0
                       )
                       - GREATEST(0,
                           COALESCE(MOD(EXTRACT(EPOCH FROM (ws.end_time - ws.start_time))::numeric + 86400, 86400) / 3600.0, 9)
                           - COALESCE(ws.unpaid_break_minutes, 0) / 60.0
                       )
                   )
               END
           FROM (VALUES (1)) AS _single(k)
           LEFT JOIN company_work_schedules ws
             ON ws.company_id = $1 AND ws.is_default = TRUE
           RETURNING id, company_id, employee_id, check_in_at, check_out_at, method, status,
                     latitude, longitude, checkout_latitude, checkout_longitude, notes, qr_token_id,
                     created_by, hours_worked, overtime_hours, is_outside_geofence, is_offsite_network,
                     check_in_session_id, check_out_session_id, check_in_device_id, check_out_device_id,
                     check_in_cookie_id, check_out_cookie_id, check_in_passkey_id,
                     check_in_user_agent, check_out_user_agent, is_untrusted_device, anomaly_flags,
                     check_in_photo_url, check_out_photo_url, check_in_photo_purged_at, check_out_photo_purged_at,
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
           SET check_in_at = $3, check_out_at = $4, status = $5::varchar, notes = $6,
               -- Recompute hours and overtime from the corrected timestamps, in
               -- numeric — the correction path must not round-trip money-feeding
               -- figures through float. Same schedule expression as check_out,
               -- including the past-midnight wrap for night shifts. Clearing the
               -- check-out clears both derived figures.
               hours_worked = CASE
                   WHEN $4::timestamptz IS NULL OR $5::varchar = 'absent' THEN NULL
                   ELSE ROUND(EXTRACT(EPOCH FROM ($4::timestamptz - $3::timestamptz)) / 3600.0, 2)
               END,
               overtime_hours = CASE
                   WHEN $4::timestamptz IS NULL THEN NULL
                   WHEN $5::varchar = 'absent' THEN 0
                   ELSE GREATEST(0,
                       GREATEST(0,
                           ROUND(EXTRACT(EPOCH FROM ($4::timestamptz - $3::timestamptz)) / 3600.0, 2)
                           - COALESCE(ws.unpaid_break_minutes, 0) / 60.0
                       )
                       - GREATEST(0,
                           COALESCE(MOD(EXTRACT(EPOCH FROM (ws.end_time - ws.start_time))::numeric + 86400, 86400) / 3600.0, 9)
                           - COALESCE(ws.unpaid_break_minutes, 0) / 60.0
                       )
                   )
               END,
               updated_at = NOW()
           FROM (SELECT $2::uuid AS cid) AS _scope
           LEFT JOIN company_work_schedules ws
             ON ws.company_id = _scope.cid AND ws.is_default = TRUE
           WHERE ar.id = $1 AND ar.company_id = $2
           RETURNING ar.id, ar.company_id, ar.employee_id, ar.check_in_at, ar.check_out_at, ar.method, ar.status,
                     ar.latitude, ar.longitude, ar.checkout_latitude, ar.checkout_longitude, ar.notes, ar.qr_token_id,
                     ar.created_by, ar.hours_worked, ar.overtime_hours, ar.is_outside_geofence, ar.is_offsite_network,
                     ar.check_in_session_id, ar.check_out_session_id, ar.check_in_device_id, ar.check_out_device_id,
                     ar.check_in_cookie_id, ar.check_out_cookie_id, ar.check_in_passkey_id,
                     ar.check_in_user_agent, ar.check_out_user_agent, ar.is_untrusted_device, ar.anomaly_flags,
                     ar.check_in_photo_url, ar.check_out_photo_url, ar.check_in_photo_purged_at, ar.check_out_photo_purged_at,
                     ar.created_at, ar.updated_at"#,
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

/// The note the auto-absent cron stamps on every placeholder it writes. It is
/// the marker that separates a cron row from an HR-edited one, so it appears in
/// three places: the INSERT below, the DELETE that a check-in uses to supersede
/// it, and — necessarily as a literal, since an index predicate cannot be
/// parameterised — migration 1019's partial unique index and the matching
/// `ON CONFLICT` clause. Changing it requires changing 1019 too; the guard
/// against forgetting is the reflection test over `pg_indexes` in
/// `attendance_tests.rs`.
pub const AUTO_ABSENT_NOTE: &str = "Auto-marked absent (no check-in recorded)";

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
             AND notes = $3
             AND check_in_at >= date_trunc('day', NOW() AT TIME ZONE $2) AT TIME ZONE $2
             AND check_in_at < (date_trunc('day', NOW() AT TIME ZONE $2) + INTERVAL '1 day') AT TIME ZONE $2"#,
        employee_id,
        tz,
        AUTO_ABSENT_NOTE,
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

/// Auto-mark absent for the given local calendar date in `tz`, skipping
/// holidays, approved leave, and employees who already have a record on that
/// date. Idempotent *and* concurrency-safe: the `NOT EXISTS` handles the serial
/// case, and `attendance_auto_absent_one_per_employee_day` (migration 1019) is
/// what makes two simultaneous runs — the daily timer racing the startup pass,
/// or a double-clicked admin backfill — insert one placeholder rather than two.
/// The target date is a parameter (not NOW()) so missed runs can be backfilled
/// and tests can pin a date. `company_id` limits the run to one tenant (admin
/// backfill); `None` covers all companies (the daily job). Returns rows
/// genuinely inserted, so a re-run reports 0 instead of double-counting.
pub async fn mark_absent(
    executor: impl Executor<'_, Database = Postgres>,
    tz: &str,
    target_date: chrono::NaiveDate,
    company_id: Option<Uuid>,
) -> AppResult<u64> {
    let rows = mark_absent_returning(executor, tz, target_date, company_id).await?;
    Ok(rows.len() as u64)
}

pub async fn mark_absent_returning(
    executor: impl Executor<'_, Database = Postgres>,
    tz: &str,
    target_date: chrono::NaiveDate,
    company_id: Option<Uuid>,
) -> AppResult<Vec<(Uuid, Uuid)>> {
    match company_id {
        Some(cid) => {
            let rows = sqlx::query!(
                r#"INSERT INTO attendance_records
                   (company_id, employee_id, check_in_at, check_out_at, method, status, notes)
                   SELECT
                       e.company_id,
                       e.id,
                       ($2::date)::timestamp AT TIME ZONE $1,
                       ($2::date)::timestamp AT TIME ZONE $1,
                       'manual',
                       'absent',
                       $4::text
                   FROM employees e
                   LEFT JOIN working_day_config wdc
                       ON wdc.company_id = e.company_id
                       AND wdc.day_of_week = EXTRACT(DOW FROM $2::date)::int
                   WHERE e.company_id = $3
                     AND e.is_active = TRUE
                     AND e.deleted_at IS NULL
                     AND e.date_joined <= $2::date
                     AND (e.date_resigned IS NULL OR e.date_resigned >= $2::date)
                     AND CASE
                           WHEN EXISTS (
                               SELECT 1 FROM working_day_config w
                               WHERE w.company_id = e.company_id
                           ) THEN COALESCE(wdc.is_working_day, FALSE)
                           ELSE EXTRACT(DOW FROM $2::date)::int BETWEEN 1 AND 5
                         END
                     AND NOT EXISTS (
                         SELECT 1 FROM holidays h
                         WHERE h.company_id = e.company_id
                           AND (
                               h.date = $2::date
                               OR (
                                   h.is_recurring
                                   AND h.date <= $2::date
                                   AND EXTRACT(MONTH FROM h.date)::int = EXTRACT(MONTH FROM $2::date)::int
                                   AND EXTRACT(DAY FROM h.date)::int = EXTRACT(DAY FROM $2::date)::int
                               )
                           )
                     )
                     AND NOT EXISTS (
                         SELECT 1 FROM leave_requests lr
                         WHERE lr.employee_id = e.id
                           AND lr.status = 'approved'
                           AND $2::date BETWEEN lr.start_date AND lr.end_date
                     )
                     AND NOT EXISTS (
                         SELECT 1 FROM attendance_records open_ar
                         WHERE open_ar.employee_id = e.id
                           AND open_ar.check_out_at IS NULL
                           AND open_ar.check_in_at > (($2::date)::timestamp AT TIME ZONE $1) - INTERVAL '24 hours'
                           AND open_ar.check_in_at < ($2::date + 1)::timestamp AT TIME ZONE $1
                     )
                     AND NOT EXISTS (
                         SELECT 1 FROM attendance_records ar
                         WHERE ar.employee_id = e.id
                           AND ar.check_in_at >= ($2::date)::timestamp AT TIME ZONE $1
                           AND ar.check_in_at < ($2::date + 1)::timestamp AT TIME ZONE $1
                     )
                   ORDER BY e.id
                   FOR UPDATE OF e
                   ON CONFLICT (employee_id, check_in_at)
                       WHERE created_by IS NULL
                         AND status = 'absent'
                         AND method = 'manual'
                         AND notes = 'Auto-marked absent (no check-in recorded)'
                   DO NOTHING
                   RETURNING company_id, employee_id"#,
                tz,
                target_date,
                cid,
                AUTO_ABSENT_NOTE,
            )
            .fetch_all(executor)
            .await?;

            Ok(rows
                .into_iter()
                .map(|row| (row.company_id, row.employee_id))
                .collect())
        }
        None => {
            let rows = sqlx::query!(
                r#"INSERT INTO attendance_records
                   (company_id, employee_id, check_in_at, check_out_at, method, status, notes)
                   SELECT
                       e.company_id,
                       e.id,
                       ($2::date)::timestamp AT TIME ZONE $1,
                       ($2::date)::timestamp AT TIME ZONE $1,
                       'manual',
                       'absent',
                       $3::text
                   FROM employees e
                   LEFT JOIN working_day_config wdc
                       ON wdc.company_id = e.company_id
                       AND wdc.day_of_week = EXTRACT(DOW FROM $2::date)::int
                   WHERE e.is_active = TRUE
                     AND e.deleted_at IS NULL
                     AND e.date_joined <= $2::date
                     AND (e.date_resigned IS NULL OR e.date_resigned >= $2::date)
                     AND CASE
                           WHEN EXISTS (
                               SELECT 1 FROM working_day_config w
                               WHERE w.company_id = e.company_id
                           ) THEN COALESCE(wdc.is_working_day, FALSE)
                           ELSE EXTRACT(DOW FROM $2::date)::int BETWEEN 1 AND 5
                         END
                     AND NOT EXISTS (
                         SELECT 1 FROM holidays h
                         WHERE h.company_id = e.company_id
                           AND (
                               h.date = $2::date
                               OR (
                                   h.is_recurring
                                   AND h.date <= $2::date
                                   AND EXTRACT(MONTH FROM h.date)::int = EXTRACT(MONTH FROM $2::date)::int
                                   AND EXTRACT(DAY FROM h.date)::int = EXTRACT(DAY FROM $2::date)::int
                               )
                           )
                     )
                     AND NOT EXISTS (
                         SELECT 1 FROM leave_requests lr
                         WHERE lr.employee_id = e.id
                           AND lr.status = 'approved'
                           AND $2::date BETWEEN lr.start_date AND lr.end_date
                     )
                     AND NOT EXISTS (
                         SELECT 1 FROM attendance_records open_ar
                         WHERE open_ar.employee_id = e.id
                           AND open_ar.check_out_at IS NULL
                           AND open_ar.check_in_at > (($2::date)::timestamp AT TIME ZONE $1) - INTERVAL '24 hours'
                           AND open_ar.check_in_at < ($2::date + 1)::timestamp AT TIME ZONE $1
                     )
                     AND NOT EXISTS (
                         SELECT 1 FROM attendance_records ar
                         WHERE ar.employee_id = e.id
                           AND ar.check_in_at >= ($2::date)::timestamp AT TIME ZONE $1
                           AND ar.check_in_at < ($2::date + 1)::timestamp AT TIME ZONE $1
                     )
                   ORDER BY e.id
                   FOR UPDATE OF e
                   ON CONFLICT (employee_id, check_in_at)
                       WHERE created_by IS NULL
                         AND status = 'absent'
                         AND method = 'manual'
                         AND notes = 'Auto-marked absent (no check-in recorded)'
                   DO NOTHING
                   RETURNING company_id, employee_id"#,
                tz,
                target_date,
                AUTO_ABSENT_NOTE,
            )
            .fetch_all(executor)
            .await?;

            Ok(rows
                .into_iter()
                .map(|row| (row.company_id, row.employee_id))
                .collect())
        }
    }
}
