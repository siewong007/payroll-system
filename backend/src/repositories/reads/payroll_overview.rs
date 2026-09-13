//! Read models for `GET /api/payroll/overview` — the operational dashboard.
//!
//! Every figure is read off committed runs (`processed` through `paid` — the
//! same status set `payroll_ytd` trusts) or live employee/approval records.
//! Nothing is estimated: a company that has never run payroll gets empty
//! sections, not placeholder numbers.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::payroll::{ActionEmployeeRef, PayrollPeriodTotals, PayrollStatusCount};

/// Per-period totals across all committed runs, newest first.
///
/// "Committed" is `processed` through `paid` — the same status set
/// `reads/payroll.rs::payroll_ytd` trusts. `draft`, `processing` and
/// `cancelled` never contribute to reported totals.
///
/// Item-level sums (overtime, deductions) cannot share the run-level GROUP BY
/// without multiplying run totals per item, so they are aggregated in a CTE
/// and joined back on the period.
pub async fn period_totals(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<PayrollPeriodTotals>> {
    let rows = sqlx::query_as!(
        PayrollPeriodTotals,
        r#"WITH committed AS (
            SELECT id, period_year, period_month, employee_count,
                total_gross, total_net, total_employer_cost,
                total_epf_employee, total_epf_employer,
                total_socso_employee, total_socso_employer,
                total_eis_employee, total_eis_employer,
                total_pcb, total_zakat
            FROM payroll_runs
            WHERE company_id = $1
              AND status::text IN ('processed', 'pending_approval', 'approved', 'paid')
        ),
        item_sums AS (
            SELECT c.period_year, c.period_month,
                SUM(pi.total_overtime)::bigint AS total_overtime,
                SUM(pi.total_deductions)::bigint AS total_deductions
            FROM payroll_items pi
            JOIN committed c ON pi.payroll_run_id = c.id
            GROUP BY c.period_year, c.period_month
        )
        SELECT c.period_year, c.period_month,
            COUNT(*)::bigint AS "run_count!",
            COALESCE(SUM(c.employee_count), 0)::bigint AS "employee_count!",
            COALESCE(SUM(c.total_gross), 0)::bigint AS "total_gross!",
            COALESCE(SUM(c.total_net), 0)::bigint AS "total_net!",
            COALESCE(SUM(c.total_employer_cost), 0)::bigint AS "total_employer_cost!",
            COALESCE(SUM(c.total_epf_employee), 0)::bigint AS "total_epf_employee!",
            COALESCE(SUM(c.total_epf_employer), 0)::bigint AS "total_epf_employer!",
            COALESCE(SUM(c.total_socso_employee), 0)::bigint AS "total_socso_employee!",
            COALESCE(SUM(c.total_socso_employer), 0)::bigint AS "total_socso_employer!",
            COALESCE(SUM(c.total_eis_employee), 0)::bigint AS "total_eis_employee!",
            COALESCE(SUM(c.total_eis_employer), 0)::bigint AS "total_eis_employer!",
            COALESCE(SUM(c.total_pcb), 0)::bigint AS "total_pcb!",
            COALESCE(SUM(c.total_zakat), 0)::bigint AS "total_zakat!",
            COALESCE(i.total_overtime, 0)::bigint AS "total_overtime!",
            COALESCE(i.total_deductions, 0)::bigint AS "total_deductions!"
        FROM committed c
        LEFT JOIN item_sums i
          ON i.period_year = c.period_year AND i.period_month = c.period_month
        GROUP BY c.period_year, c.period_month, i.total_overtime, i.total_deductions
        ORDER BY c.period_year DESC, c.period_month DESC"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Every run for the company grouped by lifecycle status — the pipeline strip.
pub async fn status_counts(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<PayrollStatusCount>> {
    let rows = sqlx::query_as!(
        PayrollStatusCount,
        r#"SELECT status::text AS "status!", COUNT(*) AS "count!"
        FROM payroll_runs
        WHERE company_id = $1
        GROUP BY status
        ORDER BY status"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// A run waiting for a lifecycle action, with its group name for the queue row.
#[derive(Debug)]
pub struct RunAwaitingAction {
    pub id: Uuid,
    pub period_year: i32,
    pub period_month: i32,
    pub status: String,
    pub payroll_group_name: String,
}

pub async fn runs_awaiting_action(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<RunAwaitingAction>> {
    let rows = sqlx::query_as!(
        RunAwaitingAction,
        r#"SELECT pr.id, pr.period_year, pr.period_month,
            pr.status::text AS "status!", pg.name AS "payroll_group_name!"
        FROM payroll_runs pr
        JOIN payroll_groups pg ON pr.payroll_group_id = pg.id
        WHERE pr.company_id = $1
          AND pr.status::text IN ('processed', 'pending_approval', 'approved')
        ORDER BY pr.period_year, pr.period_month, pg.name"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Count of staged payroll entries still waiting to be consumed by a run.
pub async fn unprocessed_entry_count(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM payroll_entries
        WHERE company_id = $1 AND is_processed = FALSE"#,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}

/// The employees a data-quality check flags, with the true total via a window
/// count so the caller does not run a second COUNT query. `limit` caps the
/// sample — the action item carries `count` separately.
async fn employee_flags(
    executor: impl Executor<'_, Database = Postgres>,
    where_clause: &str,
    company_id: Uuid,
    limit: i64,
) -> AppResult<(i64, Vec<ActionEmployeeRef>)> {
    // The WHERE clause is a fixed string chosen by the caller, never request
    // input — this helper exists only so the five flag queries below share one
    // sample-with-total shape without duplicating the boilerplate.
    let sql = format!(
        r#"SELECT id AS employee_id, employee_number, full_name AS employee_name,
               COUNT(*) OVER() AS "total!"
        FROM employees
        WHERE company_id = $1 AND deleted_at IS NULL AND {where_clause}
        ORDER BY full_name
        LIMIT $2"#
    );
    let rows = sqlx::query_as::<_, (Uuid, String, String, i64)>(&sql)
        .bind(company_id)
        .bind(limit)
        .fetch_all(executor)
        .await?;
    let total = rows.first().map(|row| row.3).unwrap_or(0);
    let employees = rows
        .into_iter()
        .map(
            |(employee_id, employee_number, employee_name, _)| ActionEmployeeRef {
                employee_id,
                employee_number,
                employee_name,
            },
        )
        .collect();
    Ok((total, employees))
}

/// Active employees in a payroll group with no bank account — the run pays
/// them but no payment file can carry them.
pub async fn missing_bank_account(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    limit: i64,
) -> AppResult<(i64, Vec<ActionEmployeeRef>)> {
    employee_flags(
        executor,
        "is_active AND payroll_group_id IS NOT NULL
         AND (bank_account_number IS NULL OR btrim(bank_account_number) = '')",
        company_id,
        limit,
    )
    .await
}

/// Active employees in a payroll group with no date of birth. Blocking: the
/// engine refuses to rate them, and the preview reports the same condition
/// under the same code.
pub async fn missing_date_of_birth(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    limit: i64,
) -> AppResult<(i64, Vec<ActionEmployeeRef>)> {
    employee_flags(
        executor,
        "is_active AND payroll_group_id IS NOT NULL AND date_of_birth IS NULL",
        company_id,
        limit,
    )
    .await
}

/// Active employees with no payroll group — no run will ever include them.
pub async fn missing_payroll_group(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    limit: i64,
) -> AppResult<(i64, Vec<ActionEmployeeRef>)> {
    employee_flags(
        executor,
        "is_active AND payroll_group_id IS NULL",
        company_id,
        limit,
    )
    .await
}

/// Inactive employees with no resignation date — the population rule that
/// blocks a run outright (`inactive_without_resignation_date` in preview).
pub async fn inactive_without_resignation(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    limit: i64,
) -> AppResult<(i64, Vec<ActionEmployeeRef>)> {
    employee_flags(
        executor,
        "NOT is_active AND date_resigned IS NULL AND payroll_group_id IS NOT NULL",
        company_id,
        limit,
    )
    .await
}

/// Citizens/PRs with no EPF number, or anyone with no TIN — statutory filings
/// (EPF Borang A, EA/CP8D) are produced per employee and a missing identifier
/// produces a row the submission rejects.
pub async fn missing_statutory_ids(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    limit: i64,
) -> AppResult<(i64, Vec<ActionEmployeeRef>)> {
    employee_flags(
        executor,
        "is_active AND payroll_group_id IS NOT NULL AND (
            (residency_status <> 'foreigner'
             AND (epf_number IS NULL OR btrim(epf_number) = ''))
            OR (tax_identification_number IS NULL OR btrim(tax_identification_number) = '')
         )",
        company_id,
        limit,
    )
    .await
}
