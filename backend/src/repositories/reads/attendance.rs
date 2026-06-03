//! Dynamic / cross-table attendance reads (filtered list, my-attendance, summary,
//! export rows).
//!
//! These build SQL at runtime from optional filters, so they use the runtime query
//! builder (not the compile-checked macros) and are not part of the offline cache.

use sqlx::PgPool;
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

/// Admin attendance list (joined with employee details), with optional filters + paging.
pub async fn list_with_employee(
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

/// An employee's own attendance, with optional date filters + paging.
pub async fn list_for_employee(
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

/// Per-employee aggregate for a date range. Employees with no records still appear.
pub async fn summary(
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
        .bind(q.date_from)
        .bind(q.date_to);

    if let Some(eid) = q.employee_id {
        query = query.bind(eid);
    }
    if let Some(ref d) = q.department {
        query = query.bind(d);
    }

    Ok(query.fetch_all(pool).await?)
}

/// Rows for CSV export (joined with employee details), with optional filters.
pub async fn export_rows(
    pool: &PgPool,
    company_id: Uuid,
    q: &AttendanceExportQuery,
) -> AppResult<Vec<AttendanceRecordWithEmployee>> {
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

    Ok(dq.fetch_all(pool).await?)
}
