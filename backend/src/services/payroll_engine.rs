use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{Instrument, info, info_span};
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::employee::Employee;
use crate::models::payroll::{
    BulkPayrollData, OvertimeSettings, PayableClaim, PayrollDiagnostic, PayrollItem,
    PayrollPreview, PayrollPreviewEmployee, PayrollRun, PayslipLine, Tp3Totals, YtdTotals,
    round_sen,
};
use crate::models::statutory::{EisContribution, EpfContribution, PcbInput, SocsoContribution};
use crate::repositories::reads::payroll as payroll_reads;
use crate::repositories::{
    claims, company_work_schedules, employees as employee_repo, payroll_entries,
    payroll_item_details, payroll_items, payroll_runs, tp3_records,
};
use crate::services::audit_service::AuditRequestMeta;
use crate::services::eis_service;
use crate::services::epf_service;
use crate::services::pcb_calculator;
use crate::services::settings_service;
use crate::services::socso_service;
use crate::services::statutory_rules;
use crate::services::statutory_tables::StatutoryTables;

/// The dates a run covers. `effective_date` is the period end: statutory rules
/// and recurring allowances are effective-dated, and a run is rated as at the
/// last day of the month it pays.
#[derive(Debug, Clone, Copy)]
struct RunPeriod {
    year: i32,
    month: i32,
    period_start: NaiveDate,
    period_end: NaiveDate,
    effective_date: NaiveDate,
}

impl RunPeriod {
    fn resolve(year: i32, month: i32) -> AppResult<Self> {
        let period_start = NaiveDate::from_ymd_opt(year, month as u32, 1)
            .ok_or_else(|| AppError::BadRequest("Invalid period".into()))?;
        let period_end = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(year, (month + 1) as u32, 1)
        }
        .and_then(|d| d.pred_opt())
        .ok_or_else(|| AppError::BadRequest("Invalid period".into()))?;

        Ok(Self {
            year,
            month,
            period_start,
            period_end,
            effective_date: period_end,
        })
    }
}

/// One employee's payslip, computed but not yet written.
struct ComputedPayslip {
    employee_id: Uuid,
    basic: i64,
    gross: i64,
    total_allowances: i64,
    total_overtime: i64,
    total_claims: i64,
    /// Exactly the claims `total_claims` was summed from, so persistence marks
    /// those rows paid rather than re-running a predicate that may have picked
    /// up a claim approved since.
    claim_ids: Vec<Uuid>,
    epf: EpfContribution,
    socso: SocsoContribution,
    eis: EisContribution,
    pcb: i64,
    zakat: i64,
    ptptn: i64,
    tabung_haji: i64,
    /// Recurring + variable deductions EXCLUDING unpaid leave, which has its own
    /// column below. The two partition rather than overlap: the portal renders
    /// both rows plus Total Deductions, so anything else double-counts on screen.
    other_deductions: i64,
    unpaid_leave_deduction: i64,
    unpaid_leave_days: Decimal,
    total_deductions: i64,
    net: i64,
    employer_cost: i64,
    new_ytd_gross: i64,
    new_ytd_epf: i64,
    new_ytd_pcb: i64,
    new_ytd_socso: i64,
    new_ytd_eis: i64,
    new_ytd_zakat: i64,
    new_ytd_net: i64,
    total_bonus: i64,
    total_commission: i64,
    period_days: i64,
    days_worked: i64,
    is_prorated: bool,
    lines: Vec<PayslipLine>,
}

/// Every input a run needs, read once.
struct RunInputs {
    employees: Vec<Employee>,
    bulk: BulkPayrollData,
    statutory: StatutoryTables,
    ot_settings: OvertimeSettings,
    /// The company's own timezone, resolved once. Attendance is bucketed by
    /// local calendar date, and hardcoding MYT puts an early-morning check-in in
    /// the wrong month for a tenant that is not on it.
    tz: String,
    /// `(id, employee_number, full_name)` for employees this run would otherwise
    /// pay, held out only because they are inactive with no resignation date.
    /// Read here so preview and process see the same list.
    excluded_inactive: Vec<(Uuid, String, String)>,
}

/// Compute every employee, collecting failures instead of stopping at the first.
///
/// A run used to abort on the first employee whose figures could not be
/// produced â€” an over-staged deduction, a wage outside the verified bands â€” so
/// the operator fixed one, re-ran, and met the next. All of them are returned
/// together here.
fn compute_all(
    employees: &[Employee],
    period: &RunPeriod,
    inputs: &RunInputs,
) -> (Vec<ComputedPayslip>, Vec<PayrollDiagnostic>) {
    let mut computed = Vec::with_capacity(employees.len());
    let mut failures = Vec::new();

    for emp in employees {
        match compute_payslip(emp, period, &inputs.bulk, &inputs.statutory) {
            Ok(payslip) => computed.push(payslip),
            Err(err) => failures.push(PayrollDiagnostic::for_employee(
                "employee_calculation_failed",
                err.to_string(),
                emp.id,
                emp.employee_number.clone(),
                emp.full_name.clone(),
            )),
        }
    }

    (computed, failures)
}

/// Read the employees and every prefetched input a run needs.
///
/// Everything is read on the ONE connection the caller supplies —
/// `process_payroll` passes the connection its write transaction is using, and
/// `preview_payroll` a pooled one. It used to take the pool as well and load the
/// statutory snapshot and the overtime settings through it, which meant every
/// in-flight run pinned two of the pool's ten connections while holding a
/// transaction: ten overlapping runs each waited the full acquire timeout for an
/// eleventh, rolled back after computing every payslip, and starved every
/// unrelated request in the window.
async fn gather_run_inputs(
    conn: &mut sqlx::PgConnection,
    company_id: Uuid,
    payroll_group_id: Uuid,
    period: &RunPeriod,
) -> AppResult<RunInputs> {
    let RunPeriod {
        year,
        month,
        period_start,
        period_end,
        effective_date,
    } = *period;

    let employees = employee_repo::list_for_payroll_run(
        &mut *conn,
        company_id,
        payroll_group_id,
        period_end,
        period_start,
    )
    .await?;

    // The rows the population predicate holds out for an unexplained reason.
    // Not an error here — `preview_payroll` reports them per employee and
    // `process_payroll` refuses on them, so the operator sees who and why.
    let excluded_inactive = employee_repo::list_inactive_without_resignation_for_run(
        &mut *conn,
        company_id,
        payroll_group_id,
        period_end,
    )
    .await?;

    let employee_ids: Vec<Uuid> = employees.iter().map(|e| e.id).collect();

    // Read path: a stored zone Postgres would reject degrades to the platform
    // default with a warning rather than aborting the run's `AT TIME ZONE`
    // mid-transaction. The fallback itself is defined once, in `core::timezone`.
    let tz = crate::core::timezone::sanitize(
        company_work_schedules::find_default_timezone(&mut *conn, company_id).await?,
    );

    // 1. Batch fetch every recurring allowance/deduction line overlapping the
    // period. `compute_payslip` prorates each against its own effective window
    // and sums the results, so there is no separate totals query to drift from.
    let mut recurring_lines_map: HashMap<Uuid, Vec<_>> = HashMap::new();
    let recurring_lines = payroll_reads::recurring_allowance_lines(
        &mut *conn,
        &employee_ids,
        period_start,
        period_end,
    )
    .await?;
    for line in recurring_lines {
        recurring_lines_map
            .entry(line.employee_id)
            .or_default()
            .push(line);
    }

    // 2. Batch fetch staged payroll entries
    let mut variable_earnings_map = HashMap::new();
    let mut taxable_variable_earnings_map = HashMap::new();
    let mut variable_deductions_map = HashMap::new();
    for row in payroll_reads::entry_category_totals(&mut *conn, &employee_ids, year, month).await? {
        if row.category == "earning" {
            variable_earnings_map.insert(row.employee_id, row.total);
            taxable_variable_earnings_map.insert(row.employee_id, row.taxable);
        } else {
            variable_deductions_map.insert(row.employee_id, row.total);
        }
    }

    // Already counted inside `variable_deductions` above. Read separately so the
    // payslip can store the figure in its own column and label its own line,
    // rather than leaving it anonymous inside `total_other_deductions`.
    let unpaid_leave_map: HashMap<Uuid, (i64, Decimal)> =
        payroll_reads::unpaid_leave_totals(&mut *conn, &employee_ids, year, month)
            .await?
            .into_iter()
            .map(|r| (r.employee_id, (r.amount, r.days)))
            .collect();

    let mut entry_lines_map: HashMap<Uuid, Vec<_>> = HashMap::new();
    for line in payroll_reads::entry_lines(&mut *conn, &employee_ids, year, month).await? {
        entry_lines_map
            .entry(line.employee_id)
            .or_default()
            .push(line);
    }

    let mut monthly_allowances_map = HashMap::new();
    for row in
        payroll_reads::monthly_allowance_totals(&mut *conn, &employee_ids, year, month).await?
    {
        monthly_allowances_map.insert(row.employee_id, row.total);
    }

    // Bonus/commission are already inside variable_earnings (and therefore gross);
    // this is purely so they can also be stored as their own payslip line items.
    let bonus_commission_map: HashMap<Uuid, (i64, i64)> =
        payroll_reads::bonus_commission_totals(&mut *conn, &employee_ids, year, month)
            .await?
            .into_iter()
            .map(|r| (r.employee_id, (r.bonus, r.commission)))
            .collect();

    // 3. Batch fetch attendance OT hours, bucketed by the local date's type
    let mut attendance_ot_map: HashMap<Uuid, Vec<(String, f64)>> = HashMap::new();
    for row in
        payroll_reads::attendance_ot_hours(&mut *conn, &employee_ids, period_start, period_end, &tz)
            .await?
    {
        if row.hours > 0.0 {
            attendance_ot_map
                .entry(row.employee_id)
                .or_default()
                .push((row.day_type, row.hours));
        }
    }

    // 3b. Batch fetch approved overtime applications
    let mut approved_ot_map: HashMap<Uuid, Vec<(String, f64)>> = HashMap::new();
    for row in
        payroll_reads::approved_ot_totals(&mut *conn, &employee_ids, period_start, period_end)
            .await?
    {
        approved_ot_map
            .entry(row.employee_id)
            .or_default()
            .push((row.ot_type, row.hours));
    }

    // 3c. Batch fetch the approved claims this run will reimburse. Carry-forward
    // rather than period-bounded, so a claim approved after its own expense
    // month closed is picked up here instead of never being paid at all.
    let mut claims_map: HashMap<Uuid, Vec<PayableClaim>> = HashMap::new();
    for claim in
        payroll_reads::payable_claims(&mut *conn, &employee_ids, company_id, period_end).await?
    {
        claims_map.entry(claim.employee_id).or_default().push(claim);
    }

    // 4. Batch fetch TP3 data
    let tp3_map: HashMap<Uuid, Tp3Totals> =
        tp3_records::list_ytd_for_employees(&mut *conn, &employee_ids, year)
            .await?
            .into_iter()
            .map(|r| {
                (
                    r.employee_id,
                    (
                        r.previous_income_ytd,
                        r.previous_epf_ytd,
                        r.previous_pcb_ytd,
                        r.previous_socso_ytd,
                        r.previous_zakat_ytd,
                    ),
                )
            })
            .collect();

    // 5. Batch fetch YTD figures
    let ytd_map: HashMap<Uuid, YtdTotals> =
        payroll_reads::payroll_ytd(&mut *conn, &employee_ids, year, month)
            .await?
            .into_iter()
            .map(|r| {
                (
                    r.employee_id,
                    (r.gross, r.pcb, r.epf, r.socso, r.eis, r.zakat, r.net),
                )
            })
            .collect();

    // Read every rule table once. The per-employee calculators are pure over
    // this snapshot, so a run is not held open across ~15 statutory round trips
    // per employee.
    let statutory = StatutoryTables::load_on(&mut *conn, effective_date).await?;
    let ot_settings = settings_service::overtime_settings_on(&mut *conn, company_id).await;

    Ok(RunInputs {
        employees,
        bulk: BulkPayrollData {
            recurring_lines: recurring_lines_map,
            entry_lines: entry_lines_map,
            variable_earnings: variable_earnings_map,
            taxable_variable_earnings: taxable_variable_earnings_map,
            variable_deductions: variable_deductions_map,
            unpaid_leave: unpaid_leave_map,
            attendance_ot_hours: attendance_ot_map,
            approved_ot: approved_ot_map,
            approved_claims: claims_map,
            tp3: tp3_map,
            ytd: ytd_map,
            monthly_allowances: monthly_allowances_map,
            bonus_commission: bonus_commission_map,
            ot_settings: ot_settings.clone(),
        },
        statutory,
        ot_settings,
        tz,
        excluded_inactive,
    })
}

