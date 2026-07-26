//! Read models for the admin dashboard: the most-recent payroll run, this
//! year's employer-cost totals, and the active head-count per department.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use chrono::NaiveDate;

use crate::core::error::AppResult;
use crate::models::dashboard::{
    AttendanceExceptionTotals, DepartmentCountRow, LastPayrollRow, PendingApprovalCounts,
    YtdEmployerTotals,
};

/// The most recent non-cancelled/non-draft run for the company, if any.
pub async fn last_payroll(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Option<LastPayrollRow>> {
    let row = sqlx::query_as!(
        LastPayrollRow,
        r#"SELECT
            period_year::text || '-' || LPAD(period_month::text, 2, '0') AS "period!",
            total_net, total_gross, employee_count
        FROM payroll_runs
        WHERE company_id = $1 AND status NOT IN ('cancelled', 'draft')
        ORDER BY period_year DESC, period_month DESC
        LIMIT 1"#,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

/// Year-to-date employer-cost totals across non-cancelled/non-draft runs.
pub async fn ytd_employer_totals(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    year: i32,
) -> AppResult<YtdEmployerTotals> {
    let totals = sqlx::query_as!(
        YtdEmployerTotals,
        r#"SELECT
            COALESCE(SUM(total_gross), 0)::BIGINT AS "total_gross!",
            COALESCE(SUM(total_epf_employer), 0)::BIGINT AS "total_epf_employer!",
            COALESCE(SUM(total_socso_employer), 0)::BIGINT AS "total_socso_employer!",
            COALESCE(SUM(total_eis_employer), 0)::BIGINT AS "total_eis_employer!"
        FROM payroll_runs
        WHERE company_id = $1 AND period_year = $2
        AND status NOT IN ('cancelled', 'draft')"#,
        company_id,
        year,
    )
    .fetch_one(executor)
    .await?;
    Ok(totals)
}

/// Attendance exceptions for one company over a local-date window, as a single
/// pass of `FILTER` aggregates.
///
/// `date_from`/`date_to` are inclusive local dates in `tz`. The bounds are
/// applied to the raw `check_in_at timestamptz` rather than wrapping the column
/// in `AT TIME ZONE`, so the `(company_id, check_in_at)` index still serves the
/// query — the same sargability rule the attendance reads follow.
pub async fn attendance_exception_totals(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    tz: &str,
    date_from: NaiveDate,
    date_to: NaiveDate,
) -> AppResult<AttendanceExceptionTotals> {
    let totals = sqlx::query_as!(
        AttendanceExceptionTotals,
        r#"SELECT
            COUNT(*) FILTER (WHERE ar.status = 'late')             AS "late_count!",
            COUNT(*) FILTER (WHERE ar.status = 'absent')           AS "absent_count!",
            COUNT(*) FILTER (WHERE ar.check_out_at IS NULL
                             AND ar.status <> 'absent')            AS "open_session_count!",
            COUNT(*) FILTER (WHERE ar.is_outside_geofence)         AS "outside_geofence_count!",
            COUNT(*) FILTER (WHERE ar.method = 'manual')           AS "manual_entry_count!"
        FROM attendance_records ar
        WHERE ar.company_id = $1
          AND ar.check_in_at >= ($2::date)::timestamp AT TIME ZONE $4
          AND ar.check_in_at <  ($3::date + 1)::timestamp AT TIME ZONE $4"#,
        company_id,
        date_from,
        date_to,
        tz,
    )
    .fetch_one(executor)
    .await?;
    Ok(totals)
}

/// Depth of each approval queue. Deliberately unbounded by date: a request left
/// pending for months is exactly what a rolling window would hide.
pub async fn pending_approval_counts(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<PendingApprovalCounts> {
    let counts = sqlx::query_as!(
        PendingApprovalCounts,
        r#"SELECT
            (SELECT COUNT(*) FROM leave_requests
              WHERE company_id = $1 AND status = 'pending')         AS "pending_leave!",
            (SELECT COUNT(*) FROM claims
              WHERE company_id = $1 AND status = 'pending')         AS "pending_claims!",
            (SELECT COUNT(*) FROM overtime_applications
              WHERE company_id = $1 AND status = 'pending')         AS "pending_overtime!""#,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(counts)
}

/// Active (non-deleted) head-count per department, busiest first.
pub async fn department_counts(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<DepartmentCountRow>> {
    let rows = sqlx::query_as!(
        DepartmentCountRow,
        r#"SELECT department, COUNT(*) AS "count!"
        FROM employees
        WHERE company_id = $1 AND is_active = TRUE AND deleted_at IS NULL
        GROUP BY department ORDER BY COUNT(*) DESC"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}
