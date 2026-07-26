//! Data access for the `payroll_runs` table.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::payroll::{PayrollRun, RunStatusRow};

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
         period_start, period_end, pay_date, version, status, processed_by, processed_at, notes, created_by)
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8,
            (SELECT COALESCE(MAX(version), 0) + 1
             FROM payroll_runs
             WHERE company_id = $2 AND payroll_group_id = $3
               AND period_year = $4 AND period_month = $5),
            'processing', $9, NOW(), $10, $9
        )"#,
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

/// The recorded calculation provenance for a run, if it has one.
///
/// Returned separately rather than as a `PayrollRun` field: the run struct is
/// serialized into every audit row, and embedding the whole snapshot there would
/// bloat the trail with a copy per transition.
pub async fn get_calculation_snapshot(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<serde_json::Value>> {
    let snapshot = sqlx::query_scalar!(
        r#"SELECT calculation_snapshot FROM payroll_runs WHERE id = $1 AND company_id = $2"#,
        run_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?
    .flatten();
    Ok(snapshot)
}

/// Record the statutory rule sets and overtime configuration a run was computed
/// from, so its figures stay reproducible after those inputs change.
pub async fn set_calculation_snapshot(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    snapshot: serde_json::Value,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE payroll_runs SET calculation_snapshot = $2 WHERE id = $1"#,
        run_id,
        snapshot,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Whether a run with this id exists in the company.
pub async fn exists(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    company_id: Uuid,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(
            SELECT 1 FROM payroll_runs WHERE id = $1 AND company_id = $2
        ) AS "exists!""#,
        run_id,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(exists)
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

pub async fn get_for_company(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    company_id: Uuid,
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
        FROM payroll_runs
        WHERE id = $1 AND company_id = $2"#,
        run_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(run)
}

/// `processed` → `pending_approval`. Returns `None` if the run isn't in `processed`.
pub async fn set_pending_approval(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    company_id: Uuid,
    updated_by: Uuid,
) -> AppResult<Option<PayrollRun>> {
    let run = sqlx::query_as!(
        PayrollRun,
        r#"UPDATE payroll_runs SET
            status = 'pending_approval', updated_by = $3, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'processed'
        RETURNING id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text AS "status!",
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by"#,
        run_id,
        company_id,
        updated_by,
    )
    .fetch_optional(executor)
    .await?;
    Ok(run)
}

/// `pending_approval` → `approved`. Returns `None` if the run isn't in `pending_approval`.
pub async fn set_approved(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    company_id: Uuid,
    approved_by: Uuid,
) -> AppResult<Option<PayrollRun>> {
    let run = sqlx::query_as!(
        PayrollRun,
        r#"UPDATE payroll_runs SET
            status = 'approved', approved_by = $3, approved_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending_approval'
        RETURNING id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text AS "status!",
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by"#,
        run_id,
        company_id,
        approved_by,
    )
    .fetch_optional(executor)
    .await?;
    Ok(run)
}

/// `pending_approval` → `processed` (returned for changes).
pub async fn set_returned(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    company_id: Uuid,
    updated_by: Uuid,
) -> AppResult<Option<PayrollRun>> {
    let run = sqlx::query_as!(
        PayrollRun,
        r#"UPDATE payroll_runs SET
            status = 'processed', updated_by = $3, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending_approval'
        RETURNING id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text AS "status!",
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by"#,
        run_id,
        company_id,
        updated_by,
    )
    .fetch_optional(executor)
    .await?;
    Ok(run)
}

/// `approved` → `paid` (locked).
pub async fn set_paid(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    company_id: Uuid,
    locked_by: Uuid,
) -> AppResult<Option<PayrollRun>> {
    let run = sqlx::query_as!(
        PayrollRun,
        r#"UPDATE payroll_runs SET
            status = 'paid', locked_by = $3, locked_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'approved'
        RETURNING id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text AS "status!",
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by"#,
        run_id,
        company_id,
        locked_by,
    )
    .fetch_optional(executor)
    .await?;
    Ok(run)
}

/// Recent runs for a company (newest period first, capped at 50).
pub async fn list_for_company(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<PayrollRun>> {
    let runs = sqlx::query_as!(
        PayrollRun,
        r#"SELECT id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text AS "status!",
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by
        FROM payroll_runs
        WHERE company_id = $1
        ORDER BY period_year DESC, period_month DESC, created_at DESC
        LIMIT 50"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(runs)
}

/// Lock a run row and return its status + period (for the PCB-edit transaction).
pub async fn get_status_locked(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<RunStatusRow>> {
    let row = sqlx::query_as!(
        RunStatusRow,
        r#"SELECT status::text AS "status!", period_year, period_month
        FROM payroll_runs
        WHERE id = $1 AND company_id = $2
        FOR UPDATE"#,
        run_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

/// Deletes a run only while it is still deletable, returning false if it is not.
///
/// The status/lock predicate is repeated here on purpose: the caller's check runs
/// outside the transaction, so a concurrent submit or approve between that read
/// and this statement would otherwise delete a locked run. Returning false lets
/// the caller roll the whole transaction back.
pub async fn delete_if_unlocked(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    company_id: Uuid,
) -> AppResult<bool> {
    let result = sqlx::query!(
        r#"DELETE FROM payroll_runs
        WHERE id = $1 AND company_id = $2
          AND locked_at IS NULL
          AND status::text NOT IN ('processing', 'pending_approval', 'approved', 'paid')"#,
        run_id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Adjust a run's PCB/net totals by `delta` after a single item's PCB is edited.
pub async fn bump_pcb_totals(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    company_id: Uuid,
    delta: i64,
    updated_by: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE payroll_runs
        SET total_pcb = total_pcb + $3,
            total_net = total_net - $3,
            updated_at = NOW(),
            updated_by = $4
        WHERE id = $1 AND company_id = $2"#,
        run_id,
        company_id,
        delta,
        updated_by,
    )
    .execute(executor)
    .await?;
    Ok(())
}