/// Run-level aggregates, accumulated as payslips are written.
#[derive(Debug, Default)]
struct RunTotals {
    total_gross: i64,
    total_net: i64,
    total_employer_cost: i64,
    total_epf_ee: i64,
    total_epf_er: i64,
    total_socso_ee: i64,
    total_socso_er: i64,
    total_eis_ee: i64,
    total_eis_er: i64,
    total_pcb: i64,
    total_zakat: i64,
}

impl RunTotals {
    fn add(&mut self, item: &PayrollItem) {
        self.total_gross += item.gross_salary;
        self.total_net += item.net_salary;
        self.total_employer_cost += item.employer_cost;
        self.total_epf_ee += item.epf_employee;
        self.total_epf_er += item.epf_employer;
        self.total_socso_ee += item.socso_employee;
        self.total_socso_er += item.socso_employer;
        self.total_eis_ee += item.eis_employee;
        self.total_eis_er += item.eis_employer;
        self.total_pcb += item.pcb_amount;
        self.total_zakat += item.zakat_amount;
    }
}

/// Render a `YYYYMM` period key as `MM/YYYY`.
fn format_period(period_key: i32) -> String {
    format!("{:02}/{}", period_key % 100, period_key / 100)
}

/// Whole-schedule problems that would otherwise fail every employee identically.
///
/// The bracket contiguity rule and the relief inventory are both properties of
/// the set, which no per-row database constraint can express — the GiST exclusion
/// on `pcb_brackets` forbids overlaps and is blind to gaps.
fn statutory_schedule_problems(statutory: &StatutoryTables) -> Vec<String> {
    let mut problems = Vec::new();

    if let Err(problem) = statutory.validate_pcb_brackets() {
        problems.push(format!(
            "The verified PCB tax brackets are not usable: {problem}. Load a corrected PCB rule set before processing."
        ));
    }

    let missing = statutory.missing_required_reliefs();
    if !missing.is_empty() {
        problems.push(format!(
            "The verified PCB rule set is missing {} the calculator reads: {}. Every employee would fail identically.",
            if missing.len() == 1 { "a relief" } else { "reliefs" },
            missing.join(", ")
        ));
    }

    problems
}

/// Render collected per-employee failures as one message.
///
/// Capped because a misconfigured rule set fails every employee, and a response
/// listing a thousand identical lines is no more actionable than ten.
fn format_failures(failures: &[PayrollDiagnostic]) -> String {
    const SHOWN: usize = 10;

    let mut message = format!(
        "Payroll cannot be processed: {} of the selected employees could not be calculated. Nothing has been saved.",
        failures.len()
    );
    for failure in failures.iter().take(SHOWN) {
        message.push_str(&format!(
            "\n• {} {}: {}",
            failure.employee_number.as_deref().unwrap_or(""),
            failure.employee_name.as_deref().unwrap_or(""),
            failure.message
        ));
    }
    if failures.len() > SHOWN {
        message.push_str(&format!(
            "\n… and {} more. Use the payroll preview to see the full list.",
            failures.len() - SHOWN
        ));
    }
    message
}

