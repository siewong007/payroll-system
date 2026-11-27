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
    EmployeeHours, EmployeeUnratedOvertime, LaterCommittedRun, OrphanedEntryEmployee, PayableClaim,
    PayableOvertime, PaymentFileRow, PayrollEntryWithEmployee, PayrollItemSummary, PayrollYtd,
    PayslipSourceLine, RunJournalItemTotals, StagedEntryRow,
};
use crate::models::report::DepartmentPayrollRow;

/// Recurring allowances/deductions per employee, one row per configured line.
///
/// Selection is an interval **overlap** with the period, not a point test at the
/// period end. The old predicate asked whether a single instant — the last day of
/// the month — fell inside the allowance's window, so a leaver whose allowance
/// was correctly ended on the 15th failed `effective_to >= period_end` and was
/// paid nothing while their basic was prorated to 15/31, and an allowance granted
/// on the 25th passed `effective_from <= period_end` and was paid in full. Both
/// errors flow into gross, so EPF/SOCSO/EIS/PCB moved with them.
///
/// Rows rather than a `SUM`: the engine prorates each line against its own
/// window, so the per-line amounts and the total have to be the same arithmetic.
/// The totals used to be a second query carrying a duplicate copy of these
/// filters, with a comment promising the two agreed.
pub async fn recurring_allowance_lines(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> AppResult<Vec<PayslipSourceLine>> {
    let rows = sqlx::query_as!(
        PayslipSourceLine,
        r#"SELECT NULL::uuid AS "id?", employee_id, category,
                  -- `employee_allowances` has no item_type of its own; the two
                  -- names below are what the payslip breakdown has always
                  -- labelled these lines.
                  CASE WHEN category = 'earning' THEN 'allowance'::text
                       ELSE 'other_deduction'::text END AS "item_type!",
                  name AS "description!", amount,
                  COALESCE(is_taxable, TRUE) AS "is_taxable!",
                  effective_from AS "effective_from?", effective_to AS "effective_to?"
           FROM employee_allowances
           WHERE employee_id = ANY($1) AND is_active = TRUE AND is_recurring = TRUE
             AND effective_from <= $3
             AND (effective_to IS NULL OR effective_to >= $2)
           ORDER BY employee_id, category, name"#,
        employee_ids,
        period_start,
        period_end,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Load all unprocessed staged payroll entries for the run's employees in a single query.
///
/// Matches the `overtime` / `claim_reimbursement` exclusions so the returned rows
/// feed `entry_lines`, `entry_category_totals`, `unpaid_leave_totals`,
/// `monthly_allowance_totals`, `bonus_commission_totals`, and `staged_entry_ids`
/// from one consistent snapshot.
pub async fn staged_entries_for_run(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    year: i32,
    month: i32,
) -> AppResult<Vec<StagedEntryRow>> {
    if employee_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as!(
        StagedEntryRow,
        r#"SELECT id, employee_id, category, item_type, description, amount,
                  quantity, COALESCE(is_taxable, TRUE) AS "is_taxable!"
           FROM payroll_entries
           WHERE employee_id = ANY($1)
             AND (
                 (period_year = $2 AND period_month = $3)
                 OR (item_type = 'unpaid_leave' AND (period_year < $2 OR (period_year = $2 AND period_month <= $3)))
             )
             AND is_processed = FALSE
             AND item_type <> 'claim_reimbursement'
           ORDER BY employee_id, category, created_at, id"#,
        employee_ids,
        year,
        month,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Lock and load all unprocessed staged payroll entries inside the write transaction.
/// Locks in deterministic `id` order before sorting into breakdown display order.
pub async fn lock_and_load_staged_entries_for_run(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    year: i32,
    month: i32,
) -> AppResult<Vec<StagedEntryRow>> {
    if employee_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as!(
        StagedEntryRow,
        r#"WITH locked AS (
               SELECT id, employee_id, category, item_type, description,
                      amount, quantity, COALESCE(is_taxable, TRUE) AS is_taxable, created_at
               FROM payroll_entries
               WHERE employee_id = ANY($1)
                 AND (
                     (period_year = $2 AND period_month = $3)
                     OR (item_type = 'unpaid_leave' AND (period_year < $2 OR (period_year = $2 AND period_month <= $3)))
                 )
                 AND is_processed = FALSE
               ORDER BY id
               FOR UPDATE
           )
           SELECT l.id, l.employee_id, l.category, l.item_type, l.description,
                  l.amount, l.quantity, l.is_taxable AS "is_taxable!"
           FROM locked l
           WHERE l.item_type <> 'claim_reimbursement'
           ORDER BY l.employee_id, l.category, l.created_at, l.id"#,
        employee_ids,
        year,
        month,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Attendance-based overtime hours per employee, excluding days already covered by an
/// approved overtime application, absent records, or records above the company ceiling.
pub async fn attendance_ot_hours(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_ids: &[Uuid],
    period_start: NaiveDate,
    period_end: NaiveDate,
    tz: &str,
    max_overtime_hours: rust_decimal::Decimal,
) -> AppResult<Vec<EmployeeHours>> {
    if employee_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as!(
        EmployeeHours,
        r#"WITH company_holidays AS (
               SELECT date, is_recurring,
                      EXTRACT(MONTH FROM date)::int AS h_month,
                      EXTRACT(DAY FROM date)::int AS h_day
               FROM holidays
               WHERE company_id = $2
           ),
           company_working_days AS (
               SELECT day_of_week, is_working_day
               FROM working_day_config
               WHERE company_id = $2
           )
           SELECT employee_id, day_type AS "day_type!", COALESCE(SUM(day_hours), 0::NUMERIC) AS "hours!"
           FROM (
               SELECT employee_id,
                      local_date,
                      day_type,
                      LEAST(COALESCE(SUM(overtime_hours), 0::NUMERIC), $6) AS day_hours
               FROM (
                   SELECT ar.employee_id,
                          ld.local_date,
                          ar.overtime_hours,
                          CASE
                            WHEN EXISTS (
                              SELECT 1 FROM company_holidays h
                              WHERE h.date = ld.local_date
                                 OR (h.is_recurring
                                     AND h.date <= ld.local_date
                                     AND h.h_month = EXTRACT(MONTH FROM ld.local_date)::int
                                     AND h.h_day = EXTRACT(DAY FROM ld.local_date)::int)
                            ) THEN 'public_holiday'
                            WHEN COALESCE(
                              cwd.is_working_day,
                              EXTRACT(DOW FROM ld.local_date)::int NOT IN (0, 6)
                            ) = FALSE THEN 'rest_day'
                            ELSE 'normal'
                          END AS day_type
                   FROM attendance_records ar
                   CROSS JOIN LATERAL (
                       SELECT (ar.check_in_at AT TIME ZONE $5)::date AS local_date
                   ) ld
                   LEFT JOIN company_working_days cwd
                       ON cwd.day_of_week = EXTRACT(DOW FROM ld.local_date)::int
                   LEFT JOIN overtime_applications oa
                       ON ar.employee_id = oa.employee_id
                       AND oa.ot_date = ld.local_date
                       AND oa.ot_date >= $3 AND oa.ot_date <= $4
                       AND oa.status IN ('approved', 'processed')
                   WHERE ar.company_id = $2
                     AND ar.employee_id = ANY($1)
                     AND ar.check_in_at >= ($3::date)::timestamp AT TIME ZONE $5
                     AND ar.check_in_at < ($4::date + 1)::timestamp AT TIME ZONE $5
                     AND ar.status <> 'absent'
                     AND ar.is_outside_geofence IS NOT TRUE
                     AND ar.is_offsite_network IS NOT TRUE
                     AND ar.overtime_hours IS NOT NULL
                     AND ar.overtime_hours > 0
                     AND ar.overtime_hours <= $6
                     AND oa.id IS NULL
               ) buckets
               GROUP BY employee_id, local_date, day_type
           ) daily_capped
           GROUP BY employee_id, day_type"#,
        employee_ids,
        company_id,
        period_start,
        period_end,
        tz,
        max_overtime_hours,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Closed attendance records in the period whose overtime was not rated —
/// either left NULL by the per-day ceiling, or written before that ceiling
/// existed and still above it. These are the forgotten check-outs.
///
/// The hours are already excluded from pay, so this feeds a preview warning
/// rather than a blocker: the run is safe to commit, but an HR correction is
/// owed to anyone who genuinely worked them.
pub async fn unrated_overtime_records(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    period_start: NaiveDate,
    period_end: NaiveDate,
    tz: &str,
    max_overtime_hours: rust_decimal::Decimal,
) -> AppResult<Vec<EmployeeUnratedOvertime>> {
    let rows = sqlx::query_as!(
        EmployeeUnratedOvertime,
        r#"SELECT e.id AS "employee_id!", e.employee_number AS "employee_number!",
                  e.full_name AS "employee_name!",
                  COUNT(*)::BIGINT AS "record_count!",
                  MAX(ar.hours_worked) AS "max_hours_worked?"
           FROM attendance_records ar
           JOIN employees e ON e.id = ar.employee_id
           WHERE ar.employee_id = ANY($1)
             AND ar.check_out_at IS NOT NULL
             AND ar.status <> 'absent'
             AND (ar.overtime_hours IS NULL OR ar.overtime_hours > $4)
             -- Same sargable local-date bounds as attendance_ot_hours, so the
             -- warning covers exactly the records that run would have paid.
             AND ar.check_in_at >= ($2::date)::timestamp AT TIME ZONE $5
             AND ar.check_in_at < ($3::date + 1)::timestamp AT TIME ZONE $5
           GROUP BY e.id, e.employee_number, e.full_name
           ORDER BY e.employee_number"#,
        employee_ids,
        period_start,
        period_end,
        max_overtime_hours,
        tz,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Approved overtime hours per employee, grouped by overtime type.
/// Individual approved, not-yet-paid overtime applications a run will pay.
///
/// Carry-forward (`ot_date <= period_end` AND `payroll_run_id IS NULL`) matching
/// claims, so exact IDs summed in `compute_payslip` are atomically retired by
/// `persist_all_payslips`.
pub async fn payable_overtime_applications(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    company_id: Uuid,
    period_end: NaiveDate,
) -> AppResult<Vec<PayableOvertime>> {
    if employee_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as!(
        PayableOvertime,
        r#"SELECT id, employee_id, ot_type, hours, ot_date
           FROM overtime_applications
           WHERE employee_id = ANY($1)
             AND company_id = $2
             AND status = 'approved'
             AND payroll_run_id IS NULL
             AND ot_date <= $3
           ORDER BY employee_id, ot_date, id"#,
        employee_ids,
        company_id,
        period_end,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// The approved, not-yet-paid claims a run will reimburse.
///
/// Carry-forward, not period-bounded. The old predicate required
/// `expense_date BETWEEN period_start AND period_end`, so a claim approved after
/// its own expense month had closed fell into a hole: that month's run refuses
/// to be re-created, and the next month's window excludes the expense date.
/// Nothing paid it and its status stayed 'approved' forever. Every approved and
/// unpaid claim incurred on or before the period end is swept here instead; a
/// future-dated expense still waits.
///
/// Rows rather than a `SUM` because the engine marks exactly the ids it summed.
/// Summing and then re-running the same predicate to mark them paid meant a
/// claim approved between the two was marked paid without being paid.
pub async fn payable_claims(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    company_id: Uuid,
    period_end: NaiveDate,
) -> AppResult<Vec<PayableClaim>> {
    let rows = sqlx::query_as!(
        PayableClaim,
        r#"SELECT id, employee_id, title, amount, expense_date
           FROM claims
           WHERE employee_id = ANY($1)
             AND company_id = $2
             AND status = 'approved'
             AND payroll_run_id IS NULL
             AND expense_date <= $3
           ORDER BY employee_id, expense_date, id"#,
        employee_ids,
        company_id,
        period_end,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Employees holding payable claims that this run will not reimburse.
///
/// The mirror of `staged_entries_outside_run` for the other money source the
/// engine reads from its own authoritative table. A claim belonging to someone
/// outside the selected payroll group is swept by no run at all, and without
/// this the operator has nowhere to notice — the claim simply stays 'approved'.
pub async fn approved_claims_outside_run(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    payroll_group_id: Uuid,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> AppResult<Vec<OrphanedEntryEmployee>> {
    let rows = sqlx::query_as!(
        OrphanedEntryEmployee,
        r#"SELECT e.id AS "employee_id!", e.employee_number AS "employee_number!",
                  e.full_name AS "employee_name!",
                  COUNT(*)::BIGINT AS "entry_count!",
                  COALESCE(SUM(c.amount), 0)::BIGINT AS "total_amount!"
           FROM claims c
           JOIN employees e ON e.id = c.employee_id
           WHERE c.company_id = $1
             AND c.status = 'approved'
             AND c.payroll_run_id IS NULL
             AND c.expense_date <= $4
             -- Same negation as staged_entries_outside_run, and COALESCE for the
             -- same reason: an employee with no payroll group compares NULL, and
             -- a NULL would drop exactly the case worth warning about.
             AND NOT COALESCE(
                   e.payroll_group_id = $2
                   AND e.deleted_at IS NULL
                   AND (e.is_active = TRUE OR e.date_resigned IS NOT NULL)
                   AND e.date_joined <= $4
                   AND (e.date_resigned IS NULL OR e.date_resigned >= $3)
                 , FALSE)
           GROUP BY e.id, e.employee_number, e.full_name
           ORDER BY e.employee_number"#,
        company_id,
        payroll_group_id,
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
             AND (
                 (pe.period_year = $2 AND pe.period_month = $3)
                 OR (pe.item_type = 'unpaid_leave' AND (pe.period_year < $2 OR (pe.period_year = $2 AND pe.period_month <= $3)))
             )
             AND pe.is_processed = FALSE
             AND pe.item_type NOT IN ('overtime', 'claim_reimbursement')
             -- COALESCE, not a bare NOT: an employee with no payroll group
             -- compares NULL here, and a NULL would drop them from the warning
             -- when they are exactly the case worth warning about.
             AND NOT COALESCE(
                   e.payroll_group_id = $4
                   AND e.deleted_at IS NULL
                   -- Mirrors employees::list_for_payroll_run: selection is the
                   -- employment window, and `is_active` only excludes someone
                   -- with no resignation date to explain it. If the two drift, a
                   -- leaver this run now pays is simultaneously reported as one
                   -- it will not.
                   AND (e.is_active = TRUE OR e.date_resigned IS NOT NULL)
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
    company_id: Uuid,
    employee_ids: &[Uuid],
    year: i32,
    month: i32,
) -> AppResult<Vec<PayrollYtd>> {
    if employee_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as!(
        PayrollYtd,
        r#"SELECT
            pi.employee_id,
            COALESCE(SUM(pi.gross_salary), 0)::BIGINT AS "gross!",
            COALESCE(SUM(COALESCE(pi.taxable_gross, GREATEST(pi.gross_salary - pi.unpaid_leave_deduction, 0))), 0)::BIGINT AS "taxable_gross!",
            COALESCE(SUM(pi.pcb_amount), 0)::BIGINT AS "pcb!",
            COALESCE(SUM(pi.epf_employee), 0)::BIGINT AS "epf!",
            COALESCE(SUM(pi.socso_employee), 0)::BIGINT AS "socso!",
            COALESCE(SUM(pi.eis_employee), 0)::BIGINT AS "eis!",
            COALESCE(SUM(pi.zakat_amount), 0)::BIGINT AS "zakat!",
            COALESCE(SUM(pi.net_salary), 0)::BIGINT AS "net!"
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        WHERE pr.company_id = $1
          AND pi.employee_id = ANY($2)
          AND pr.period_year = $3
          AND pr.period_month < $4
          AND pr.status IN ('processed', 'pending_approval', 'approved', 'paid')
        GROUP BY pi.employee_id"#,
        company_id,
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
            pe.is_taxable, pe.is_processed, pe.payroll_run_id, pe.overtime_application_id,
            pe.leave_request_id, pe.created_at, pe.updated_at,
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
        ORDER BY pe.period_year DESC, pe.period_month DESC, e.employee_number, pe.created_at DESC
        LIMIT 2000"#,
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

/// Whether any employee paid by `run_id` already appears in a later committed run
/// within the SAME tax year (since YTD resets each Jan 1, runs in later years do not
/// depend on prior-year runs).
pub async fn run_has_later_committed_run(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    run_id: Uuid,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar!(
        r#"WITH target AS (
               SELECT period_year, period_month FROM payroll_runs WHERE id = $2
           )
           SELECT EXISTS(
               SELECT 1
               FROM payroll_items pi
               JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
               CROSS JOIN target t
               WHERE pr.company_id = $1
                 AND pr.id <> $2
                 AND pr.status IN ('processed', 'pending_approval', 'approved', 'paid')
                 AND pr.period_year = t.period_year
                 AND pr.period_month > t.period_month
                 AND pi.employee_id IN (
                     SELECT employee_id FROM payroll_items WHERE payroll_run_id = $2
                 )
           ) AS "exists!""#,
        company_id,
        run_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// Employees in a proposed run who already appear in another non-cancelled run
/// for the SAME period (e.g. transferred between payroll groups mid-month).
pub async fn employees_already_paid_in_period(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    company_id: Uuid,
    period_year: i32,
    period_month: i32,
    exclude_run_id: Option<Uuid>,
) -> AppResult<Vec<LaterCommittedRun>> {
    if employee_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as!(
        LaterCommittedRun,
        r#"SELECT e.id AS "employee_id!", e.employee_number AS "employee_number!",
                  e.full_name AS "employee_name!",
                  (pr.period_year * 100 + pr.period_month)::INT AS "earliest_later_period!"
           FROM payroll_items pi
           JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
           JOIN employees e ON e.id = pi.employee_id
           WHERE pi.employee_id = ANY($1)
             AND pr.company_id = $2
             AND pr.period_year = $3
             AND pr.period_month = $4
             AND pr.status <> 'cancelled'
             AND ($5::uuid IS NULL OR pr.id <> $5)
           GROUP BY e.id, e.employee_number, e.full_name, pr.period_year, pr.period_month
           ORDER BY e.employee_number"#,
        employee_ids,
        company_id,
        period_year,
        period_month,
        exclude_run_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Employees in a proposed run who already appear in a LATER committed run in the same tax year.
pub async fn employees_with_later_committed_run(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    company_id: Uuid,
    period_year: i32,
    period_month: i32,
) -> AppResult<Vec<LaterCommittedRun>> {
    if employee_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as!(
        LaterCommittedRun,
        r#"SELECT e.id AS "employee_id!", e.employee_number AS "employee_number!",
                  e.full_name AS "employee_name!",
                  MIN(pr.period_year * 100 + pr.period_month)::INT AS "earliest_later_period!"
           FROM payroll_items pi
           JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
           JOIN employees e ON e.id = pi.employee_id
           WHERE pi.employee_id = ANY($1)
             AND pr.company_id = $2
             AND pr.status IN ('processed', 'pending_approval', 'approved', 'paid')
             AND pr.period_year = $3
             AND pr.period_month > $4
           GROUP BY e.id, e.employee_number, e.full_name
           ORDER BY e.employee_number"#,
        employee_ids,
        company_id,
        period_year,
        period_month,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Months of the given tax year for which this group already has a committed run.
pub async fn committed_months_for_group(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    payroll_group_id: Uuid,
    period_year: i32,
) -> AppResult<Vec<i32>> {
    let months = sqlx::query_scalar!(
        r#"SELECT period_month AS "period_month!"
           FROM payroll_runs
           WHERE company_id = $1
             AND payroll_group_id = $2
             AND period_year = $3
             AND status IN ('processed', 'pending_approval', 'approved', 'paid')
           GROUP BY period_month
           ORDER BY period_month"#,
        company_id,
        payroll_group_id,
        period_year,
    )
    .fetch_all(executor)
    .await?;
    Ok(months)
}

/// Whether a later committed run in the same tax year already exists for an employee — blocks PCB edits.
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
              AND pr.status IN ('processed', 'pending_approval', 'approved', 'paid')
              AND pr.period_year = $3
              AND pr.period_month > $4
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

/// Aggregated item-level sums for a run's journal preview in a single pass.
pub async fn run_journal_item_totals(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
) -> AppResult<RunJournalItemTotals> {
    let row = sqlx::query_as!(
        RunJournalItemTotals,
        r#"SELECT
            COALESCE(SUM(total_claims), 0)::bigint AS "total_claims!",
            COALESCE(SUM(ptptn_amount), 0)::bigint AS "total_ptptn!",
            COALESCE(SUM(tabung_haji_amount), 0)::bigint AS "total_tabung_haji!",
            COALESCE(SUM(total_loan_deductions + total_other_deductions), 0)::bigint AS "total_other_deductions!",
            COALESCE(SUM(unpaid_leave_deduction), 0)::bigint AS "total_unpaid_leave!"
        FROM payroll_items
        WHERE payroll_run_id = $1"#,
        run_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(row)
}

/// Per-employee net pay and bank details for a run's payment file.
///
/// Run-scoped rather than period-scoped: the file must reflect exactly what
/// this run committed to pay, including a corrected figure after a PCB edit.
/// Missing bank fields come back NULL — the service surfaces them as the
/// `missing_bank_account` rows the action queue already counts.
pub async fn payment_file_rows(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
) -> AppResult<Vec<PaymentFileRow>> {
    let rows = sqlx::query_as!(
        PaymentFileRow,
        r#"SELECT e.employee_number, e.full_name AS employee_name,
            pi.bank_name, pi.bank_account_number, pi.bank_account_type,
            pi.net_salary
        FROM payroll_items pi
        JOIN employees e ON pi.employee_id = e.id
        WHERE pi.payroll_run_id = $1
        ORDER BY e.employee_number"#,
        run_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Department totals for one run — the expense-side allocation of the journal
/// preview. `department` is NULL for employees with no department on record;
/// the service labels it "Unallocated" rather than dropping the wages.
pub async fn run_department_totals(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
) -> AppResult<Vec<DepartmentPayrollRow>> {
    let rows = sqlx::query_as!(
        DepartmentPayrollRow,
        r#"SELECT e.department,
            COUNT(DISTINCT pi.employee_id) AS "employee_count!",
            COALESCE(SUM(pi.gross_salary), 0)::bigint AS "total_gross!",
            COALESCE(SUM(pi.net_salary), 0)::bigint AS "total_net!",
            COALESCE(SUM(pi.employer_cost), 0)::bigint AS "total_employer_cost!"
        FROM payroll_items pi
        JOIN employees e ON pi.employee_id = e.id
        WHERE pi.payroll_run_id = $1
        GROUP BY e.department
        ORDER BY COALESCE(SUM(pi.gross_salary), 0) DESC"#,
        run_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}
