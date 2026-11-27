//! Dynamic / cross-table attendance reads (filtered list, my-attendance, summary,
//! export rows).
//!
//! The filtered reads build SQL at runtime, so they use `sqlx::QueryBuilder`
//! (not the compile-checked macros) and are not part of the offline cache.
//! `push_bind` keeps each predicate and its bound value together, so there is no
//! manual parameter-index bookkeeping to drift out of sync. Fixed queries here
//! (`auto_absent_targets`) still use the checked macros.
//!
//! Date-range filters are written as *sargable* ranges on the raw `timestamptz`
//! (`check_in_at >= <local midnight of from> AND check_in_at < <local midnight
//! after to>`) instead of wrapping the column in `AT TIME ZONE`, so the
//! `(company_id, check_in_at)` / `(employee_id, check_in_at)` btrees serve them.
//! The semantics are identical to bucketing by local calendar date.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use sqlx::{Executor, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::attendance::{
    AttendanceExportQuery, AttendanceListQuery, AttendanceRecord, AttendanceRecordWithEmployee,
    AttendanceSummaryItem, AttendanceSummaryQuery, EmployeeAttendanceDeviceWithEmployee,
    PaginatedAttendance, PairedCheckInCluster, SharedDeviceEmployeeEntry, SharedDeviceEvent,
};

/// One tenant's inputs for the auto-absent catch-up.
pub struct AutoAbsentTarget {
    pub company_id: Uuid,
    /// The company's own zone — the calendar its placeholders are written on,
    /// and the one `delete_auto_absent_today` and the reads layer bucket by.
    /// Deliberately raw: the caller parses it so a corrupt value skips that one
    /// tenant instead of being silently marked on somebody else's calendar.
    pub timezone: String,
    /// Last local date the job completed for this company; `None` on a company
    /// that has never been run.
    pub last_run_date: Option<NaiveDate>,
}

/// Every active company with the zone and bookmark the auto-absent job needs.
///
/// `ORDER BY c.id` is load-bearing: without it the tenant order varied run to
/// run, so which companies were reached before an abort was nondeterministic
/// and the log stream could not be compared tick to tick.
pub async fn auto_absent_targets(
    executor: impl Executor<'_, Database = Postgres>,
) -> AppResult<Vec<AutoAbsentTarget>> {
    // Wrap the nullable LEFT JOIN column in `COALESCE(..., literal)` so Postgres
    // describes the output expression identically even if tests alter triggers
    // on `company_work_schedules`, while avoiding the per-row correlated SubPlan.
    let rows = sqlx::query!(
        r#"SELECT c.id AS "company_id!",
                  COALESCE(ws.timezone, 'Asia/Kuala_Lumpur') AS "timezone!",
                  c.auto_absent_last_run_date AS "last_run_date?"
           FROM companies c
           LEFT JOIN company_work_schedules ws
             ON ws.company_id = c.id
            AND ws.is_default = TRUE
           WHERE c.is_active = TRUE
           ORDER BY c.id"#
    )
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| AutoAbsentTarget {
            company_id: r.company_id,
            timezone: r.timezone,
            last_run_date: r.last_run_date,
        })
        .collect())
}

fn resolve_pagination(q: &AttendanceListQuery) -> (i64, i64, i64) {
    let per_page = q.per_page.unwrap_or(50).clamp(1, 200);
    let page = q.page.unwrap_or(1).clamp(1, 100_000);
    let offset = (page - 1).saturating_mul(per_page);
    (page, per_page, offset)
}

/// `ar.check_in_at >= <local midnight of date, in tz>`
fn push_date_from(qb: &mut QueryBuilder<Postgres>, tz: &str, date_from: NaiveDate) {
    qb.push(" AND ar.check_in_at >= (");
    qb.push_bind(date_from);
    qb.push("::date)::timestamp AT TIME ZONE ");
    qb.push_bind(tz.to_owned());
}