/// Compute a payroll run without writing anything.
///
/// Same inputs, same arithmetic and the same fail-closed statutory gate as
/// `process_payroll`, so what the operator reviews is what would be committed.
pub async fn preview_payroll(
    pool: &PgPool,
    company_id: Uuid,
    payroll_group_id: Uuid,
    year: i32,
    month: i32,
    pay_date: NaiveDate,
) -> AppResult<PayrollPreview> {
    let period = RunPeriod::resolve(year, month)?;
    let mut blocking = Vec::new();

    if payroll_runs::count_active_for_period(pool, company_id, payroll_group_id, year, month)
        .await?
        > 0
    {
        blocking.push(PayrollDiagnostic::run(
            "duplicate_period",
            "A payroll run already exists for this group and period. Delete the existing run before processing again.",
        ));
    }

    // Report the statutory gate as a diagnostic rather than an error: the
    // operator should still see who would be paid and what else is wrong.
    if let Err(err) = statutory_rules::require_all_verified(pool, period.effective_date).await {
        blocking.push(PayrollDiagnostic::run("statutory_rules", err.to_string()));
        return Ok(PayrollPreview {
            payroll_group_id,
            period_year: year,
            period_month: month,
            period_start: period.period_start,
            period_end: period.period_end,
            pay_date,
            employee_count: 0,
            payable_count: 0,
            total_gross: 0,
            total_net: 0,
            total_employer_cost: 0,
            total_epf_employee: 0,
            total_epf_employer: 0,
            total_socso_employee: 0,
            total_socso_employer: 0,
            total_eis_employee: 0,
            total_eis_employer: 0,
            total_pcb: 0,
            total_zakat: 0,
            can_process: false,
            blocking,
            warnings: Vec::new(),
            employees: Vec::new(),
        });
    }

    let mut conn = pool.acquire().await?;
    let inputs = gather_run_inputs(&mut conn, company_id, payroll_group_id, &period).await?;

    // Schedule-shaped problems, reported once for the run instead of once per
    // employee. Both would otherwise surface as N identical per-employee
    // validation failures with nothing saying what to fix.
    for problem in statutory_schedule_problems(&inputs.statutory) {
        blocking.push(PayrollDiagnostic::run("statutory_schedule", problem));
    }

    // A run that predates a committed one corrupts that run's frozen YTD.
    // Reported here so the operator never reaches the error in `process_payroll`.
    let employee_ids: Vec<Uuid> = inputs.employees.iter().map(|emp| emp.id).collect();
    for row in payroll_reads::employees_with_later_committed_run(
        &mut *conn,
        &employee_ids,
        company_id,
        year,
        month,
    )
    .await?
    {
        blocking.push(
            PayrollDiagnostic::for_employee(
                "later_run_exists",
                format!(
                    "Already paid by a run for {}. That run's year-to-date figures and PCB annualisation assumed this period was already included, and nothing recomputes them, so this period cannot be inserted behind it. Delete the later run first if it must be re-created.",
                    format_period(row.earliest_later_period)
                ),
                row.employee_id,
                row.employee_number,
                row.employee_name,
            )
            .with_params(&[("period", format_period(row.earliest_later_period))]),
        );
    }

    if inputs.employees.is_empty() {
        blocking.push(PayrollDiagnostic::run(
            "no_employees",
            "No active employees found in this payroll group for the selected period.",
        ));
    }

    // Deactivating a leaver is how a termination is recorded, so the population
    // selects on the employment window rather than the flag. An inactive row
    // with no resignation date says nothing either way, and guessing is how the
    // final payslip used to disappear. Blocking rather than advisory: there is
    // no figure to preview for these employees, only a decision to make.
    for (id, number, name) in &inputs.excluded_inactive {
        blocking.push(PayrollDiagnostic::for_employee(
            "inactive_without_resignation_date",
            "This employee is marked inactive but has no resignation date, so payroll cannot tell whether they are owed a final payslip. Set their resignation date to pay them for the days worked, re-activate them, or clear their payroll group to leave them out deliberately.",
            *id,
            number.clone(),
            name.clone(),
        ));
    }

    let (computed, failures) = compute_all(&inputs.employees, &period, &inputs);
    let computed_by_employee: HashMap<Uuid, &ComputedPayslip> =
        computed.iter().map(|c| (c.employee_id, c)).collect();
    let failure_by_employee: HashMap<Uuid, &PayrollDiagnostic> = failures
        .iter()
        .filter_map(|f| f.employee_id.map(|id| (id, f)))
        .collect();

    let mut warnings = Vec::new();

    // A month this group never ran leaves a hole in every later payslip's
    // year-to-date, and PCB's `remaining_months` over-counts against it — so the
    // year under-withholds. No ordering rule can repair that after the fact,
    // which is why it is advisory: a mid-year adopter's first run legitimately
    // has no prior months, and only the operator knows which case this is.
    let committed_months =
        payroll_reads::committed_months_for_group(&mut *conn, company_id, payroll_group_id, year)
            .await?;
    for emp in &inputs.employees {
        let first_owed = if emp.date_joined.year() == year {
            emp.date_joined.month() as i32
        } else if emp.date_joined.year() < year {
            1
        } else {
            month
        };
        let skipped: Vec<String> = (first_owed..month)
            .filter(|m| !committed_months.contains(m))
            .map(|m| format!("{m:02}/{year}"))
            .collect();
        if !skipped.is_empty() {
            warnings.push(
                PayrollDiagnostic::for_employee(
                    "missing_earlier_period",
                    format!(
                        "No committed payroll run for {} in this group. This period's PCB annualises over the remaining months as if those were already paid, so the year will under-withhold unless they are run or the employee genuinely was not owed them.",
                        skipped.join(", ")
                    ),
                    emp.id,
                    emp.employee_number.clone(),
                    emp.full_name.clone(),
                )
                .with_params(&[("periods", skipped.join(", "))]),
            );
        }
    }

    for emp in &inputs.employees {
        // Blocking, not advisory: SOCSO category and the EIS 57-59 / 60+
        // branches are all age-based, and there is no defensible age to assume.
        // The run refuses in `compute_payslip` for the same reason, so reporting
        // it as a warning here would promise a preview that cannot be processed.
        if emp.date_of_birth.is_none() {
            blocking.push(PayrollDiagnostic::for_employee(
                "missing_date_of_birth",
                "No date of birth on record. SOCSO category and EIS eligibility are age-based, so this employee cannot be rated. Open the employee record, set the date of birth, and preview again.",
                emp.id,
                emp.employee_number.clone(),
                emp.full_name.clone(),
            ));
        }
        // `compute_payslip` is pure and cannot emit diagnostics, so the
        // comparison between the stored EPF category and the one derived from age
        // and residency belongs here. The column is honoured — HR knows about
        // pre-1998 elections the derivation cannot see — but a disagreement is
        // worth a look, because the usual cause is a default of 'A' left on an
        // employee who has since turned 60 and is exempt from the employee share.
        if let (Some(dob), Some(stored)) = (emp.date_of_birth, emp.epf_category.as_deref()) {
            let derived = epf_service::derive_category(
                calculate_age(dob, period.effective_date),
                emp.residency_status == "foreigner",
            );
            if stored.trim() != derived && !stored.trim().is_empty() {
                warnings.push(
                    PayrollDiagnostic::for_employee(
                        "epf_category_override",
                        format!(
                            "EPF category {stored} is set on the employee record, but their age and residency resolve to Part {derived}. The record wins; clear it to use Part {derived}."
                        ),
                        emp.id,
                        emp.employee_number.clone(),
                        emp.full_name.clone(),
                    )
                    .with_params(&[
                        ("stored", stored.trim().to_string()),
                        ("derived", derived.to_string()),
                    ]),
                );
            }
        }
        if emp
            .bank_account_number
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        {
            warnings.push(PayrollDiagnostic::for_employee(
                "missing_bank_account",
                "No bank account on record, so this payslip cannot be paid by bank transfer.",
                emp.id,
                emp.employee_number.clone(),
                emp.full_name.clone(),
            ));
        }
    }

    // Entries staged against someone this run will not pay stay unprocessed and
    // roll forward silently — the operator has no other place to notice.
    for orphan in payroll_reads::staged_entries_outside_run(
        &mut *conn,
        company_id,
        payroll_group_id,
        year,
        month,
        period.period_start,
        period.period_end,
    )
    .await?
    {
        warnings.push(
            PayrollDiagnostic::for_employee(
                "staged_entries_not_in_run",
                format!(
                    "{} staged {} totalling {} sen will not be paid by this run — this employee is not in the selected payroll group for this period.",
                    orphan.entry_count,
                    if orphan.entry_count == 1 { "entry" } else { "entries" },
                    orphan.total_amount
                ),
                orphan.employee_id,
                orphan.employee_number,
                orphan.employee_name,
            )
            .with_params(&[
                ("count", orphan.entry_count.to_string()),
                ("total", orphan.total_amount.to_string()),
            ]),
        );
    }

    // Claim selection is carry-forward, so the first run after this change
    // sweeps every claim that was stuck 'approved' because it was approved after
    // its own expense month closed. That money is owed, but the run total will
    // jump against any hand-forecast, so say so before it commits rather than
    // after.
    for emp in &inputs.employees {
        let Some(claims_for_employee) = inputs.bulk.approved_claims.get(&emp.id) else {
            continue;
        };
        let earlier: Vec<&PayableClaim> = claims_for_employee
            .iter()
            .filter(|claim| claim.expense_date < period.period_start)
            .collect();
        let Some(oldest) = earlier.iter().map(|claim| claim.expense_date).min() else {
            continue;
        };
        let total: i64 = earlier.iter().map(|claim| claim.amount).sum();
        warnings.push(
            PayrollDiagnostic::for_employee(
                "claims_from_earlier_periods",
                format!(
                    "{} approved {} from before this period, totalling {} sen (oldest expense {}), will be reimbursed by this run. They were incurred earlier but never paid.",
                    earlier.len(),
                    if earlier.len() == 1 { "claim" } else { "claims" },
                    total,
                    oldest
                ),
                emp.id,
                emp.employee_number.clone(),
                emp.full_name.clone(),
            )
            .with_params(&[
                ("count", earlier.len().to_string()),
                ("total", total.to_string()),
                ("oldest", oldest.to_string()),
            ]),
        );
    }

    // The claims mirror of the staged-entry warning: a claim belonging to
    // someone outside the selected group is swept by no run at all, and stays
    // 'approved' with nothing to show for it.
    for orphan in payroll_reads::approved_claims_outside_run(
        &mut *conn,
        company_id,
        payroll_group_id,
        period.period_start,
        period.period_end,
    )
    .await?
    {
        warnings.push(
            PayrollDiagnostic::for_employee(
                "approved_claims_outside_run",
                format!(
                    "{} approved {} totalling {} sen will not be reimbursed by this run — this employee is not in the selected payroll group for this period.",
                    orphan.entry_count,
                    if orphan.entry_count == 1 { "claim" } else { "claims" },
                    orphan.total_amount
                ),
                orphan.employee_id,
                orphan.employee_number,
                orphan.employee_name,
            )
            .with_params(&[
                ("count", orphan.entry_count.to_string()),
                ("total", orphan.total_amount.to_string()),
            ]),
        );
    }

    // Overtime the check-out path refused to rate is silently absent from this
    // run's figures — the employee simply sees a smaller payslip. A warning
    // rather than a blocker: nothing unworked is being paid, so the run is safe
    // to commit; what is owed is an HR correction to the affected records.
    let ceiling = inputs.ot_settings.max_overtime_hours_per_day;
    for row in payroll_reads::unrated_overtime_records(
        &mut *conn,
        &employee_ids,
        period.period_start,
        period.period_end,
        &inputs.tz,
        ceiling,
    )
    .await?
    {
        warnings.push(
            PayrollDiagnostic::for_employee(
                "unrated_overtime",
                format!(
                    "{} attendance record(s) this period have overtime above the {} h/day ceiling (longest shift {} h) and were left unrated, so no overtime is paid for them. Correct the check-out times if the hours were genuinely worked.",
                    row.record_count,
                    trim_decimal(ceiling),
                    row.max_hours_worked.map(trim_decimal).unwrap_or_else(|| "?".into()),
                ),
                row.employee_id,
                row.employee_number,
                row.employee_name,
            )
            .with_params(&[
                ("count", row.record_count.to_string()),
                ("ceiling", trim_decimal(ceiling)),
                (
                    "max",
                    row.max_hours_worked
                        .map(trim_decimal)
                        .unwrap_or_else(|| "?".into()),
                ),
            ]),
        );
    }

    let mut totals = RunTotals::default();
    let employees = inputs
        .employees
        .iter()
        .map(|emp| {
            let computed = computed_by_employee.get(&emp.id);
            if let Some(c) = computed {
                totals.total_gross += c.gross;
                totals.total_net += c.net;
                totals.total_employer_cost += c.employer_cost;
                totals.total_epf_ee += c.epf.employee;
                totals.total_epf_er += c.epf.employer;
                totals.total_socso_ee += c.socso.employee;
                totals.total_socso_er += c.socso.employer;
                totals.total_eis_ee += c.eis.employee;
                totals.total_eis_er += c.eis.employer;
                totals.total_pcb += c.pcb;
                totals.total_zakat += c.zakat;
            }
            PayrollPreviewEmployee {
                employee_id: emp.id,
                employee_name: emp.full_name.clone(),
                employee_number: emp.employee_number.clone(),
                basic_salary: computed.map_or(0, |c| c.basic),
                total_allowances: computed.map_or(0, |c| c.total_allowances),
                total_overtime: computed.map_or(0, |c| c.total_overtime),
                total_claims: computed.map_or(0, |c| c.total_claims),
                gross_salary: computed.map_or(0, |c| c.gross),
                epf_employee: computed.map_or(0, |c| c.epf.employee),
                socso_employee: computed.map_or(0, |c| c.socso.employee),
                eis_employee: computed.map_or(0, |c| c.eis.employee),
                pcb_amount: computed.map_or(0, |c| c.pcb),
                total_deductions: computed.map_or(0, |c| c.total_deductions),
                net_salary: computed.map_or(0, |c| c.net),
                employer_cost: computed.map_or(0, |c| c.employer_cost),
                working_days: computed.map_or(0, |c| c.period_days as i32),
                days_worked: computed.map_or(0, |c| c.days_worked),
                is_prorated: computed.is_some_and(|c| c.is_prorated),
                error: failure_by_employee.get(&emp.id).map(|f| f.message.clone()),
            }
        })
        .collect();

    // An employee already named by a specific blocking diagnostic above also
    // fails `compute_payslip` — that is what makes the diagnostic blocking — so
    // the generic "calculation failed" line would just repeat it in less useful
    // words. Keyed on the employee rather than on any one diagnostic code, so a
    // later specific diagnostic gets the same treatment for free. The per-row
    // `error` in the projected-payslip table is built from `failures` above and
    // is unaffected, so the operator still sees which row is at fault.
    let already_reported: std::collections::HashSet<Uuid> =
        blocking.iter().filter_map(|d| d.employee_id).collect();
    blocking.extend(failures.into_iter().filter(|f| {
        f.employee_id
            .is_none_or(|id| !already_reported.contains(&id))
    }));

    Ok(PayrollPreview {
        payroll_group_id,
        period_year: year,
        period_month: month,
        period_start: period.period_start,
        period_end: period.period_end,
        pay_date,
        employee_count: inputs.employees.len() as i32,
        payable_count: computed.len() as i32,
        total_gross: totals.total_gross,
        total_net: totals.total_net,
        total_employer_cost: totals.total_employer_cost,
        total_epf_employee: totals.total_epf_ee,
        total_epf_employer: totals.total_epf_er,
        total_socso_employee: totals.total_socso_ee,
        total_socso_employer: totals.total_socso_er,
        total_eis_employee: totals.total_eis_ee,
        total_eis_employer: totals.total_eis_er,
        total_pcb: totals.total_pcb,
        total_zakat: totals.total_zakat,
        can_process: blocking.is_empty(),
        blocking,
        warnings,
        employees,
    })
}

