//! Read models for `GET /api/payroll/overview` — the operational dashboard.
//!
//! Every figure is read off committed runs (`processed` through `paid` — the
//! same status set `payroll_ytd` trusts) or live employee/approval records.
//! Nothing is estimated: a company that has never run payroll gets empty
//! sections, not placeholder numbers.

use std::collections::HashMap;

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::payroll::{ActionEmployeeRef, PayrollPeriodTotals, PayrollStatusCount};

/// Per-period totals across the 12 most recent committed periods, newest first.
///
/// "Committed" is `processed` through `paid` — the same status set
/// `reads/payroll.rs::payroll_ytd` trusts. `draft`, `processing` and
/// `cancelled` never contribute to reported totals.
///
/// Top 12 `(period_year, period_month)` pairs are selected BEFORE joining
/// `payroll_items`, bounding the item-level aggregation to at most 12 periods.
pub async fn period_totals(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<PayrollPeriodTotals>> {
    let rows = sqlx::query_as!(
        PayrollPeriodTotals,
        r#"WITH top_periods AS (
            SELECT DISTINCT period_year, period_month
            FROM payroll_runs
            WHERE company_id = $1
              AND status IN ('processed', 'pending_approval', 'approved', 'paid')
            ORDER BY period_year DESC, period_month DESC
            LIMIT 12
        ),
        committed AS (
            SELECT pr.id, pr.period_year, pr.period_month, pr.employee_count,
                pr.total_gross, pr.total_net, pr.total_employer_cost,
                pr.total_epf_employee, pr.total_epf_employer,
                pr.total_socso_employee, pr.total_socso_employer,
                pr.total_eis_employee, pr.total_eis_employer,
                pr.total_pcb, pr.total_zakat
            FROM payroll_runs pr
            JOIN top_periods tp
              ON tp.period_year = pr.period_year AND tp.period_month = pr.period_month
            WHERE pr.company_id = $1
              AND pr.status IN ('processed', 'pending_approval', 'approved', 'paid')
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
          AND pr.status IN ('processed', 'pending_approval', 'approved')
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

/// All five action-queue employee data-quality flags evaluated in a single scan
/// over `employees`, returning `(total_count, sample_employees)` per flag key.
pub async fn action_queue_employee_flags(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    limit: i64,
) -> AppResult<HashMap<String, (i64, Vec<ActionEmployeeRef>)>> {
    let rows = sqlx::query!(
        r#"WITH base AS (
            SELECT id, employee_number, full_name, is_active, payroll_group_id,
                   date_of_birth, date_resigned, bank_account_number,
                   epf_number, tax_identification_number
            FROM employees
            WHERE company_id = $1 AND deleted_at IS NULL
        ),
        flagged AS (
            SELECT v.flag_key, b.id, b.employee_number, b.full_name
            FROM base b
            CROSS JOIN LATERAL (VALUES
                (
                    'missing_date_of_birth',
                    b.is_active IS TRUE AND b.payroll_group_id IS NOT NULL AND b.date_of_birth IS NULL
                ),
                (
                    'inactive_without_resignation_date',
                    b.is_active IS NOT TRUE AND b.date_resigned IS NULL AND b.payroll_group_id IS NOT NULL
                ),
                (
                    'missing_payroll_group',
                    b.is_active IS TRUE AND b.payroll_group_id IS NULL
                ),
                (
                    'missing_bank_account',
                    b.is_active IS TRUE AND b.payroll_group_id IS NOT NULL
                        AND (b.bank_account_number IS NULL OR btrim(b.bank_account_number) = '')
                ),
                (
                    'missing_statutory_ids',
                    b.is_active IS TRUE AND b.payroll_group_id IS NOT NULL AND (
                        ((b.date_of_birth IS NULL OR age(CURRENT_DATE, b.date_of_birth) < INTERVAL '75 years')
                         AND (b.epf_number IS NULL OR btrim(b.epf_number) = ''))
                        OR (b.tax_identification_number IS NULL OR btrim(b.tax_identification_number) = '')
                    )
                )
            ) AS v(flag_key, matches)
            WHERE v.matches
        ),
        numbered AS (
            SELECT flag_key, id, employee_number, full_name,
                   ROW_NUMBER() OVER (PARTITION BY flag_key ORDER BY full_name, id) AS rn,
                   COUNT(*) OVER (PARTITION BY flag_key) AS total
            FROM flagged
        )
        SELECT flag_key AS "flag_key!",
               total AS "total!",
               id AS "employee_id!",
               employee_number AS "employee_number!",
               full_name AS "employee_name!"
        FROM numbered
        WHERE rn <= $2
        ORDER BY flag_key, rn"#,
        company_id,
        limit,
    )
    .fetch_all(executor)
    .await?;

    let mut out: HashMap<String, (i64, Vec<ActionEmployeeRef>)> = HashMap::new();
    for row in rows {
        let entry = out
            .entry(row.flag_key)
            .or_insert_with(|| (row.total, Vec::new()));
        entry.1.push(ActionEmployeeRef {
            employee_id: row.employee_id,
            employee_number: row.employee_number,
            employee_name: row.employee_name,
        });
    }
    Ok(out)
}

/// Department breakdown for one period across all committed runs (`processed` through `paid`),
/// matching `period_totals` so `overview` does not show period totals alongside empty departments.
pub async fn department_totals_committed(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    year: i32,
    month: i32,
) -> AppResult<Vec<crate::models::report::DepartmentPayrollRow>> {
    let rows = sqlx::query_as!(
        crate::models::report::DepartmentPayrollRow,
        r#"SELECT
            e.department,
            COUNT(DISTINCT pi.employee_id) AS "employee_count!",
            COALESCE(SUM(pi.gross_salary), 0)::bigint AS "total_gross!",
            COALESCE(SUM(pi.net_salary), 0)::bigint AS "total_net!",
            COALESCE(SUM(pi.employer_cost), 0)::bigint AS "total_employer_cost!"
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        JOIN employees e ON pi.employee_id = e.id
        WHERE pr.company_id = $1 AND pr.period_year = $2 AND pr.period_month = $3
          AND pr.status IN ('processed', 'pending_approval', 'approved', 'paid')
        GROUP BY e.department
        ORDER BY COALESCE(SUM(pi.gross_salary), 0) DESC"#,
        company_id,
        year,
        month,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}