/// `ar.check_in_at < <local midnight after date, in tz>` (inclusive local date)
fn push_date_to(qb: &mut QueryBuilder<Postgres>, tz: &str, date_to: NaiveDate) {
    qb.push(" AND ar.check_in_at < (");
    qb.push_bind(date_to);
    qb.push("::date + 1)::timestamp AT TIME ZONE ");
    qb.push_bind(tz.to_owned());
}

/// Shared optional filters for the admin list / export queries.
fn push_list_filters(qb: &mut QueryBuilder<Postgres>, tz: &str, q: &AttendanceListQuery) {
    if let Some(eid) = q.employee_id {
        qb.push(" AND ar.employee_id = ");
        qb.push_bind(eid);
    }
    if let Some(df) = q.date_from {
        push_date_from(qb, tz, df);
    }
    if let Some(dt) = q.date_to {
        push_date_to(qb, tz, dt);
    }
    if let Some(ref st) = q.status {
        qb.push(" AND ar.status = ");
        qb.push_bind(st.clone());
    }
    if let Some(ref m) = q.method {
        qb.push(" AND ar.method = ");
        qb.push_bind(m.clone());
    }
    if q.open_only.unwrap_or(false) {
        // Sessions still open (never checked out). Absent placeholders are
        // closed rows, but exclude the status defensively anyway.
        qb.push(" AND ar.check_out_at IS NULL AND ar.status <> 'absent'");
    }
    if q.outside_geofence_only.unwrap_or(false) {
        qb.push(" AND ar.is_outside_geofence = TRUE");
    }
    if q.offsite_network_only.unwrap_or(false) {
        qb.push(" AND ar.is_offsite_network = TRUE");
    }
    if q.flagged_only.unwrap_or(false) {
        qb.push(" AND cardinality(ar.anomaly_flags) > 0");
    }
}

const RECORD_WITH_EMPLOYEE_COLUMNS: &str = r#"
            ar.id, ar.company_id, ar.employee_id,
            e.employee_number, e.full_name, e.department,
            ar.check_in_at, ar.check_out_at,
            ar.method, ar.status,
            ar.latitude, ar.longitude,
            ar.checkout_latitude, ar.checkout_longitude,
            ar.notes,
            ar.hours_worked, ar.overtime_hours, ar.is_outside_geofence,
            ar.is_offsite_network,
            ar.check_in_device_id, ar.check_out_device_id,
            d_in.device_label AS check_in_device_label,
            d_out.device_label AS check_out_device_label,
            ar.check_in_passkey_id,
            ar.check_in_user_agent, ar.check_out_user_agent,
            ar.is_untrusted_device, ar.anomaly_flags,
            ar.check_in_photo_url, ar.check_out_photo_url,
            ar.check_in_photo_purged_at, ar.check_out_photo_purged_at,
            ar.created_at"#;