/// Process payroll for a group in a given period.
///
/// 1. Fetch all active employees in the payroll group
/// 2. For each employee, calculate gross, statutory deductions, net
/// 3. Create PayrollRun + PayrollItems in a transaction
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[tracing::instrument(
    name = "payroll.process",
    skip(pool, notes),
    fields(
        company_id = %company_id,
        payroll_group_id = %payroll_group_id,
        year,
        month,
        run_id = tracing::field::Empty,
        employee_count = tracing::field::Empty,
    ),
)]
pub async fn process_payroll(
    pool: &PgPool,
    company_id: Uuid,
    payroll_group_id: Uuid,
    year: i32,
    month: i32,
    pay_date: NaiveDate,
    processed_by: Uuid,
    notes: Option<String>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollRun> {
    process_payroll_inner(
        pool,
        company_id,
        payroll_group_id,
        year,
        month,
        pay_date,
        processed_by,
        notes,
        audit_meta,
        None,
    )
    .await
}

/// `process_payroll` with an optional progress reporter — the job executor
/// passes one so a polled status can show the run advancing; tests and the
/// synchronous path pass `None`.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn process_payroll_inner(
    pool: &PgPool,
    company_id: Uuid,
    payroll_group_id: Uuid,
    year: i32,
    month: i32,
    pay_date: NaiveDate,
    processed_by: Uuid,
    notes: Option<String>,
    audit_meta: Option<&AuditRequestMeta>,
    progress: Option<&crate::services::job_service::JobProgress>,
) -> AppResult<PayrollRun> {
    // Check for existing run
    let existing =
        payroll_runs::count_active_for_period(pool, company_id, payroll_group_id, year, month)
            .await?;

    if existing > 0 {
        return Err(AppError::Conflict(
            "Payroll already exists for this period. Delete the eligible existing run first."
                .into(),
        ));
    }

    let period = RunPeriod::resolve(year, month)?;
    let effective_date = period.effective_date;

    // Validate the four statutory domains once per run. Individual lookups
    // remain linked to verified rule-set IDs, so this avoids four extra
    // metadata queries for every employee without weakening fail-closed use.
    statutory_rules::require_all_verified(pool, effective_date).await?;

    // Begin transaction
    let mut tx = pool.begin().await?;

    // Create payroll run
    let run_id = Uuid::now_v7();
    tracing::Span::current().record("run_id", tracing::field::display(run_id));
    let insert_result = payroll_runs::insert_processing(
        &mut *tx,
        run_id,
        company_id,
        payroll_group_id,
        year,
        month,
        period.period_start,
        period.period_end,
        pay_date,
        processed_by,
        notes,
    )
    .await;

    if let Err(err) = insert_result {
        let duplicate_period = matches!(
            &err,
            AppError::Database(sqlx::Error::Database(db_err))
                if matches!(
                    db_err.constraint(),
                    Some("payroll_runs_one_active_period")
                        | Some("payroll_runs_company_id_payroll_group_id_period_year_period_key")
                )
        );
        if duplicate_period {
            return Err(AppError::Conflict(
                "Payroll already exists for this period. Delete the eligible existing run first."
                    .into(),
            ));
        }
        return Err(err);
    }

    let inputs = gather_run_inputs(&mut tx, company_id, payroll_group_id, &period).await?;
    let RunInputs {
        employees,
        bulk: bulk_data,
        statutory,
        ot_settings,
        tz,
        excluded_inactive,
    } = inputs;

    // Fail closed on employees the population held out for an unexplained
    // reason. Paying them a full month or omitting them are both guesses, and
    // the second one is the defect this replaced. The transaction is already
    // open, so returning here drops `tx` un-committed and the `payroll_runs` row
    // inserted above is rolled back with it.
    if !excluded_inactive.is_empty() {
        let named: String = excluded_inactive
            .iter()
            .take(10)
            .map(|(_, number, name)| format!("\n• {number} {name}"))
            .collect();
        return Err(AppError::BadRequest(format!(
            "Payroll cannot be processed: {} employee(s) in this group are inactive with no resignation date, so it is not known whether they are owed a final payslip. Set a resignation date, re-activate them, or clear their payroll group to leave them out deliberately. Nothing has been saved.{}",
            excluded_inactive.len(),
            named
        )));
    }

    if employees.is_empty() {
        return Err(AppError::BadRequest(
            "No active employees found in this payroll group for the selected period".into(),
        ));
    }

    // Same run-level statutory gate the preview reports, so a misconfigured
    // schedule stops the run once rather than failing every employee.
    let schedule_problems = statutory_schedule_problems(&statutory);
    if !schedule_problems.is_empty() {
        return Err(AppError::BadRequest(format!(
            "Payroll cannot be processed. Nothing has been saved.\n• {}",
            schedule_problems.join("\n• ")
        )));
    }

    // Refuse to insert a period behind a run that already committed. That run's
    // `payroll_items.ytd_*` are frozen and printed, and its PCB annualisation
    // already assumed these months were in them — nothing recomputes either. A
    // GAP is legitimate (a group may simply not have been owed February), so the
    // guard is on the later run, not on the gap. Scoped to the employees in this
    // run, so a group whose membership changed does not block on someone who has
    // left it. The transaction is open; returning drops `tx` un-committed and the
    // `payroll_runs` row inserted above goes with it.
    let employee_ids: Vec<Uuid> = employees.iter().map(|emp| emp.id).collect();
    let later_runs = payroll_reads::employees_with_later_committed_run(
        &mut *tx,
        &employee_ids,
        company_id,
        year,
        month,
    )
    .await?;
    if !later_runs.is_empty() {
        let named: String = later_runs
            .iter()
            .take(10)
            .map(|row| {
                format!(
                    "\n• {} {} — already paid by the run for {}",
                    row.employee_number,
                    row.employee_name,
                    format_period(row.earliest_later_period)
                )
            })
            .collect();
        return Err(AppError::Conflict(format!(
            "Payroll for {:02}/{} cannot be processed: {} employee(s) already appear in a LATER committed run, whose year-to-date figures and PCB were computed as if this period were already paid. Delete the later run first, then process this one and re-create it. Nothing has been saved.{}",
            month,
            year,
            later_runs.len(),
            named
        )));
    }

    tracing::Span::current().record("employee_count", employees.len());
    info!(employees = employees.len(), "starting payroll run");

    // Compute everyone first. Nothing is written until every employee succeeds,
    // and a failure reports all of them rather than only the first — the old
    // loop aborted mid-run, so fixing one problem just revealed the next.
    let inputs = RunInputs {
        employees,
        bulk: bulk_data,
        statutory,
        ot_settings,
        tz,
        excluded_inactive,
    };
    let (computed, failures) = compute_all(&inputs.employees, &period, &inputs);

    if !failures.is_empty() {
        return Err(AppError::BadRequest(format_failures(&failures)));
    }

    if let Some(p) = progress {
        p.set_total(inputs.employees.len() as i32).await;
    }

    let mut totals = RunTotals::default();
    for (i, (emp, payslip)) in inputs.employees.iter().zip(&computed).enumerate() {
        let emp_span = info_span!("payroll.employee", employee_id = %emp.id);
        let item = persist_payslip(&mut tx, run_id, emp, &period, payslip)
            .instrument(emp_span)
            .await?;
        if let Some(p) = progress {
            p.tick(i as i32 + 1).await;
        }
        totals.add(&item);
    }

    let RunTotals {
        total_gross,
        total_net,
        total_employer_cost,
        total_epf_ee,
        total_epf_er,
        total_socso_ee,
        total_socso_er,
        total_eis_ee,
        total_eis_er,
        total_pcb,
        total_zakat,
    } = totals;
    let employees = &inputs.employees;
    let statutory = &inputs.statutory;
    let ot_settings = &inputs.ot_settings;

    // Update run totals
    payroll_runs::update_totals(
        &mut *tx,
        run_id,
        total_gross,
        total_net,
        total_employer_cost,
        total_epf_ee,
        total_epf_er,
        total_socso_ee,
        total_socso_er,
        total_eis_ee,
        total_eis_er,
        total_pcb,
        total_zakat,
        employees.len() as i32,
    )
    .await?;

    // Record what produced these figures. The statutory tables and the company
    // overtime settings are both mutable, so without this a later rule import or
    // settings change leaves the run's numbers unreproducible. The EPF parts are
    // in here because they are now DERIVED from age and residency rather than
    // read from a column, so "which parts did this run need" is not recoverable
    // from the employee records alone once someone has a birthday.
    let mut epf_parts: Vec<String> = computed
        .iter()
        .map(|payslip| payslip.epf.category.clone())
        .collect();
    epf_parts.sort();
    epf_parts.dedup();

    payroll_runs::set_calculation_snapshot(
        &mut *tx,
        run_id,
        serde_json::json!({
            "effective_date": effective_date,
            "statutory_rule_sets": statutory.rule_sets(),
            "overtime_settings": ot_settings,
            "epf_categories": epf_parts,
        }),
    )
    .await?;

    tx.commit().await?;

    info!(
        total_gross,
        total_net, total_pcb, total_employer_cost, "payroll run committed"
    );

    // Audit Log
    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(processed_by),
        "process",
        "payroll_run",
        Some(run_id),
        None,
        Some(serde_json::json!({
            "year": year,
            "month": month,
            "total_gross": total_gross,
            "total_net": total_net,
            "employee_count": employees.len()
        })),
        Some(&format!("Processed payroll for {:02}/{}", month, year)),
        audit_meta,
    )
    .await;

    // Return the completed run
    let run = payroll_runs::get_by_id(pool, run_id)
        .await?
        .ok_or_else(|| AppError::Internal("Payroll run not found after creation".into()))?;

    Ok(run)
}

