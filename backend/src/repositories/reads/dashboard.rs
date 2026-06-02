//! Read models for the admin dashboard: the most-recent payroll run, this
//! year's employer-cost totals, and the active head-count per department.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

#[derive(Debug, sqlx::FromRow)]
pub struct LastPayrollRow {
    pub period: String,
    pub total_net: i64,
    pub total_gross: i64,
    pub employee_count: i32,
}

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

#[derive(Debug, sqlx::FromRow)]
pub struct YtdEmployerTotals {
    pub total_gross: i64,
    pub total_epf_employer: i64,
    pub total_socso_employer: i64,
    pub total_eis_employer: i64,
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

#[derive(Debug, sqlx::FromRow)]
pub struct DepartmentCountRow {
    pub department: Option<String>,
    pub count: i64,
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
