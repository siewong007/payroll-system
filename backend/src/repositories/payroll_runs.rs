//! Data access for the `payroll_runs` table.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::payroll::PayrollRun;

/// Count non-cancelled runs for a (company, group, period) — used to block duplicates.
pub async fn count_active_for_period(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    payroll_group_id: Uuid,
    year: i32,
    month: i32,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM payroll_runs
        WHERE company_id = $1 AND payroll_group_id = $2
        AND period_year = $3 AND period_month = $4
        AND status NOT IN ('cancelled')"#,
        company_id,
        payroll_group_id,
        year,
        month,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}

/// Insert a new run in `processing` status.
#[allow(clippy::too_many_arguments)]
pub async fn insert_processing(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    company_id: Uuid,
    payroll_group_id: Uuid,
    year: i32,
    month: i32,
    period_start: NaiveDate,
    period_end: NaiveDate,
    pay_date: NaiveDate,
    processed_by: Uuid,
    notes: Option<String>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO payroll_runs
        (id, company_id, payroll_group_id, period_year, period_month,
         period_start, period_end, pay_date, status, processed_by, processed_at, notes, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'processing', $9, NOW(), $10, $9)"#,
        run_id,
        company_id,
        payroll_group_id,
        year,
        month,
        period_start,
        period_end,
        pay_date,
        processed_by,
        notes,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Set the run to `processed` and write its aggregate totals.
#[allow(clippy::too_many_arguments)]
pub async fn update_totals(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    total_gross: i64,
    total_net: i64,
    total_employer_cost: i64,
    total_epf_employee: i64,
    total_epf_employer: i64,
    total_socso_employee: i64,
    total_socso_employer: i64,
    total_eis_employee: i64,
    total_eis_employer: i64,
    total_pcb: i64,
    total_zakat: i64,
    employee_count: i32,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE payroll_runs SET
        status = 'processed',
        total_gross = $2, total_net = $3, total_employer_cost = $4,
        total_epf_employee = $5, total_epf_employer = $6,
        total_socso_employee = $7, total_socso_employer = $8,
        total_eis_employee = $9, total_eis_employer = $10,
        total_pcb = $11, total_zakat = $12,
        employee_count = $13, updated_at = NOW()
        WHERE id = $1"#,
        run_id,
        total_gross,
        total_net,
        total_employer_cost,
        total_epf_employee,
        total_epf_employer,
        total_socso_employee,
        total_socso_employer,
        total_eis_employee,
        total_eis_employer,
        total_pcb,
        total_zakat,
        employee_count,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn get_by_id(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
) -> AppResult<Option<PayrollRun>> {
    let run = sqlx::query_as!(
        PayrollRun,
        r#"SELECT id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text AS "status!",
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by
        FROM payroll_runs WHERE id = $1"#,
        run_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(run)
}
