//! Dynamic / cross-table attendance reads (filtered list, my-attendance, summary,
//! export rows).
//!
//! These build SQL at runtime from optional filters, so they use `sqlx::QueryBuilder`
//! (not the compile-checked macros) and are not part of the offline cache.
//! `push_bind` keeps each predicate and its bound value together, so there is no
//! manual parameter-index bookkeeping to drift out of sync.
//!
//! Date-range filters are written as *sargable* ranges on the raw `timestamptz`
//! (`check_in_at >= <local midnight of from> AND check_in_at < <local midnight
//! after to>`) instead of wrapping the column in `AT TIME ZONE`, so the
//! `(company_id, check_in_at)` / `(employee_id, check_in_at)` btrees serve them.
//! The semantics are identical to bucketing by local calendar date.

use chrono::NaiveDate;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::attendance::{
    AttendanceExportQuery, AttendanceListQuery, AttendanceRecord, AttendanceRecordWithEmployee,
    AttendanceSummaryItem, AttendanceSummaryQuery, PaginatedAttendance,
};

fn resolve_pagination(q: &AttendanceListQuery) -> (i64, i64, i64) {
    let per_page = q.per_page.unwrap_or(50).clamp(1, 200);
    let page = q.page.unwrap_or(1).max(1);
    let offset = (page - 1) * per_page;
    (page, per_page, offset)
}

/// `ar.check_in_at >= <local midnight of date, in tz>`
fn push_date_from(qb: &mut QueryBuilder<'_, Postgres>, tz: &str, date_from: NaiveDate) {
    qb.push(" AND ar.check_in_at >= (");
    qb.push_bind(date_from);
    qb.push("::date)::timestamp AT TIME ZONE ");
    qb.push_bind(tz.to_owned());
}

/// `ar.check_in_at < <local midnight after date, in tz>` (inclusive local date)
fn push_date_to(qb: &mut QueryBuilder<'_, Postgres>, tz: &str, date_to: NaiveDate) {
    qb.push(" AND ar.check_in_at < (");
    qb.push_bind(date_to);
    qb.push("::date + 1)::timestamp AT TIME ZONE ");
    qb.push_bind(tz.to_owned());
}

/// Shared optional filters for the admin list / export queries.
fn push_list_filters(qb: &mut QueryBuilder<'_, Postgres>, tz: &str, q: &AttendanceListQuery) {
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

    let push_filters = |qb: &mut QueryBuilder<'_, Postgres>| {
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
/// The service guarantees a bounded date range before calling this.
pub async fn export_rows(
    pool: &PgPool,
    company_id: Uuid,
    tz: &str,
    q: &AttendanceExportQuery,
) -> AppResult<Vec<AttendanceRecordWithEmployee>> {
    let list_filters = AttendanceListQuery {
        employee_id: q.employee_id,
        date_from: q.date_from,
        date_to: q.date_to,
        status: q.status.clone(),
        method: q.method.clone(),
        open_only: None,
        page: None,
        per_page: None,
    };

    let mut qb = QueryBuilder::new("SELECT");
    qb.push(RECORD_WITH_EMPLOYEE_COLUMNS);
    qb.push(
        r#"
           FROM attendance_records ar
           JOIN employees e ON ar.employee_id = e.id AND e.company_id = ar.company_id
           WHERE ar.company_id = "#,
    );
    qb.push_bind(company_id);
    push_list_filters(&mut qb, tz, &list_filters);
    qb.push(" ORDER BY ar.check_in_at DESC");

    Ok(qb
        .build_query_as::<AttendanceRecordWithEmployee>()
        .fetch_all(pool)
        .await?)
}