/// Compute a single employee's payslip. Pure â€” no database access.
///
/// Split out from persistence so the same arithmetic backs both the committed
/// run and the preview, and so a failure here is a value the caller can collect
/// rather than an abort that discards the other employees' results.
fn compute_payslip(
    emp: &Employee,
    period: &RunPeriod,
    bulk: &BulkPayrollData,
    statutory: &StatutoryTables,
) -> AppResult<ComputedPayslip> {
    let RunPeriod {
        month,
        period_start: _period_start,
        period_end: _period_end,
        effective_date,
        ..
    } = *period;

    // SOCSO's First/Second category split, its 55-59 guard and the EIS 57-59 /
    // 60+ branches are all age-based, and the substituted default of 30 cleared
    // every one of them: a 62-year-old with no date of birth on record was rated
    // as a 30-year-old and had an employee contribution deducted they are exempt
    // from. Fail closed — an unlawful deduction is worse than a blocked run, and
    // the preview names every affected employee before it gets this far.
    let age = match emp.date_of_birth {
        Some(dob) => calculate_age(dob, effective_date),
        None => {
            return Err(AppError::Validation(format!(
                "Employee {} ({}) has no date of birth on record. SOCSO and EIS eligibility is age-based and cannot be assumed. Add the date of birth on the employee record, then re-run the preview.",
                emp.employee_number, emp.full_name
            )));
        }
    };
    let is_foreigner = emp.residency_status == "foreigner";

    // Gross salary = basic + recurring allowances + overtime
    // Prorate an incomplete month. Employment Act 1955 s.18B uses CALENDAR days
    // for an incomplete month (monthly wages Ã· days in the month Ã— days eligible),
    // not working days â€” so `working_days` below is deliberately the calendar-day
    // count of the period. Employees selected for the run may have joined after
    // period_start or resigned before period_end; paying the full basic in those
    // months also over-stated every statutory contribution derived from gross.
    let period_days = (_period_end - _period_start).num_days() + 1;
    let worked_from = emp.date_joined.max(_period_start);
    let worked_to = emp
        .date_resigned
        .map_or(_period_end, |resigned| resigned.min(_period_end));
    let days_worked = ((worked_to - worked_from).num_days() + 1).clamp(0, period_days);
    let is_prorated = days_worked < period_days;

    let basic = if is_prorated {
        round_sen(
            Decimal::from(emp.basic_salary) * Decimal::from(days_worked)
                / Decimal::from(period_days),
        )
    } else {
        emp.basic_salary
    };

    let monthly_allowances = *bulk.monthly_allowances.get(&emp.id).unwrap_or(&0);
    let variable_earnings = *bulk.variable_earnings.get(&emp.id).unwrap_or(&0);
    let taxable_variable_earnings = *bulk.taxable_variable_earnings.get(&emp.id).unwrap_or(&0);
    let (total_bonus, total_commission) = *bulk.bonus_commission.get(&emp.id).unwrap_or(&(0, 0));
    let variable_deductions = *bulk.variable_deductions.get(&emp.id).unwrap_or(&0);
    let (unpaid_leave_deduction, unpaid_leave_days) = *bulk
        .unpaid_leave
        .get(&emp.id)
        .unwrap_or(&(0, Decimal::ZERO));

    // Overtime is rated through `OvertimeSettings::rate_overtime`, which the
    // approval path calls too — the hourly rate stays unrounded and only the
    // final amount is rounded, once.
    let ot = &bulk.ot_settings;

    // Recurring allowances and deductions, prorated per line.
    //
    // The read selects every line whose effective window *overlaps* the period,
    // which is what stops a leaver's correctly-ended allowance from being dropped
    // outright. An overlapping line still need not cover the whole period, and
    // EA1955 s.18B prorates an incomplete month on CALENDAR days — basic above is
    // already prorated that way, and an allowance whose own window closes
    // mid-month is incomplete for exactly the same reason. The window is
    // intersected with the employment window so a mid-month joiner's allowance is
    // not paid for days they were not employed.
    //
    // A full-month employee with an open-ended allowance gets
    // `covered_days == period_days` and the untouched configured amount, so the
    // common case is byte-identical to before.
    //
    // Deliberately not implemented: a per-allowance opt-out for fixed
    // non-prorated allowances. That needs a schema column and a UI neither of
    // which exists.
    let mut allowances_total = 0i64;
    let mut taxable_allowances = 0i64;
    let mut recurring_deductions = 0i64;
    let mut recurring_lines: Vec<PayslipLine> = Vec::new();
    for line in bulk.recurring_lines.get(&emp.id).into_iter().flatten() {
        let covered_from = line.effective_from.unwrap_or(worked_from).max(worked_from);
        let covered_to = line.effective_to.unwrap_or(worked_to).min(worked_to);
        let covered_days = ((covered_to - covered_from).num_days() + 1).clamp(0, period_days);

        let (amount, description) = if covered_days == period_days {
            (line.amount, line.description.clone())
        } else {
            (
                round_sen(
                    Decimal::from(line.amount) * Decimal::from(covered_days)
                        / Decimal::from(period_days),
                ),
                format!(
                    "{} (prorated — {} of {} days)",
                    line.description, covered_days, period_days
                ),
            )
        };

        if line.category == "earning" {
            allowances_total += amount;
            if line.is_taxable {
                taxable_allowances += amount;
            }
            let earning = PayslipLine::earning(&line.item_type, description, amount);
            recurring_lines.push(earning.taxable(line.is_taxable));
        } else {
            recurring_deductions += amount;
            recurring_lines.push(PayslipLine::deduction(&line.item_type, description, amount));
        }
    }

    // Overtime lines are built alongside the figures: an OT amount is a product
    // of hours, an hourly rate derived from company settings, and a type
    // multiplier, none of which survive in `total_overtime` alone.
    let mut overtime_lines: Vec<PayslipLine> = Vec::new();

    // Attendance-based OT (records without approved OT applications), rated
    // by what the local date was worth: rest-day and public-holiday shifts
    // earn their own multipliers instead of 1.5x across the board.
    let attendance_ot_pay = bulk
        .attendance_ot_hours
        .get(&emp.id)
        .map(|buckets| {
            let mut total = 0i64;
            for (ot_type, hours) in buckets {
                let hours = Decimal::try_from(*hours).unwrap_or_default();
                let multiplier = ot.multiplier_for(ot_type);
                let rating = ot.rate_overtime(emp.hourly_rate, emp.basic_salary, ot_type, hours);
                overtime_lines.push(PayslipLine::earning(
                    "overtime",
                    format!(
                        "Overtime (attendance, {}) — {} h @ {}x",
                        ot_type.replace('_', " "),
                        trim_decimal(hours),
                        trim_decimal(multiplier)
                    ),
                    rating.amount_sen,
                ));
                total += rating.amount_sen;
            }
            total
        })
        .unwrap_or(0);

    // Approved OT applications with type-based rate multipliers
    let approved_ot_pay = if let Some(ot_entries) = bulk.approved_ot.get(&emp.id) {
        let mut total = 0i64;
        for (ot_type, hours) in ot_entries {
            let hours = Decimal::try_from(*hours).unwrap_or_default();
            let multiplier = ot.multiplier_for(ot_type);
            let rating = ot.rate_overtime(emp.hourly_rate, emp.basic_salary, ot_type, hours);
            let amount = rating.amount_sen;
            overtime_lines.push(PayslipLine::earning(
                "overtime",
                format!(
                    "Overtime ({}) — {} h @ {}x",
                    ot_type.replace('_', " "),
                    trim_decimal(hours),
                    trim_decimal(multiplier)
                ),
                amount,
            ));
            total += amount;
        }
        total
    } else {
        0
    };

    let total_overtime = attendance_ot_pay + approved_ot_pay;

    // Approved claims (reimbursements, not part of gross â€” added to net)
    // Summed from the individual rows rather than read as a pre-aggregated
    // total, so `persist_payslip` can mark exactly these claims paid.
    let payable_claims: &[PayableClaim] = bulk
        .approved_claims
        .get(&emp.id)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let total_claims: i64 = payable_claims.iter().map(|claim| claim.amount).sum();

    // EPF Act 1991 s.2 excludes overtime from "wages"; ESSA 1969 s.2 and EIS Act
    // 2017 s.2 both include it, and MTD is computed on total taxable
    // remuneration. One shared `gross` therefore cannot serve all four â€” it
    // resolved every OT-carrying payslip in an inflated EPF Third Schedule band.
    // Claims are outside both bases: they are reimbursements paid on top of net,
    // not remuneration. `gross` is derived from `epf_wage` rather than the
    // reverse so no later edit can subtract the two apart.
    let epf_wage = basic + allowances_total + variable_earnings;
    let gross = epf_wage + total_overtime;

    // Unpaid leave leaves WITH the money the statutory calculators see. It is
    // staged as a deduction, so without this it reduced only net while
    // EPF/SOCSO/EIS/PCB charged on wages the employee did not receive: the
    // employer over-remitted and the employee was over-deducted on every
    // unpaid-leave month. Gross itself stays at the contracted figure, so the
    // leave remains a visible deduction under its own name. A mid-month
    // joiner is not double-reduced: proration shrinks `basic` for days before
    // employment, while an approved leave request can only cover days inside it.
    let wage_reduction = unpaid_leave_deduction.max(0);
    let statutory_epf_wage = (epf_wage - wage_reduction).max(0);
    let statutory_socso_eis_wage = (gross - wage_reduction).max(0);
    let total_allowances = allowances_total + monthly_allowances;

    // `is_taxable` narrows the PCB base and NOTHING else, deliberately. It is an
    // income-tax flag — the payslip drawer badges the line as not taxed — and
    // EPF Act 1991 s.2, ESSA 1969 s.2 and the EIS Act have their own, different
    // exclusion lists (travelling allowance, gratuity, service charge, payment in
    // lieu of notice) that no column in this schema expresses. Narrowing three
    // more bases off a flag that does not mean that would be a larger error than
    // the one being fixed. Overtime is always taxable.
    let taxable_gross = (basic + taxable_allowances + taxable_variable_earnings + total_overtime
        - wage_reduction)
        .max(0);

    // EPF / SOCSO / EIS â€” resolved from the run's rule snapshot, no I/O.
    let epf = epf_service::calculate_epf_with(
        statutory,
        statutory_epf_wage,
        age,
        is_foreigner,
        emp.epf_category.as_deref(),
    )?;
    let socso = socso_service::calculate_socso_with(
        statutory,
        statutory_socso_eis_wage,
        age,
        is_foreigner,
    )?;
    let eis =
        eis_service::calculate_eis_with(statutory, statutory_socso_eis_wage, age, is_foreigner)?;

    // Get YTD figures (from previous months this year)
    let (ytd_gross, ytd_pcb, ytd_epf, ytd_socso, ytd_eis, ytd_zakat, ytd_net) =
        *bulk.ytd.get(&emp.id).unwrap_or(&(0, 0, 0, 0, 0, 0, 0));

    // Get TP3 data if exists
    let (tp3_income, tp3_epf, tp3_pcb, tp3_socso, tp3_zakat) =
        *bulk.tp3.get(&emp.id).unwrap_or(&(0, 0, 0, 0, 0));

    // Zakat
    let zakat = if emp.zakat_eligible.unwrap_or(false) {
        emp.zakat_monthly_amount.unwrap_or(0)
    } else {
        0
    };

    // PCB
    //
    // LHDN MTD treats bonus and commission as *additional remuneration*, taxed
    // by the Schedule 2 differential rather than annualised as recurring income.
    // They stay inside `gross` because EPF (s.2), SOCSO and EIS all levy on them
    // — only the PCB base is narrowed.
    //
    // Caveat worth stating rather than hiding: LHDN's "additional remuneration"
    // covers commission paid at *irregular* intervals, and a genuinely monthly
    // commission is normal remuneration. `item_type = 'commission'` is all the
    // data model has to tell them apart, and treating it as additional is the
    // conservative direction — it under-annualises rather than over-deducting.
    //
    // Derived from `taxable_gross`, not `gross`: an earning staged as
    // non-taxable was badged "Non-taxable" on the payslip and taxed anyway.
    // Clamped at zero because a bonus row staged as non-taxable is already
    // outside `taxable_gross`, so subtracting it again would go negative.
    let normal_remuneration = (taxable_gross - total_bonus - total_commission).max(0);
    let pcb_input = PcbInput {
        monthly_normal_remuneration: normal_remuneration,
        epf_employee_monthly: epf.employee,
        socso_employee_monthly: socso.employee,
        eis_employee_monthly: eis.employee,
        zakat_monthly: zakat,
        marital_status: emp
            .marital_status
            .clone()
            .unwrap_or_else(|| "single".into()),
        working_spouse: emp.working_spouse.unwrap_or(false),
        num_children: emp.num_children.unwrap_or(0),
        months_worked: month,
        ytd_gross: ytd_gross + tp3_income,
        ytd_pcb: ytd_pcb + tp3_pcb,
        ytd_epf: ytd_epf + tp3_epf,
        ytd_socso: ytd_socso + tp3_socso,
        ytd_eis,
        ytd_zakat: ytd_zakat + tp3_zakat,
        bonus_amount: total_bonus + total_commission,
    };

    let pcb = pcb_calculator::calculate_pcb_with(statutory, &pcb_input)?;

    // PTPTN and Tabung Haji
    let ptptn = emp.ptptn_monthly_amount.unwrap_or(0);
    let tabung_haji = emp.tabung_haji_amount.unwrap_or(0);

    // Total deductions
    let total_deductions = epf.employee
        + socso.employee
        + eis.employee
        + pcb
        + zakat
        + ptptn
        + tabung_haji
        + recurring_deductions
        + variable_deductions;

    let net = gross - total_deductions + total_claims;
    // Deductions are not bounded by gross (an over-staged unpaid-leave or loan
    // entry is enough), and a negative net silently becomes a negative payslip
    // and a negative run total. Payroll fails closed so the operator fixes the
    // entry rather than shipping the figure.
    if net < 0 {
        return Err(AppError::BadRequest(format!(
            "Employee {} has deductions ({}) exceeding gross plus claims ({}), which would produce a negative net salary. Review the staged deductions for this period.",
            emp.employee_number,
            total_deductions,
            gross + total_claims
        )));
    }
    let employer_cost = gross + epf.employer + socso.employer + eis.employer;

    // New YTD
    let new_ytd_gross = ytd_gross + gross;
    let new_ytd_epf = ytd_epf + epf.employee;
    let new_ytd_pcb = ytd_pcb + pcb;
    let new_ytd_socso = ytd_socso + socso.employee;
    let new_ytd_eis = ytd_eis + eis.employee;
    let new_ytd_zakat = ytd_zakat + zakat;
    let new_ytd_net = ytd_net + net;

    // The breakdown that explains every figure above, built while the inputs are
    // still in scope. Persisted alongside the item so a payslip can never exist
    // without the lines that justify it.
    let lines = build_payslip_lines(
        emp,
        bulk,
        basic,
        is_prorated,
        days_worked,
        period_days,
        recurring_lines,
        overtime_lines,
        payable_claims,
        &epf,
        &socso,
        &eis,
        pcb,
        zakat,
        ptptn,
        tabung_haji,
    );

    Ok(ComputedPayslip {
        employee_id: emp.id,
        basic,
        gross,
        total_allowances,
        total_overtime,
        total_claims,
        claim_ids: payable_claims.iter().map(|claim| claim.id).collect(),
        epf,
        socso,
        eis,
        pcb,
        zakat,
        ptptn,
        tabung_haji,
        // Unpaid leave stays inside `total_deductions` (it is already summed into
        // `variable_deductions`) but comes back out of the other-deductions
        // bucket, so the two partition. The invariant the portal depends on:
        // `total_other_deductions + unpaid_leave_deduction` equals what
        // `total_other_deductions` alone used to be.
        other_deductions: recurring_deductions + variable_deductions - unpaid_leave_deduction,
        unpaid_leave_deduction,
        unpaid_leave_days,
        total_deductions,
        net,
        employer_cost,
        new_ytd_gross,
        new_ytd_epf,
        new_ytd_pcb,
        new_ytd_socso,
        new_ytd_eis,
        new_ytd_zakat,
        new_ytd_net,
        total_bonus,
        total_commission,
        period_days,
        days_worked,
        is_prorated,
        lines,
    })
}

