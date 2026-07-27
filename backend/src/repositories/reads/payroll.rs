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
    EmployeeTotal, EmployeeUnpaidLeave, EmployeeUnratedOvertime, LaterCommittedRun,
    OrphanedEntryEmployee, PayableClaim, PayrollEntryWithEmployee, PayrollItemSummary, PayrollYtd,
    PayslipSourceLine,
};

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
        r#"SELECT employee_id, category,
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
        r#"SELECT employee_id, category,
                  -- Carried so the breakdown can label an unpaid-leave deduction
                  -- as one instead of folding it into "other deduction".
                  item_type AS "item_type!",
                  description AS "description!", amount,
                  COALESCE(is_taxable, TRUE) AS "is_taxable!",
                  -- Staged entries are keyed by (period_year, period_month) and
                  -- carry no window of their own, so they are never prorated.
                  NULL::date AS "effective_from?", NULL::date AS "effective_to?"
           FROM payroll_entries
           WHERE employee_id = ANY($1) AND period_year = $2 AND period_month = $3
             AND is_processed = FALSE
             AND item_type NOT IN ('overtime', 'claim_reimbursement')
           -- `id` breaks ties on `created_at`: a restore writes a whole period's
           -- entries at one instant, and without it the breakdown's line order
           -- is nondeterministic between two reads of the same payslip.
           ORDER BY employee_id, category, created_at, id"#,
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
/// `claim_reimbursement` from `payable_claims`. Approving an OT application
/// stages a `payroll_entries` row, so counting those rows here would add the
/// same money a second time.
///
/// The `claim_reimbursement` half of the exclusion is now defence in depth:
/// claim approval no longer stages an entry and migration 1010 deleted the rows
/// it used to write. It stays because dropping it would make a reimbursement
/// EPF/SOCSO/EIS/PCB-liable the moment any future write path reintroduces the
/// item type — which is the exact failure the staging was retired to prevent.
///
/// `taxable` is the same sum narrowed to rows the admin marked taxable. The flag
/// was honoured by the rendered payslip line — which badges an earning
/// "Non-taxable" — and by nothing else, so an earning staged as exempt was still
/// inside the PCB base. Only the PCB base uses it; see `compute_payslip`.
pub async fn entry_category_totals(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    year: i32,
    month: i32,
) -> AppResult<Vec<EmployeeCategoryTotal>> {
    let rows = sqlx::query_as!(
        EmployeeCategoryTotal,
        r#"SELECT employee_id, category, SUM(amount)::BIGINT AS "total!",
                  COALESCE(SUM(amount) FILTER (WHERE COALESCE(is_taxable, TRUE)), 0)::BIGINT AS "taxable!"
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

/// Staged unpaid-leave deductions per employee: amount and working days.
///
/// Same shape and filters as `bonus_commission_totals`, so the two reconcile
/// against `entry_category_totals`. Both `payroll_items.unpaid_leave_deduction`
/// and `unpaid_leave_days` existed and were never written, so the payslip, the
/// portal, the PDF fallback and the backup archive all shipped a constant zero
/// while the money sat anonymously inside `total_other_deductions`.
///
/// `quantity` is NULL on any row staged before it started being written, so the
/// day count reads 0 for those rather than erroring.
pub async fn unpaid_leave_totals(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    year: i32,
    month: i32,
) -> AppResult<Vec<EmployeeUnpaidLeave>> {
    let rows = sqlx::query_as!(
        EmployeeUnpaidLeave,
        r#"SELECT employee_id,
                  COALESCE(SUM(amount), 0)::BIGINT AS "amount!",
                  COALESCE(SUM(quantity), 0)::NUMERIC AS "days!"
           FROM payroll_entries
           WHERE employee_id = ANY($1) AND period_year = $2 AND period_month = $3
             AND is_processed = FALSE
             AND category = 'deduction'
             AND item_type = 'unpaid_leave'
           GROUP BY employee_id"#,
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
    tz: &str,
) -> AppResult<Vec<EmployeeHours>> {
    let rows = sqlx::query_as!(
        EmployeeHours,
        r#"SELECT ar.employee_id, COALESCE(SUM(ar.overtime_hours), 0)::FLOAT AS "hours!"
           FROM attendance_records ar
           LEFT JOIN overtime_applications oa
               ON ar.employee_id = oa.employee_id
               AND (ar.check_in_at AT TIME ZONE $4)::date = oa.ot_date
               AND oa.status = 'approved'
           WHERE ar.employee_id = ANY($1)
             -- Bucket by the company's local date, as a half-open range on the
             -- raw timestamptz. Comparing the UTC instant against ::date paid a
             -- 00:00-08:00 local check-in on the 1st in the previous month, and
             -- a closed upper bound at period_end + 1 day put a check-in at
             -- exactly that midnight in two consecutive runs. Wrapping the
             -- column in AT TIME ZONE fixed both but cost the
             -- (company_id, check_in_at) index, and hardcoded MYT for tenants
             -- that are not on it.
             AND ar.check_in_at >= ($2::date)::timestamp AT TIME ZONE $4
             AND ar.check_in_at < ($3::date + 1)::timestamp AT TIME ZONE $4
             AND oa.id IS NULL
           -- COALESCE, not a bare SUM: a group whose overtime is entirely NULL
           -- (a forgotten check-out left unrated, or a correction that reopened
           -- the session) still produces a row, and SUM returns NULL for it.
           -- The non-null assertion on "hours!" turned that into a runtime error
           -- that failed the whole run.
           GROUP BY ar.employee_id"#,
        employee_ids,
        period_start,
        period_end,
        tz,
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
             AND pe.period_year = $2 AND pe.period_month = $3
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

/// Employees in a proposed run who already appear in a LATER committed run.
///
/// The batch, create-time mirror of `employee_has_later_run`. A run's
/// `payroll_items.ytd_*` are frozen when it commits and its PCB annualisation
/// already assumed every earlier month was in them, and nothing recomputes
/// either — so processing February *after* March silently corrupts March's
/// printed figures. A gap is legitimate (a group may simply not have been owed
/// February); inserting behind a committed run is not, which is why the guard is
/// on the later run's existence rather than on the gap.
///
/// The status filter is the one `payroll_ytd` sums, so the guard covers exactly
/// what the YTD chain depends on. Employee number, name and the offending period
/// come back with it so the refusal names who and when.
pub async fn employees_with_later_committed_run(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    company_id: Uuid,
    period_year: i32,
    period_month: i32,
) -> AppResult<Vec<LaterCommittedRun>> {
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
             AND pr.status::text IN ('processed', 'pending_approval', 'approved', 'paid')
             AND (pr.period_year > $3 OR (pr.period_year = $3 AND pr.period_month > $4))
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
///
/// Feeds the advisory "you skipped a month" diagnostic: a genuinely skipped
/// month makes the PCB annualisation's `remaining_months` over-count and
/// under-withhold, and no ordering rule can repair that after the fact — the
/// operator has to decide.
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
             AND status::text IN ('processed', 'pending_approval', 'approved', 'paid')
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
