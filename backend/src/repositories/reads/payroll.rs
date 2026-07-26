//! Bulk-prefetch read model for a payroll run: per-employee aggregations and joins
//! gathered up-front so the engine can compute each employee's payslip from in-memory
//! maps. Executor-generic so the engine calls them inside its transaction.
//!
//! NOTE: query indentation matches the byte-exact SQL in the offline `.sqlx` cache.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::payroll::{
    EmployeeBonusCommission, EmployeeCategoryTotal, EmployeeHours, EmployeeOtTypeHours,
    EmployeeTotal, OrphanedEntryEmployee, PayrollEntryWithEmployee, PayrollItemSummary, PayrollYtd,
    PayslipSourceLine,
};

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

/// Recurring allowances/deductions per employee, one row per configured line.
///
/// `recurring_allowance_totals` gives the engine the figure it needs for gross;
/// this gives it the lines it needs for the payslip breakdown, which used to be
/// discarded — `payroll_item_details` was never written, so a payslip could show
/// `total_allowances` without saying what the allowances were. Both reads apply
/// the same filters, so the lines always sum to the total.
pub async fn recurring_allowance_lines(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    effective_date: NaiveDate,
) -> AppResult<Vec<PayslipSourceLine>> {
    let rows = sqlx::query_as!(
        PayslipSourceLine,
        r#"SELECT employee_id, category, name AS "description!", amount,
                  COALESCE(is_taxable, TRUE) AS "is_taxable!"
           FROM employee_allowances
           WHERE employee_id = ANY($1) AND is_active = TRUE AND is_recurring = TRUE
             AND effective_from <= $2 AND (effective_to IS NULL OR effective_to >= $2)
           ORDER BY employee_id, category, name"#,
        employee_ids,
        effective_date,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Staged payroll entries per employee, one row per entry.
///
/// Filters match `entry_category_totals` exactly (including the `overtime` /
/// `claim_reimbursement` exclusion), so these lines reconcile to the totals the
/// engine puts into gross.
pub async fn entry_lines(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    year: i32,
    month: i32,
) -> AppResult<Vec<PayslipSourceLine>> {
    let rows = sqlx::query_as!(
        PayslipSourceLine,
        r#"SELECT employee_id, category, description AS "description!", amount,
                  COALESCE(is_taxable, TRUE) AS "is_taxable!"
           FROM payroll_entries
           WHERE employee_id = ANY($1) AND period_year = $2 AND period_month = $3
             AND is_processed = FALSE
             AND item_type NOT IN ('overtime', 'claim_reimbursement')
           ORDER BY employee_id, category, created_at"#,
        employee_ids,
        year,
        month,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Staged payroll entries per employee, summed by category.
///
/// Excludes the item types the engine recomputes from their own authoritative
/// source: `overtime` comes from `approved_ot_totals`/`attendance_ot_hours` and
/// `claim_reimbursement` from `approved_claim_totals`. Approving an OT
/// application or a claim also stages a `payroll_entries` row, so counting
/// those rows here would add the same money a second time — and, for claims,
/// would pull a reimbursement into gross where it becomes EPF/SOCSO/EIS/PCB
/// liable despite not being wages.
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
             AND item_type NOT IN ('overtime', 'claim_reimbursement')
           GROUP BY employee_id, category"#,
        employee_ids,
        year,
        month,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Staged bonus and commission entries per employee.
///
/// These already reach gross through `entry_category_totals`; this read exists so
/// the amounts can also be stored in their own `payroll_items` columns. Those
/// columns defaulted to 0, so the payslip printed "Bonus 0" while the figure sat
/// inside TOTAL EARNINGS, and the statutory EA form's income lines — which sum
/// these columns — did not add up to the reported YTD gross.
pub async fn bonus_commission_totals(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    year: i32,
    month: i32,
) -> AppResult<Vec<EmployeeBonusCommission>> {
    let rows = sqlx::query_as!(
        EmployeeBonusCommission,
        r#"SELECT employee_id,
                  COALESCE(SUM(amount) FILTER (WHERE item_type = 'bonus'), 0)::BIGINT AS "bonus!",
                  COALESCE(SUM(amount) FILTER (WHERE item_type = 'commission'), 0)::BIGINT AS "commission!"
           FROM payroll_entries
           WHERE employee_id = ANY($1) AND period_year = $2 AND period_month = $3
             AND is_processed = FALSE
             AND category = 'earning'
             AND item_type IN ('bonus', 'commission')
           GROUP BY employee_id"#,
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
               AND (ar.check_in_at AT TIME ZONE 'Asia/Kuala_Lumpur')::date = oa.ot_date
               AND oa.status = 'approved'
           WHERE ar.employee_id = ANY($1)
             -- Bucket by Malaysian local date. The old window compared the UTC
             -- instant against ::date, so a 00:00-08:00 MYT check-in on the 1st
             -- was paid in the previous month; and its upper bound was closed at
             -- period_end + 1 day, so a check-in at exactly that midnight fell in
             -- two consecutive runs.
             AND (ar.check_in_at AT TIME ZONE 'Asia/Kuala_Lumpur')::date BETWEEN $2 AND $3
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

/// Employees holding unprocessed entries for the period that this run will not pay.
///
/// A staged allowance or deduction is only picked up if its employee is selected
/// by `employees::list_for_payroll_run`, so an entry staged against someone in a
/// different payroll group — or someone resigned before the period, or with no
/// group at all — is silently left behind. The predicate below is the negation of
/// that selection, so the preview can say so before the run is committed.
pub async fn staged_entries_outside_run(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    payroll_group_id: Uuid,
    year: i32,
    month: i32,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> AppResult<Vec<OrphanedEntryEmployee>> {
    let rows = sqlx::query_as!(
        OrphanedEntryEmployee,
        r#"SELECT e.id AS "employee_id!", e.employee_number AS "employee_number!",
                  e.full_name AS "employee_name!",
                  COUNT(*)::BIGINT AS "entry_count!",
                  COALESCE(SUM(pe.amount), 0)::BIGINT AS "total_amount!"
           FROM payroll_entries pe
           JOIN employees e ON e.id = pe.employee_id
           WHERE pe.company_id = $1
             AND pe.period_year = $2 AND pe.period_month = $3
             AND pe.is_processed = FALSE
             AND pe.item_type NOT IN ('overtime', 'claim_reimbursement')
             -- COALESCE, not a bare NOT: an employee with no payroll group
             -- compares NULL here, and a NULL would drop them from the warning
             -- when they are exactly the case worth warning about.
             AND NOT COALESCE(
                   e.payroll_group_id = $4
                   AND e.is_active = TRUE AND e.deleted_at IS NULL
                   AND e.date_joined <= $6
                   AND (e.date_resigned IS NULL OR e.date_resigned >= $5)
                 , FALSE)
           GROUP BY e.id, e.employee_number, e.full_name
           ORDER BY e.employee_number"#,
        company_id,
        year,
        month,
        payroll_group_id,
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

/// Whether any employee paid by `run_id` already appears in a later committed run.
///
/// Deleting a run that a later run's stored `ytd_*` and PCB annualisation were
/// computed from would leave those figures describing a run that no longer
/// exists. `payroll_ytd` sums exactly these statuses, so the guard matches what
/// the YTD chain actually depends on — the same rule `employee_has_later_run`
/// applies to PCB edits, lifted to the whole run.
pub async fn run_has_later_committed_run(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    run_id: Uuid,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(
            SELECT 1
            FROM payroll_items pi
            JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
            WHERE pr.company_id = $1
              AND pr.id <> $2
              AND pr.status::text IN ('processed', 'pending_approval', 'approved', 'paid')
              AND pi.employee_id IN (
                  SELECT employee_id FROM payroll_items WHERE payroll_run_id = $2
              )
              AND (pr.period_year, pr.period_month) > (
                  SELECT period_year, period_month FROM payroll_runs WHERE id = $2
              )
        ) AS "exists!""#,
        company_id,
        run_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// Whether a later committed run already exists for an employee — blocks PCB edits.
pub async fn employee_has_later_run(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    company_id: Uuid,
    period_year: i32,
    period_month: i32,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(
            SELECT 1
            FROM payroll_items pi
            JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
            WHERE pi.employee_id = $1
              AND pr.company_id = $2
              AND pr.status::text IN ('processed', 'pending_approval', 'approved', 'paid')
              AND (pr.period_year > $3 OR (pr.period_year = $3 AND pr.period_month > $4))
        ) AS "exists!""#,
        employee_id,
        company_id,
        period_year,
        period_month,
    )
    .fetch_one(executor)
    .await?;
    Ok(exists)
}