/// Admin attendance list (joined with employee details), with optional filters + paging.
/// Dates are bucketed by the company's local calendar in `tz`.
pub async fn list_with_employee(
    pool: &PgPool,
    company_id: Uuid,
    tz: &str,
    q: &AttendanceListQuery,
) -> AppResult<PaginatedAttendance<AttendanceRecordWithEmployee>> {
    let (page, per_page, offset) = resolve_pagination(q);

    let mut count_qb =
        QueryBuilder::new("SELECT COUNT(*) FROM attendance_records ar WHERE ar.company_id = ");
    count_qb.push_bind(company_id);
    push_list_filters(&mut count_qb, tz, q);
    let total: i64 = count_qb.build_query_scalar().fetch_one(pool).await?;

    let mut data_qb = QueryBuilder::new("SELECT");
    data_qb.push(RECORD_WITH_EMPLOYEE_COLUMNS);
    // Tenant equality on the JOIN as defence in depth: even if a record ever
    // pointed at another company's employee, it must not leak their details.
    data_qb.push(
        r#"
           FROM attendance_records ar
           JOIN employees e ON ar.employee_id = e.id AND e.company_id = ar.company_id
           LEFT JOIN employee_attendance_devices d_in
             ON d_in.id = ar.check_in_device_id AND d_in.company_id = ar.company_id
           LEFT JOIN employee_attendance_devices d_out
             ON d_out.id = ar.check_out_device_id AND d_out.company_id = ar.company_id
           WHERE ar.company_id = "#,
    );
    data_qb.push_bind(company_id);
    push_list_filters(&mut data_qb, tz, q);
    data_qb.push(" ORDER BY ar.check_in_at DESC LIMIT ");
    data_qb.push_bind(per_page);
    data_qb.push(" OFFSET ");
    data_qb.push_bind(offset);

    let data = data_qb
        .build_query_as::<AttendanceRecordWithEmployee>()
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

/// An employee's own attendance, with optional date filters + paging.
pub async fn list_for_employee(
    pool: &PgPool,
    employee_id: Uuid,
    tz: &str,
    q: &AttendanceListQuery,
) -> AppResult<PaginatedAttendance<AttendanceRecord>> {
    let (page, per_page, offset) = resolve_pagination(q);

    let push_filters = |qb: &mut QueryBuilder<Postgres>| {
        if let Some(df) = q.date_from {
            push_date_from(qb, tz, df);
        }
        if let Some(dt) = q.date_to {
            push_date_to(qb, tz, dt);
        }
    };

    let mut count_qb =
        QueryBuilder::new("SELECT COUNT(*) FROM attendance_records ar WHERE ar.employee_id = ");
    count_qb.push_bind(employee_id);
    push_filters(&mut count_qb);
    let total: i64 = count_qb.build_query_scalar().fetch_one(pool).await?;

    let mut data_qb =
        QueryBuilder::new("SELECT ar.* FROM attendance_records ar WHERE ar.employee_id = ");
    data_qb.push_bind(employee_id);
    push_filters(&mut data_qb);
    data_qb.push(" ORDER BY ar.check_in_at DESC LIMIT ");
    data_qb.push_bind(per_page);
    data_qb.push(" OFFSET ");
    data_qb.push_bind(offset);

    let data = data_qb
        .build_query_as::<AttendanceRecord>()
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

/// Per-employee aggregate for a date range. Employees with no records still appear.
///
/// Counts are per distinct local *day*, not per record: an employee with a
/// split shift (two present records in one day) counts one present day, and a
/// day carrying both an auto-absent placeholder and a real late check-in
/// counts once as late. Precedence within a day: late > half_day > present >
/// absent.
pub async fn summary(
    pool: &PgPool,
    company_id: Uuid,
    tz: &str,
    q: &AttendanceSummaryQuery,
) -> AppResult<Vec<AttendanceSummaryItem>> {
    let mut qb = QueryBuilder::new(
        r#"SELECT
               e.id              AS employee_id,
               e.employee_number,
               e.full_name,
               e.department,
               COUNT(*) FILTER (WHERE d.day_status = 'present')  AS present_days,
               COUNT(*) FILTER (WHERE d.day_status = 'late')     AS late_days,
               COUNT(*) FILTER (WHERE d.day_status = 'absent')   AS absent_days,
               COUNT(*) FILTER (WHERE d.day_status = 'half_day') AS half_days,
               COALESCE(SUM(d.hours_worked),    0)::NUMERIC(10,2) AS total_hours,
               COALESCE(SUM(d.overtime_hours),  0)::NUMERIC(10,2) AS overtime_hours,
               COUNT(*) FILTER (WHERE d.has_open) AS unchecked_out_days
           FROM employees e
           LEFT JOIN (
               SELECT
                   ar.employee_id,
                   (ar.check_in_at AT TIME ZONE "#,
    );
    qb.push_bind(tz.to_owned());
    qb.push(
        r#")::date AS local_date,
                   CASE
                       WHEN BOOL_OR(ar.status = 'late')     THEN 'late'
                       WHEN BOOL_OR(ar.status = 'half_day') THEN 'half_day'
                       WHEN BOOL_OR(ar.status = 'present')  THEN 'present'
                       ELSE 'absent'
                   END AS day_status,
                   SUM(ar.hours_worked)   AS hours_worked,
                   SUM(ar.overtime_hours) AS overtime_hours,
                   BOOL_OR(ar.check_out_at IS NULL AND ar.status <> 'absent') AS has_open
               FROM attendance_records ar
               WHERE ar.company_id = "#,
    );
    qb.push_bind(company_id);
    push_date_from(&mut qb, tz, q.date_from);
    push_date_to(&mut qb, tz, q.date_to);
    // Group by ordinal: the local-date expression uses a bound parameter, and
    // repeating it with a *different* placeholder is not recognised as the same
    // expression ("column must appear in the GROUP BY clause").
    qb.push(
        r#"
               GROUP BY 1, 2
           ) d ON d.employee_id = e.id
           WHERE e.company_id = "#,
    );
    qb.push_bind(company_id);
    qb.push(" AND e.is_active = TRUE AND e.deleted_at IS NULL");
    if let Some(eid) = q.employee_id {
        qb.push(" AND e.id = ");
        qb.push_bind(eid);
    }
    if let Some(ref d) = q.department {
        qb.push(" AND e.department = ");
        qb.push_bind(d.clone());
    }
    qb.push(
        r#"
           GROUP BY e.id, e.employee_number, e.full_name, e.department
           ORDER BY e.full_name"#,
    );

    Ok(qb
        .build_query_as::<AttendanceSummaryItem>()
        .fetch_all(pool)
        .await?)
}

/// Rows for CSV export (joined with employee details), with optional filters.
///
/// The service guarantees a bounded date range before calling this and passes
/// its row ceiling as `limit`. Deliberately `LIMIT limit + 1`: the caller reads
/// the extra row as "there was more" and fails the export, rather than handing
/// an admin a truncated CSV indistinguishable from a complete one. `, ar.id`
/// makes that truncation deterministic — `check_in_at` alone is not unique, so
/// two identical exports could otherwise drop different rows.
pub async fn export_rows(
    pool: &PgPool,
    company_id: Uuid,
    tz: &str,
    q: &AttendanceExportQuery,
    limit: i64,
) -> AppResult<Vec<AttendanceRecordWithEmployee>> {
    let list_filters = AttendanceListQuery {
        employee_id: q.employee_id,
        date_from: q.date_from,
        date_to: q.date_to,
        status: q.status.clone(),
        method: q.method.clone(),
        open_only: q.open_only,
        outside_geofence_only: q.outside_geofence_only,
        offsite_network_only: q.offsite_network_only,
        flagged_only: q.flagged_only,
        page: None,
        per_page: None,
    };

    let mut qb = QueryBuilder::new("SELECT");
    qb.push(RECORD_WITH_EMPLOYEE_COLUMNS);
    qb.push(
        r#"
           FROM attendance_records ar
           JOIN employees e ON ar.employee_id = e.id AND e.company_id = ar.company_id
           LEFT JOIN employee_attendance_devices d_in
             ON d_in.id = ar.check_in_device_id AND d_in.company_id = ar.company_id
           LEFT JOIN employee_attendance_devices d_out
             ON d_out.id = ar.check_out_device_id AND d_out.company_id = ar.company_id
           WHERE ar.company_id = "#,
    );
    qb.push_bind(company_id);
    push_list_filters(&mut qb, tz, &list_filters);
    qb.push(" ORDER BY ar.check_in_at DESC, ar.id LIMIT ");
    qb.push_bind(limit.saturating_add(1));

    Ok(qb
        .build_query_as::<AttendanceRecordWithEmployee>()
        .fetch_all(pool)
        .await?)
}

pub async fn list_devices_with_employee(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    status_filter: Option<&str>,
) -> AppResult<Vec<EmployeeAttendanceDeviceWithEmployee>> {
    let rows = sqlx::query_as!(
        EmployeeAttendanceDeviceWithEmployee,
        r#"SELECT d.id, d.company_id, d.employee_id,
                  e.employee_number, e.full_name, e.department,
                  d.device_label, d.public_key_fingerprint, d.user_agent,
                  d.status, d.approved_by, d.approved_at,
                  d.revoked_by, d.revoked_at, d.revoke_reason,
                  d.last_used_at, d.created_at
           FROM employee_attendance_devices d
           JOIN employees e ON e.id = d.employee_id AND e.company_id = d.company_id
           WHERE d.company_id = $1
             AND ($2::text IS NULL OR d.status = $2)
           ORDER BY
             CASE d.status WHEN 'pending' THEN 0 WHEN 'approved' THEN 1 ELSE 2 END,
             d.created_at DESC"#,
        company_id,
        status_filter,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn count_flagged_records_in_range(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    tz: &str,
    date_from: NaiveDate,
    date_to: NaiveDate,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!"
           FROM attendance_records ar
           WHERE ar.company_id = $1
             AND cardinality(ar.anomaly_flags) > 0
             AND ar.check_in_at >= ($2::date)::timestamp AT TIME ZONE $4
             AND ar.check_in_at < ($3::date + 1)::timestamp AT TIME ZONE $4"#,
        company_id,
        date_from,
        date_to,
        tz,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}

pub async fn shared_device_events(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    tz: &str,
    date_from: NaiveDate,
    date_to: NaiveDate,
) -> AppResult<Vec<SharedDeviceEvent>> {
    let rows = sqlx::query!(
        r#"WITH cookie_events AS (
               SELECT (ar.check_in_at AT TIME ZONE $4)::date AS local_date,
                      ar.check_in_cookie_id AS cookie_id,
                      ar.employee_id,
                      ar.check_in_at AS event_at
               FROM attendance_records ar
               WHERE ar.company_id = $1
                 AND ar.check_in_cookie_id IS NOT NULL
                 AND ar.check_in_at >= ($2::date)::timestamp AT TIME ZONE $4
                 AND ar.check_in_at < ($3::date + 1)::timestamp AT TIME ZONE $4
               UNION ALL
               SELECT (ar.check_out_at AT TIME ZONE $4)::date AS local_date,
                      ar.check_out_cookie_id AS cookie_id,
                      ar.employee_id,
                      ar.check_out_at AS event_at
               FROM attendance_records ar
               WHERE ar.company_id = $1
                 AND ar.check_out_cookie_id IS NOT NULL
                 AND ar.check_out_at IS NOT NULL
                 AND ar.check_out_at >= ($2::date)::timestamp AT TIME ZONE $4
                 AND ar.check_out_at < ($3::date + 1)::timestamp AT TIME ZONE $4
           ),
           per_emp_day AS (
               SELECT local_date, cookie_id, employee_id, MIN(event_at) AS first_event_at
               FROM cookie_events
               GROUP BY 1, 2, 3
           ),
           shared AS (
               SELECT local_date, cookie_id
               FROM per_emp_day
               GROUP BY 1, 2
               HAVING COUNT(*) >= 2
           )
           SELECT s.local_date AS "local_date!",
                  SUBSTRING(encode(sha256(s.cookie_id::text::bytea), 'hex') FROM 1 FOR 10) AS "device_tag!",
                  ped.employee_id AS "employee_id!",
                  e.employee_number AS "employee_number!",
                  e.full_name AS "full_name!",
                  ped.first_event_at AS "check_in_at!"
           FROM shared s
           JOIN per_emp_day ped ON ped.local_date = s.local_date AND ped.cookie_id = s.cookie_id
           JOIN employees e ON e.id = ped.employee_id AND e.company_id = $1
           ORDER BY s.local_date DESC, "device_tag!", ped.first_event_at ASC
           LIMIT 200"#,
        company_id,
        date_from,
        date_to,
        tz,
    )
    .fetch_all(executor)
    .await?;

    let mut grouped: BTreeMap<(NaiveDate, String), Vec<SharedDeviceEmployeeEntry>> =
        BTreeMap::new();
    for r in rows {
        grouped
            .entry((r.local_date, r.device_tag))
            .or_default()
            .push(SharedDeviceEmployeeEntry {
                employee_id: r.employee_id,
                employee_number: r.employee_number,
                full_name: r.full_name,
                check_in_at: r.check_in_at,
            });
    }

    let mut events: Vec<SharedDeviceEvent> = grouped
        .into_iter()
        .map(|((local_date, device_tag), employees)| SharedDeviceEvent {
            local_date,
            device_tag,
            employee_count: employees.len() as i64,
            employees,
        })
        .collect();
    events.sort_by_key(|a| std::cmp::Reverse(a.local_date));
    Ok(events)
}

pub async fn paired_checkins(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    tz: &str,
    date_from: NaiveDate,
    date_to: NaiveDate,
) -> AppResult<Vec<PairedCheckInCluster>> {
    let rows = sqlx::query_as!(
        PairedCheckInCluster,
        r#"WITH daily AS (
               SELECT ar.employee_id,
                      (ar.check_in_at AT TIME ZONE $4)::date AS local_date,
                      MIN(ar.check_in_at) AS check_in_at,
                      MAX(ar.check_out_at) AS check_out_at,
                      (ARRAY_AGG(ar.latitude ORDER BY ar.check_in_at) FILTER (WHERE ar.latitude IS NOT NULL))[1] AS lat,
                      (ARRAY_AGG(ar.longitude ORDER BY ar.check_in_at) FILTER (WHERE ar.longitude IS NOT NULL))[1] AS lng
               FROM attendance_records ar
               WHERE ar.company_id = $1
                 AND ar.status <> 'absent'
                 AND ar.method <> 'manual'
                 AND ar.check_in_at >= ($2::date)::timestamp AT TIME ZONE $4
                 AND ar.check_in_at < ($3::date + 1)::timestamp AT TIME ZONE $4
               GROUP BY ar.employee_id, (ar.check_in_at AT TIME ZONE $4)::date
           )
           SELECT a.employee_id AS "employee_a_id!",
                  ea.employee_number AS "employee_a_number!",
                  ea.full_name AS "employee_a_name!",
                  b.employee_id AS "employee_b_id!",
                  eb.employee_number AS "employee_b_number!",
                  eb.full_name AS "employee_b_name!",
                  COUNT(*)::bigint AS "paired_days!",
                  COUNT(*) FILTER (
                      WHERE a.check_out_at IS NOT NULL
                        AND b.check_out_at IS NOT NULL
                        AND ABS(EXTRACT(EPOCH FROM (a.check_out_at - b.check_out_at))) <= 60
                  )::bigint AS "paired_checkout_days!",
                  COUNT(*) FILTER (
                      WHERE a.lat IS NOT NULL AND a.lng IS NOT NULL
                        AND b.lat IS NOT NULL AND b.lng IS NOT NULL
                        AND ABS(a.lat - b.lat) <= 0.00015
                        AND ABS(a.lng - b.lng) <= 0.00015
                  )::bigint AS "co_located_days!",
                  ROUND(AVG(ABS(EXTRACT(EPOCH FROM (a.check_in_at - b.check_in_at)))))::bigint AS "avg_gap_seconds!",
                  MAX(a.local_date) AS "last_paired_date!"
           FROM daily a
           JOIN daily b
             ON a.local_date = b.local_date
            AND a.employee_id < b.employee_id
            AND ABS(EXTRACT(EPOCH FROM (a.check_in_at - b.check_in_at))) <= 60
           JOIN employees ea ON ea.id = a.employee_id AND ea.company_id = $1
           JOIN employees eb ON eb.id = b.employee_id AND eb.company_id = $1
           GROUP BY a.employee_id, ea.employee_number, ea.full_name,
                    b.employee_id, eb.employee_number, eb.full_name
           HAVING COUNT(*) >= 3
           ORDER BY COUNT(*) DESC, MAX(a.local_date) DESC
           LIMIT 100"#,
        company_id,
        date_from,
        date_to,
        tz,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}