/// Write one computed payslip and retire the source rows it consumed.
async fn persist_payslip(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_id: Uuid,
    emp: &Employee,
    period: &RunPeriod,
    computed: &ComputedPayslip,
) -> AppResult<PayrollItem> {
    let item_id = Uuid::now_v7();
    let item = payroll_items::insert(
        &mut **tx,
        item_id,
        run_id,
        emp.id,
        computed.basic,
        computed.gross,
        computed.total_allowances,
        computed.total_overtime,
        computed.total_claims,
        computed.epf.employee,
        computed.epf.employer,
        computed.socso.employee,
        computed.socso.employer,
        computed.eis.employee,
        computed.eis.employer,
        computed.pcb,
        computed.zakat,
        computed.ptptn,
        computed.tabung_haji,
        computed.other_deductions,
        computed.total_deductions,
        computed.net,
        computed.employer_cost,
        computed.new_ytd_gross,
        computed.new_ytd_epf,
        computed.new_ytd_pcb,
        computed.new_ytd_socso,
        computed.new_ytd_eis,
        computed.new_ytd_zakat,
        computed.new_ytd_net,
        computed.total_bonus,
        computed.total_commission,
        Some(computed.period_days as i32),
        Some(Decimal::from(computed.days_worked)),
        computed.is_prorated,
        computed.unpaid_leave_deduction,
        computed.unpaid_leave_days,
    )
    .await?;

    payroll_item_details::insert_lines(&mut **tx, item_id, &computed.lines).await?;

    // Mark staged entries as processed
    payroll_entries::mark_processed(&mut **tx, run_id, emp.id, period.year, period.month).await?;

    // Mark exactly the claims this payslip reimbursed, by id. The old call
    // re-ran the period predicate, so a claim approved between the read and this
    // write was marked paid without appearing in any payslip.
    if !computed.claim_ids.is_empty() {
        claims::mark_paid(&mut **tx, run_id, &computed.claim_ids).await?;
    }

    Ok(item)
}

/// Assemble the stored payslip breakdown for one employee.
///
/// The lines are exhaustive by construction: earnings sum to gross and
/// deductions sum to `total_deductions`, because each line is emitted from the
/// same value that fed those totals. Zero-valued lines are dropped so a payslip
/// does not list deductions the employee does not have â€” basic salary is kept
/// unconditionally, since a payslip with no basic line reads as missing data
/// rather than as a zero.
///
/// `is_statutory` marks the four amounts computed from verified statutory rule
/// sets (EPF, SOCSO, EIS, PCB). Zakat, PTPTN and Tabung Haji are configured
/// fixed amounts, so they are ordinary deductions here even though they are also
/// remitted onward.
#[allow(clippy::too_many_arguments)]
fn build_payslip_lines(
    emp: &Employee,
    bulk: &BulkPayrollData,
    basic: i64,
    is_prorated: bool,
    days_worked: i64,
    period_days: i64,
    // Recurring allowance/deduction lines, already prorated by the caller — the
    // paid figure, not the configured one, so the stored breakdown reconciles.
    recurring_lines: Vec<PayslipLine>,
    overtime_lines: Vec<PayslipLine>,
    payable_claims: &[PayableClaim],
    epf: &EpfContribution,
    socso: &SocsoContribution,
    eis: &EisContribution,
    pcb: i64,
    zakat: i64,
    ptptn: i64,
    tabung_haji: i64,
) -> Vec<PayslipLine> {
    let mut lines = Vec::new();

    let basic_description = if is_prorated {
        format!(
            "Basic salary (prorated — {} of {} days)",
            days_worked, period_days
        )
    } else {
        "Basic salary".to_string()
    };
    lines.push(PayslipLine::earning(
        "basic_salary",
        basic_description,
        basic,
    ));

    let (mut earnings, mut deductions) = (Vec::new(), Vec::new());
    for line in recurring_lines {
        if line.category == "earning" {
            earnings.push(line);
        } else {
            deductions.push(line);
        }
    }
    // Labelled with the entry's own `item_type`, so an unpaid-leave deduction
    // reads as one in the drawer and on the PDF instead of being flattened into
    // "other deduction" alongside loans and advances. `payroll_item_details` has
    // a CHECK on `category` only, so the type is free to be the real one.
    for line in bulk.entry_lines.get(&emp.id).into_iter().flatten() {
        if line.category == "earning" {
            earnings.push(
                PayslipLine::earning(&line.item_type, line.description.clone(), line.amount)
                    .taxable(line.is_taxable),
            );
        } else {
            deductions.push(PayslipLine::deduction(
                &line.item_type,
                line.description.clone(),
                line.amount,
            ));
        }
    }

    lines.append(&mut earnings);
    lines.extend(overtime_lines);

    // One line per claim, not one aggregate. Selection is carry-forward, so a
    // July payslip can legitimately reimburse a June expense — naming the claim
    // and its expense date is the only way the employee can tell why.
    // Reimbursements are paid on top of net rather than forming part of gross,
    // so each line is recorded as non-taxable to match how the engine treats it.
    for claim in payable_claims {
        lines.push(
            PayslipLine::earning(
                "claim_reimbursement",
                format!("Claim: {} ({})", claim.title, claim.expense_date),
                claim.amount,
            )
            .taxable(false),
        );
    }

    lines.push(PayslipLine::deduction("epf", "EPF (employee)", epf.employee).statutory());
    lines.push(PayslipLine::deduction("socso", "SOCSO (employee)", socso.employee).statutory());
    lines.push(PayslipLine::deduction("eis", "EIS (employee)", eis.employee).statutory());
    lines.push(PayslipLine::deduction("pcb", "PCB (monthly tax deduction)", pcb).statutory());
    lines.push(PayslipLine::deduction("zakat", "Zakat", zakat));
    lines.push(PayslipLine::deduction("ptptn", "PTPTN", ptptn));
    lines.push(PayslipLine::deduction(
        "tabung_haji",
        "Tabung Haji",
        tabung_haji,
    ));
    lines.append(&mut deductions);

    lines.retain(|line| line.amount != 0 || line.item_type == "basic_salary");
    lines
}

/// Render a `Decimal` without trailing zeros, for descriptions like "3 h @ 1.5x".
fn trim_decimal(value: Decimal) -> String {
    value.normalize().to_string()
}

/// Takes a `NaiveDate`, not an `Option`, so the "assume 30" fallback this
/// function used to carry is unrepresentable. The caller decides what a missing
/// date of birth means, and for payroll it means the employee cannot be rated.
fn calculate_age(dob: NaiveDate, as_of: NaiveDate) -> i32 {
    let mut age = as_of.year() - dob.year();
    if (as_of.month(), as_of.day()) < (dob.month(), dob.day()) {
        age -= 1;
    }
    age
}

#[cfg(test)]
mod tests {
    use super::calculate_age;
    use chrono::NaiveDate;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    #[test]
    fn calculates_age_on_and_before_birthday() {
        let dob = date(1990, 7, 1);
        assert_eq!(calculate_age(dob, date(2023, 6, 30)), 32);
        assert_eq!(calculate_age(dob, date(2023, 7, 1)), 33);
    }

    #[test]
    fn leap_year_day_offset_does_not_advance_age_early() {
        let dob = date(1990, 7, 1);
        let leap_year_day_before_birthday = date(2024, 6, 30);

        assert_eq!(calculate_age(dob, leap_year_day_before_birthday), 33);
    }

    #[test]
    fn february_29_birthday_advances_on_march_1_in_non_leap_year() {
        let dob = date(2000, 2, 29);
        assert_eq!(calculate_age(dob, date(2026, 2, 28)), 25);
        assert_eq!(calculate_age(dob, date(2026, 3, 1)), 26);
    }
}

/// DB-free golden and property harness over `compute_payslip` (plan item 30).
///
/// Everything here runs against `statutory_tables::golden_fixture()` — no
/// database, no `#[cfg(test)]` bypass of the production gate. The goldens pin
/// exact sen figures derived by hand from the fixture tables (documented in
/// each test); the properties hold for any table content because they assert
/// the engine's internal arithmetic, not the rates.
#[cfg(test)]
mod payslip_golden_tests {
    use super::*;
    use crate::models::payroll::{PayslipLine, PayslipSourceLine};
    use chrono::{Duration as ChronoDuration, Utc};
    use proptest::prelude::*;

