//! Bulk-prefetch read model for a payroll run: per-employee aggregations and joins
//! gathered up-front so the engine can compute each employee's payslip from in-memory
//! maps. Executor-generic so the engine calls them inside its transaction.
//!
//! NOTE: query indentation matches the byte-exact SQL in the offline `.sqlx` cache.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::payroll::{PayrollEntryWithEmployee, PayrollItemSummary};

#[derive(Debug)]
pub struct EmployeeCategoryTotal {
    pub employee_id: Uuid,
    pub category: String,
    pub total: i64,
}

#[derive(Debug)]
pub struct EmployeeTotal {
    pub employee_id: Uuid,
    pub total: i64,
}

#[derive(Debug)]
pub struct EmployeeHours {
    pub employee_id: Uuid,
    pub hours: f64,
}

#[derive(Debug)]
pub struct EmployeeOtTypeHours {
    pub employee_id: Uuid,
    pub ot_type: String,
    pub hours: f64,
}

#[derive(Debug)]
pub struct PayrollYtd {
    pub employee_id: Uuid,
    pub gross: i64,
    pub pcb: i64,
    pub epf: i64,
    pub socso: i64,
    pub eis: i64,
    pub zakat: i64,
    pub net: i64,
}

/// Recurring allowances/deductions per employee, summed by category.
pub async fn recurring_allowance_totals(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    effective_date: NaiveDate,
) -> AppResult<Vec<EmployeeCategoryTotal>> {
    let rows = sqlx::query_as!(
        EmployeeCategoryTotal,
        r#"SELECT employee_id, category, SUM(amount)::BIGINT AS "total!"
           FROM employee_allowances
           WHERE employee_id = ANY($1) AND is_active = TRUE AND is_recurring = TRUE
             AND effective_from <= $2 AND (effective_to IS NULL OR effective_to >= $2)
           GROUP BY employee_id, category"#,
        employee_ids,
        effective_date,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Staged payroll entries per employee, summed by category.
pub async fn entry_category_totals(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    year: i32,
    month: i32,
) -> AppResult<Vec<EmployeeCategoryTotal>> {
    let rows = sqlx::query_as!(
        EmployeeCategoryTotal,
        r#"SELECT employee_id, category, SUM(amount)::BIGINT AS "total!"
           FROM payroll_entries
           WHERE employee_id = ANY($1) AND period_year = $2 AND period_month = $3
             AND is_processed = FALSE
           GROUP BY employee_id, category"#,
        employee_ids,
        year,
        month,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Staged monthly-allowance entries per employee.
pub async fn monthly_allowance_totals(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    year: i32,
    month: i32,
) -> AppResult<Vec<EmployeeTotal>> {
    let rows = sqlx::query_as!(
        EmployeeTotal,
        r#"SELECT employee_id, SUM(amount)::BIGINT AS "total!"
           FROM payroll_entries
           WHERE employee_id = ANY($1) AND period_year = $2 AND period_month = $3
             AND is_processed = FALSE
             AND category = 'earning'
             AND item_type IN ('allowance', 'monthly_allowance')
           GROUP BY employee_id"#,
        employee_ids,
        year,
        month,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Attendance-based overtime hours per employee, excluding days already covered by an
/// approved overtime application.
pub async fn attendance_ot_hours(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> AppResult<Vec<EmployeeHours>> {
    let rows = sqlx::query_as!(
        EmployeeHours,
        r#"SELECT ar.employee_id, SUM(ar.overtime_hours)::FLOAT AS "hours!"
           FROM attendance_records ar
           LEFT JOIN overtime_applications oa
               ON ar.employee_id = oa.employee_id
               AND DATE(ar.check_in_at) = oa.ot_date
               AND oa.status = 'approved'
           WHERE ar.employee_id = ANY($1)
             AND ar.check_in_at >= $2::date AND ar.check_in_at <= $3::date + INTERVAL '1 day'
             AND oa.id IS NULL
           GROUP BY ar.employee_id"#,
        employee_ids,
        period_start,
        period_end,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Approved overtime hours per employee, grouped by overtime type.
pub async fn approved_ot_totals(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> AppResult<Vec<EmployeeOtTypeHours>> {
    let rows = sqlx::query_as!(
        EmployeeOtTypeHours,
        r#"SELECT employee_id, ot_type, SUM(hours)::FLOAT AS "hours!"
           FROM overtime_applications
           WHERE employee_id = ANY($1)
             AND ot_date >= $2 AND ot_date <= $3
             AND status = 'approved'
           GROUP BY employee_id, ot_type"#,
        employee_ids,
        period_start,
        period_end,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Approved claims per employee within the period.
pub async fn approved_claim_totals(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    company_id: Uuid,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> AppResult<Vec<EmployeeTotal>> {
    let rows = sqlx::query_as!(
        EmployeeTotal,
        r#"SELECT employee_id, SUM(amount)::BIGINT AS "total!"
           FROM claims
           WHERE employee_id = ANY($1)
             AND company_id = $2
             AND status = 'approved'
             AND expense_date >= $3 AND expense_date <= $4
           GROUP BY employee_id"#,
        employee_ids,
        company_id,
        period_start,
        period_end,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Year-to-date statutory figures per employee from prior committed runs this year.
pub async fn payroll_ytd(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    year: i32,
    month: i32,
) -> AppResult<Vec<PayrollYtd>> {
    let rows = sqlx::query_as!(
        PayrollYtd,
        r#"SELECT
            pi.employee_id,
            COALESCE(SUM(pi.gross_salary), 0)::BIGINT AS "gross!",
            COALESCE(SUM(pi.pcb_amount), 0)::BIGINT AS "pcb!",
            COALESCE(SUM(pi.epf_employee), 0)::BIGINT AS "epf!",
            COALESCE(SUM(pi.socso_employee), 0)::BIGINT AS "socso!",
            COALESCE(SUM(pi.eis_employee), 0)::BIGINT AS "eis!",
            COALESCE(SUM(pi.zakat_amount), 0)::BIGINT AS "zakat!",
            COALESCE(SUM(pi.net_salary), 0)::BIGINT AS "net!"
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        WHERE pi.employee_id = ANY($1) AND pr.period_year = $2 AND pr.period_month < $3
        AND pr.status::text IN ('processed', 'pending_approval', 'approved', 'paid')
        GROUP BY pi.employee_id"#,
        employee_ids,
        year,
        month,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Staged payroll entries (joined with employee name/number), with optional filters.
pub async fn entries_with_employee(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    period_year: Option<i32>,
    period_month: Option<i32>,
    employee_id: Option<Uuid>,
    item_type: Option<&str>,
    include_processed: bool,
) -> AppResult<Vec<PayrollEntryWithEmployee>> {
    let entries = sqlx::query_as!(
        PayrollEntryWithEmployee,
        r#"SELECT pe.id, pe.employee_id, pe.company_id, pe.period_year, pe.period_month,
            pe.category, pe.item_type, pe.description, pe.amount, pe.quantity, pe.rate,
            pe.is_taxable, pe.is_processed, pe.payroll_run_id, pe.created_at, pe.updated_at,
            pe.created_by, pe.updated_by,
            e.full_name AS "employee_name?", e.employee_number AS "employee_number?"
        FROM payroll_entries pe
        JOIN employees e ON pe.employee_id = e.id
        WHERE pe.company_id = $1
          AND ($2::int IS NULL OR pe.period_year = $2)
          AND ($3::int IS NULL OR pe.period_month = $3)
          AND ($4::uuid IS NULL OR pe.employee_id = $4)
          AND ($5::text IS NULL OR pe.item_type = $5)
          AND ($6::bool = TRUE OR pe.is_processed = FALSE)
        ORDER BY pe.period_year DESC, pe.period_month DESC, e.employee_number, pe.created_at DESC"#,
        company_id,
        period_year,
        period_month,
        employee_id,
        item_type,
        include_processed,
    )
    .fetch_all(executor)
    .await?;
    Ok(entries)
}

/// Per-employee payslip summaries for a run (joined with employee name/number).
pub async fn item_summaries_for_run(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
) -> AppResult<Vec<PayrollItemSummary>> {
    let items = sqlx::query!(
        r#"SELECT pi.employee_id, e.full_name, e.employee_number,
           pi.basic_salary, pi.total_allowances, pi.total_overtime, pi.total_claims,
           pi.gross_salary, pi.total_deductions, pi.net_salary,
           pi.epf_employee, pi.socso_employee, pi.eis_employee, pi.pcb_amount
        FROM payroll_items pi
        JOIN employees e ON pi.employee_id = e.id
        WHERE pi.payroll_run_id = $1
        ORDER BY e.employee_number"#,
        run_id,
    )
    .fetch_all(executor)
    .await?;

    Ok(items
        .into_iter()
        .map(|row| PayrollItemSummary {
            employee_id: row.employee_id,
            employee_name: row.full_name,
            employee_number: row.employee_number,
            basic_salary: row.basic_salary,
            total_allowances: row.total_allowances,
            total_overtime: row.total_overtime,
            total_claims: row.total_claims,
            gross_salary: row.gross_salary,
            total_deductions: row.total_deductions,
            net_salary: row.net_salary,
            epf_employee: row.epf_employee,
            socso_employee: row.socso_employee,
            eis_employee: row.eis_employee,
            pcb_amount: row.pcb_amount,
        })
        .collect())
}