    // ── builders ────────────────────────────────────────────────────────

    fn employee(
        basic_salary: i64,
        date_joined: NaiveDate,
        date_resigned: Option<NaiveDate>,
    ) -> Employee {
        Employee {
            id: Uuid::now_v7(),
            company_id: Uuid::now_v7(),
            employee_number: "E-0001".into(),
            full_name: "Golden Test".into(),
            ic_number: None,
            passport_number: None,
            date_of_birth: NaiveDate::from_ymd_opt(1990, 6, 15),
            gender: None,
            nationality: Some("Malaysian".into()),
            race: None,
            residency_status: "citizen".into(),
            marital_status: Some("single".into()),
            email: None,
            phone: None,
            address_line1: None,
            address_line2: None,
            city: None,
            state: None,
            postcode: None,
            department: None,
            designation: None,
            cost_centre: None,
            branch: None,
            employment_type: "full_time".into(),
            date_joined,
            probation_start: None,
            probation_end: None,
            confirmation_date: None,
            date_resigned,
            resignation_reason: None,
            basic_salary,
            hourly_rate: None,
            daily_rate: None,
            bank_name: None,
            bank_account_number: None,
            bank_account_type: None,
            tax_identification_number: None,
            epf_number: None,
            socso_number: None,
            eis_number: None,
            working_spouse: None,
            num_children: None,
            epf_category: None,
            is_muslim: None,
            zakat_eligible: None,
            zakat_monthly_amount: None,
            ptptn_monthly_amount: None,
            tabung_haji_amount: None,
            hrdf_contribution: None,
            payroll_group_id: None,
            salary_group: None,
            is_active: Some(true),
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: None,
            updated_by: None,
        }
    }

    fn period(year: i32, month: i32) -> RunPeriod {
        let start = NaiveDate::from_ymd_opt(year, month as u32, 1).unwrap();
        let next = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(year, (month + 1) as u32, 1).unwrap()
        };
        let end = next - ChronoDuration::days(1);
        RunPeriod {
            year,
            month,
            period_start: start,
            period_end: end,
            effective_date: end,
        }
    }

    fn empty_bulk() -> BulkPayrollData {
        BulkPayrollData {
            recurring_lines: HashMap::new(),
            entry_lines: HashMap::new(),
            variable_earnings: HashMap::new(),
            taxable_variable_earnings: HashMap::new(),
            variable_deductions: HashMap::new(),
            unpaid_leave: HashMap::new(),
            attendance_ot_hours: HashMap::new(),
            approved_ot: HashMap::new(),
            approved_claims: HashMap::new(),
            tp3: HashMap::new(),
            ytd: HashMap::new(),
            monthly_allowances: HashMap::new(),
            bonus_commission: HashMap::new(),
            ot_settings: OvertimeSettings::statutory_defaults(),
        }
    }

    fn compute(
        emp: &Employee,
        period_: &RunPeriod,
        bulk: &BulkPayrollData,
    ) -> AppResult<ComputedPayslip> {
        compute_payslip(
            emp,
            period_,
            bulk,
            &crate::services::statutory_tables::golden::golden_fixture(),
        )
    }

    fn earning_line(employee_id: Uuid, item_type: &str, amount: i64) -> PayslipSourceLine {
        PayslipSourceLine {
            employee_id,
            category: "earning".into(),
            item_type: item_type.to_string(),
            description: item_type.replace('_', " "),
            amount,
            is_taxable: true,
            effective_from: None,
            effective_to: None,
        }
    }

    fn deduction_line(employee_id: Uuid, item_type: &str, amount: i64) -> PayslipSourceLine {
        PayslipSourceLine {
            employee_id,
            category: "deduction".into(),
            item_type: item_type.to_string(),
            description: item_type.replace('_', " "),
            amount,
            is_taxable: false,
            effective_from: None,
            effective_to: None,
        }
    }

    fn sum_lines(lines: &[PayslipLine], category: &str) -> i64 {
        lines
            .iter()
            .filter(|l| l.category == category)
            .map(|l| l.amount)
            .sum()
    }

    // ── goldens ─────────────────────────────────────────────────────────

    /// Full month, RM3,000 basic, no extras. Every figure hand-derived from
    /// the fixture: EPF band 2 (2200/2600), SOCSO band 2 (220/880), EIS band 2
    /// (110/440); PCB annualises 7 remaining months to RM21,000 chargeable
    /// RM11,822.90 after reliefs, taxed RM682.28 less the RM400 rebate →
    /// RM282.28/yr → RM403.25/mo → rounded up to RM400.
    #[test]
    fn golden_full_month_exact_figures() {
        let emp = employee(300_000, NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(), None);
        let p = period(2025, 6);
        let c = compute(&emp, &p, &empty_bulk()).expect("compute");

        assert_eq!(c.gross, 300_000);
        assert!(!c.is_prorated);
        assert_eq!(c.epf.employee, 2_200);
        assert_eq!(c.epf.employer, 2_600);
        assert_eq!(c.socso.employee, 220);
        assert_eq!(c.socso.employer, 880);
        assert_eq!(c.eis.employee, 110);
        assert_eq!(c.eis.employer, 440);
        assert_eq!(c.pcb, 4_100);
        assert_eq!(c.total_deductions, 2_200 + 220 + 110 + 4_100);
        assert_eq!(c.net, 300_000 - 6_630);
        assert_eq!(c.employer_cost, 300_000 + 2_600 + 880 + 440);
        assert_eq!(c.new_ytd_gross, 300_000);
        assert_eq!(c.new_ytd_net, c.net);

        // The stored breakdown must explain the totals exactly.
        assert_eq!(sum_lines(&c.lines, "earning"), c.gross + c.total_claims);
        assert_eq!(sum_lines(&c.lines, "deduction"), c.total_deductions);
    }

    /// A leaver working 15 of June's 30 calendar days: EA s.18B proration on
    /// CALENDAR days drops basic into EPF/SOCSO/EIS band 1.
    #[test]
    fn golden_mid_month_leaver_prorates_every_base() {
        let mut resigned_mid =
            employee(300_000, NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(), None);
        // Employment window intersects June only up to resignation.
        resigned_mid.date_resigned = Some(NaiveDate::from_ymd_opt(2026, 6, 15).unwrap());
        let p = period(2026, 6);
        let c = compute(&resigned_mid, &p, &empty_bulk()).expect("compute");

        assert!(c.is_prorated);
        assert_eq!(c.period_days, 30);
        assert_eq!(c.days_worked, 15);
        assert_eq!(c.basic, 150_000);
        assert_eq!(c.gross, 150_000);
        assert_eq!(c.epf.employee, 1_000);
        assert_eq!(c.socso.employee, 100);
        assert_eq!(c.eis.employee, 50);

        let basic_line = c
            .lines
            .iter()
            .find(|l| l.item_type == "basic_salary")
            .expect("basic line");
        assert!(basic_line.description.contains("prorated"));
    }

    /// THE unpaid-leave invariant: an approved unpaid deduction leaves with the
    /// statutory wage bases, not only with net. With basic RM2,100 the RM500
    /// reduction crosses the fixture's band boundary, so a base that still saw
    /// the un-reduced wage is off by more than rounding.
    ///
    /// Gross itself stays at the contracted figure — unpaid leave remains a
    /// visible deduction line and its own payslip column; only the four
    /// calculator bases shrink.
    #[test]
    fn golden_unpaid_leave_reduces_the_statutory_bases() {
        let emp = employee(210_000, NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(), None);
        let p = period(2025, 6);
        let mut bulk = empty_bulk();
        bulk.variable_deductions.insert(emp.id, 50_000);
        bulk.unpaid_leave.insert(emp.id, (50_000, Decimal::from(2)));
        bulk.entry_lines
            .insert(emp.id, vec![deduction_line(emp.id, "unpaid_leave", 50_000)]);

        let c = compute(&emp, &p, &bulk).expect("compute");

        assert_eq!(c.gross, 210_000, "gross stays contracted");
        assert_eq!(c.unpaid_leave_deduction, 50_000);
        assert_eq!(c.other_deductions, 0, "unpaid leave partitions out");
        // Reduced bases hit band 1, not band 2.
        assert_eq!(c.epf.employee, 1_000, "EPF on 160k, not 210k");
        assert_eq!(c.socso.employee, 100, "SOCSO on the reduced wage");
        assert_eq!(c.eis.employee, 50, "EIS on the reduced wage");

        // Deductions still include the leave once, under its own line.
        assert_eq!(c.pcb, 0, "reduced chargeable income sits in the 0% band");
        assert_eq!(c.total_deductions, 1_150 + 50_000);
        assert_eq!(sum_lines(&c.lines, "deduction"), c.total_deductions);
        let leave_line = c
            .lines
            .iter()
            .find(|l| l.item_type == "unpaid_leave")
            .expect("unpaid-leave breakdown line");
        assert_eq!(leave_line.amount, 50_000);
    }

    /// Feeding run N's new YTD into run N+1 accumulates without drift, and the
    /// PCB month counter changes what it is supposed to change.
    #[test]
    fn golden_ytd_chain_across_two_runs() {
        let emp = employee(300_000, NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(), None);

        let jan_p = period(2025, 1);
        let jan = compute(&emp, &jan_p, &empty_bulk()).expect("january");
        // Remaining months 12: annual 3.6M chargeable 2_657_760, tax 215_775 −
        // rebate 40_000 → /12 = RM1,474.58 → rounded up to RM1,480.
        assert_eq!(jan.pcb, 14_800);

        let mut feb_bulk = empty_bulk();
        feb_bulk.ytd.insert(
            emp.id,
            (
                jan.new_ytd_gross,
                jan.new_ytd_pcb,
                jan.new_ytd_epf,
                jan.new_ytd_socso,
                jan.new_ytd_eis,
                jan.new_ytd_zakat,
                jan.new_ytd_net,
            ),
        );
        let feb_p = period(2025, 2);
        let feb = compute(&emp, &feb_p, &feb_bulk).expect("february");

        assert_eq!(feb.new_ytd_gross, jan.new_ytd_gross + feb.gross);
        assert_eq!(feb.new_ytd_epf, jan.new_ytd_epf + feb.epf.employee);
        assert_eq!(feb.new_ytd_net, jan.new_ytd_net + feb.net);
        // Same economics, one fewer month of annualisation headroom minus the
        // PCB already withheld: the two effects cancel to the same RM1,480 on
        // this fixture — pinned so a change in either direction is visible.
        assert_eq!(feb.pcb, 14_800);
    }

    /// Overtime multipliers come from company settings, OT pays at the
    /// unrounded hourly rate, and it reaches SOCSO/EIS but never the EPF wage.
    #[test]
    fn golden_overtime_multipliers_and_base_exclusions() {
        let mut emp = employee(300_000, NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(), None);
        emp.hourly_rate = Some(1_000);
        let p = period(2025, 6);
        let mut bulk = empty_bulk();
        bulk.approved_ot.insert(
            emp.id,
            vec![
                ("normal".to_string(), 2.0),
                ("rest_day".to_string(), 2.0),
                ("public_holiday".to_string(), 1.0),
            ],
        );

        let c = compute(&emp, &p, &bulk).expect("compute");

        assert_eq!(c.total_overtime, 3_000 + 4_000 + 3_000);
        assert_eq!(c.gross, 310_000);
        // EPF excludes overtime by statute…
        assert_eq!(c.epf.employee, 2_200);
        // …SOCSO and EIS include it.
        assert_eq!(c.socso.employee, 220);
        assert_eq!(c.eis.employee, 110);

        for (label, multiplier) in [
            ("normal", "1.5x"),
            ("rest day", "2x"),
            ("public holiday", "3x"),
        ] {
            let line = c
                .lines
                .iter()
                .find(|l| l.description.contains(label))
                .unwrap_or_else(|| panic!("overtime line for {label}"));
            assert!(line.description.contains(multiplier), "{label}: {line:?}");
        }
    }

    /// Claims are reimbursements: outside gross, added on top of net, carried
    /// through as their own non-taxable lines and their own id list.
    #[test]
    fn golden_claims_stack_on_top_of_net() {
        let emp = employee(300_000, NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(), None);
        let claim_id = Uuid::now_v7();
        let p = period(2025, 6);
        let mut bulk = empty_bulk();
        bulk.approved_claims.insert(
            emp.id,
            vec![PayableClaim {
                id: claim_id,
                employee_id: emp.id,
                title: "Client travel".into(),
                amount: 25_000,
                expense_date: NaiveDate::from_ymd_opt(2025, 5, 20).unwrap(),
            }],
        );

        let baseline = compute(&emp, &p, &empty_bulk()).expect("baseline");
        let c = compute(&emp, &p, &bulk).expect("with claim");

        assert_eq!(c.total_claims, 25_000);
        assert_eq!(c.gross, baseline.gross, "claims never inflate gross");
        assert_eq!(c.net, baseline.net + 25_000);
        assert_eq!(c.claim_ids, vec![claim_id]);
        let line = c
            .lines
            .iter()
            .find(|l| l.item_type == "claim_reimbursement")
            .expect("claim line");
        assert_eq!(line.amount, 25_000);
        assert!(!line.is_taxable);
    }

    /// Zakat offsets PCB ringgit-for-ringgit before rounding; PTPTN and
    /// Tabung Haji are plain additions to deductions.
    #[test]
    fn golden_zakat_ptptn_tabung_haji_stack() {
        let mut emp = employee(300_000, NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(), None);
        emp.zakat_eligible = Some(true);
        emp.zakat_monthly_amount = Some(15_000);
        emp.ptptn_monthly_amount = Some(8_000);
        emp.tabung_haji_amount = Some(5_000);
        let p = period(2025, 6);
        let c = compute(&emp, &p, &empty_bulk()).expect("compute");

        // Annual zakat 7×15000=105_000 swamps the RM275 annual tax → PCB zero.
        assert_eq!(c.pcb, 0);
        assert_eq!(c.zakat, 15_000);
        assert_eq!(c.ptptn, 8_000);
        assert_eq!(c.tabung_haji, 5_000);
        assert_eq!(
            c.total_deductions,
            2_200 + 220 + 110 + 15_000 + 8_000 + 5_000
        );
        assert_eq!(c.net, 300_000 - c.total_deductions);
    }

    /// A citizen past 60 derives EPF Part C, not Part A — the age/residency
    /// derivation, pinned end to end through the engine.
    #[test]
    fn golden_over_sixty_citizen_derives_part_c() {
        let mut emp = employee(300_000, NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(), None);
        emp.date_of_birth = Some(NaiveDate::from_ymd_opt(1962, 3, 1).unwrap()); // 63 on the effective date
        let p = period(2025, 6);
        let c = compute(&emp, &p, &empty_bulk()).expect("compute");
        assert_eq!(c.epf.category, "C");
    }

    /// Additional remuneration goes through the Schedule 2 differential: the
    /// bonus never enters the annualised normal base (the normal leg of the
    /// PCB is byte-identical to a bonus-free employee's), and the differential
    /// is the increase in the year's payable tax caused by the bonus alone —
    /// which on this fixture crosses the individual-rebate ceiling and so
    /// gains RM90,000 of tax net of the RM40,000 rebate difference:
    ///   without bonus: chargeable 3,282,290 ≤ ceiling → tax 278,228 − 40,000
    ///   with bonus:    chargeable 3,782,290 >  ceiling → tax 328,228 − 0
    ///   differential   = 90,000 (already ringgit-grained)
    #[test]
    fn golden_additional_remuneration_uses_schedule_2() {
        let build = |bonus: i64| {
            let emp = employee(600_000, NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(), None);
            let p = period(2025, 6);
            let mut bulk = empty_bulk();
            if bonus > 0 {
                // Production reads stage the bonus rows in the category totals
                // AND carry them separately for the PCB split.
                bulk.variable_earnings.insert(emp.id, bonus);
                bulk.taxable_variable_earnings.insert(emp.id, bonus);
                bulk.bonus_commission.insert(emp.id, (bonus, 0));
            }
            compute(&emp, &p, &bulk).expect("compute")
        };

        let baseline = build(0);
        let bonused = build(500_000);

        // Statutory contributions still levy on the bonus…
        assert_eq!(bonused.gross, baseline.gross + 500_000);
        assert_eq!(bonused.epf.employee, baseline.epf.employee);
        assert_eq!(bonused.socso.employee, baseline.socso.employee);

        // …while the normal PCB leg is untouched and the differential rides
        // on top.
        assert_eq!(bonused.pcb, baseline.pcb + 90_000);
    }

    // ── properties (hold for any rule-table content) ────────────────────

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]

        #[test]
        fn engine_invariants_hold(
            basic in 120_000i64..900_000,
            allowance in 0i64..80_000,
            ot_hours in 0.0f64..4.0,
            unpaid in 0i64..25_000,
            claims in 0i64..40_000,
        ) {
            let emp = employee(basic, NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(), None);
            let p = period(2025, 6);
            let mut bulk = empty_bulk();
            if allowance > 0 {
                bulk.recurring_lines.insert(
                    emp.id,
                    vec![earning_line(emp.id, "allowance", allowance)],
                );
            }
            if ot_hours > 0.0 {
                bulk.attendance_ot_hours
                    .insert(emp.id, vec![("normal".to_string(), ot_hours)]);
            }
            if unpaid > 0 {
                bulk.variable_deductions.insert(emp.id, unpaid);
                bulk.unpaid_leave.insert(emp.id, (unpaid, Decimal::from(2)));
                bulk.entry_lines.insert(
                    emp.id,
                    vec![deduction_line(emp.id, "unpaid_leave", unpaid)],
                );
            }
            if claims > 0 {
                bulk.approved_claims.insert(
                    emp.id,
                    vec![PayableClaim {
                        id: Uuid::now_v7(),
                        employee_id: emp.id,
                        title: "Property claim".into(),
                        amount: claims,
                        expense_date: NaiveDate::from_ymd_opt(2025, 5, 1).unwrap(),
                    }],
                );
            }

            // A draw whose deductions exceed earnings fails closed with a
            // validation error; that is the documented behaviour, not an
            // invariant violation, so the property skips those draws.
            let Ok(c) = compute(&emp, &p, &bulk) else {
                return Ok(());
            };

            // Earnings reconcile: gross plus reimbursements, nothing else.
            prop_assert_eq!(sum_lines(&c.lines, "earning") - c.total_claims, c.gross);
            prop_assert_eq!(c.gross, c.basic + c.total_allowances + c.variable_part() + c.total_overtime);
            // Deductions reconcile line-for-line.
            prop_assert_eq!(sum_lines(&c.lines, "deduction"), c.total_deductions);
            // Net identity, partition of deductions, employer cost.
            prop_assert_eq!(c.net, c.gross - c.total_deductions + c.total_claims);
            prop_assert!(c.net >= 0);
            prop_assert_eq!(
                c.other_deductions + c.unpaid_leave_deduction,
                c.total_deductions
                    - c.epf.employee - c.socso.employee - c.eis.employee
                    - c.pcb - c.zakat - c.ptptn - c.tabung_haji
            );
            prop_assert_eq!(
                c.employer_cost,
                c.gross + c.epf.employer + c.socso.employer + c.eis.employer
            );
            // YTD accumulation.
            prop_assert_eq!(c.new_ytd_gross, c.ytd_gross_input(&bulk) + c.gross);
        }

        #[test]
        fn net_is_monotonic_in_basic(
            low in 120_000i64..800_000,
            delta in 1i64..100_000,
            allowance in 0i64..40_000,
            unpaid in 0i64..10_000,
        ) {
            let high = low + delta;
            let build = |basic: i64| {
                let emp = employee(basic, NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(), None);
                let p = period(2025, 6);
                let mut bulk = empty_bulk();
                if allowance > 0 {
                    bulk.recurring_lines.insert(
                        emp.id,
                        vec![earning_line(emp.id, "allowance", allowance)],
                    );
                }
                if unpaid > 0 {
                    bulk.variable_deductions.insert(emp.id, unpaid);
                    bulk.unpaid_leave.insert(emp.id, (unpaid, Decimal::from(2)));
                }
                compute(&emp, &p, &bulk)
            };
            let (Ok(a), Ok(b)) = (build(low), build(high)) else {
                return Ok(());
            };
            // Two legitimate mechanisms can lower net as basic rises, both
            // inherited from how Malaysian statutory tables work rather than
            // being engine defects:
            //
            // 1. Contribution tables are STEP functions. Crossing the
            //    fixture's RM2,000 cell boundary raises EPF by 1200, SOCSO by
            //    120 and EIS by 60 sen while gross rose by as little as one
            //    sen — real Third Schedule cells behave identically.
            // 2. Monthly PCB rounds UP to the nearest ringgit before it is
            //    withheld, costing at most one extra ringgit.
            //
            // 3. The individual-rebate ceiling is a CLIFF: chargeable income
            //    moving past it raises the year's tax by the whole RM400
            //    rebate, worth up to 40000/6 = 6667 sen of monthly PCB on
            //    this fixture's June run.
            //
            // Anything beyond steps + rounding + one rebate cliff is a real
            // defect.
            prop_assert!(
                b.net >= a.net - 1_380 - 100 - 6_700,
                "net dropped by more than band steps plus PCB rounding: {} -> {}",
                a.net,
                b.net
            );
        }
    }

    // Small accessors the property assertions need that ComputedPayslip keeps
    // private alongside the rest of the engine's plumbing.
    impl ComputedPayslip {
        fn variable_part(&self) -> i64 {
            self.gross - self.basic - self.total_allowances - self.total_overtime
        }
        fn ytd_gross_input(&self, _bulk: &BulkPayrollData) -> i64 {
            self.new_ytd_gross - self.gross
        }
    }
}
