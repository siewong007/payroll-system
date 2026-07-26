use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;
use rust_decimal::prelude::ToPrimitive;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{Instrument, info, info_span};
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::employee::Employee;
use crate::models::payroll::{
    BulkPayrollData, OvertimeSettings, PayableClaim, PayrollDiagnostic, PayrollItem,
    PayrollPreview, PayrollPreviewEmployee, PayrollRun, PayslipLine, Tp3Totals, YtdTotals,
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
    other_deductions: i64,
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

/// Fallback when a company has no default work schedule to read a timezone from.
const DEFAULT_TIMEZONE: &str = "Asia/Kuala_Lumpur";

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
/// Executor-generic over a connection so `process_payroll` can gather inside its
/// transaction (unchanged from before) while `preview_payroll` gathers on a
/// pooled connection without opening one.
async fn gather_run_inputs(
    conn: &mut sqlx::PgConnection,
    pool: &PgPool,
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

    let tz = company_work_schedules::find_default_timezone(&mut *conn, company_id)
        .await?
        .unwrap_or_else(|| DEFAULT_TIMEZONE.to_string());

    // 1. Batch fetch recurring allowances and deductions
    let mut recurring_allowances_map = HashMap::new();
    let mut recurring_deductions_map = HashMap::new();
    for row in
        payroll_reads::recurring_allowance_totals(&mut *conn, &employee_ids, effective_date).await?
    {
        if row.category == "earning" {
            recurring_allowances_map.insert(row.employee_id, row.total);
        } else {
            recurring_deductions_map.insert(row.employee_id, row.total);
        }
    }

    // 1b. The individual lines behind those totals, for the stored payslip
    // breakdown. Same filters as the totals above, so the two reconcile.
    let mut recurring_lines_map: HashMap<Uuid, Vec<_>> = HashMap::new();
    for line in
        payroll_reads::recurring_allowance_lines(&mut *conn, &employee_ids, effective_date).await?
    {
        recurring_lines_map
            .entry(line.employee_id)
            .or_default()
            .push(line);
    }

    // 2. Batch fetch staged payroll entries
    let mut variable_earnings_map = HashMap::new();
    let mut variable_deductions_map = HashMap::new();
    for row in payroll_reads::entry_category_totals(&mut *conn, &employee_ids, year, month).await? {
        if row.category == "earning" {
            variable_earnings_map.insert(row.employee_id, row.total);
        } else {
            variable_deductions_map.insert(row.employee_id, row.total);
        }
    }

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

    // 3. Batch fetch attendance OT hours
    let attendance_ot_map: HashMap<Uuid, f64> = payroll_reads::attendance_ot_hours(
        &mut *conn,
        &employee_ids,
        period_start,
        period_end,
        &tz,
    )
    .await?
    .into_iter()
    .map(|r| (r.employee_id, r.hours))
    .collect();

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
    let statutory = StatutoryTables::load(pool, effective_date).await?;
    let ot_settings = settings_service::overtime_settings(pool, company_id).await;

    Ok(RunInputs {
        employees,
        bulk: BulkPayrollData {
            recurring_allowances: recurring_allowances_map,
            recurring_deductions: recurring_deductions_map,
            recurring_lines: recurring_lines_map,
            entry_lines: entry_lines_map,
            variable_earnings: variable_earnings_map,
            variable_deductions: variable_deductions_map,
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
    let inputs = gather_run_inputs(&mut conn, pool, company_id, payroll_group_id, &period).await?;

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
        warnings.push(PayrollDiagnostic::for_employee(
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
        ));
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
        warnings.push(PayrollDiagnostic::for_employee(
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
        ));
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
        warnings.push(PayrollDiagnostic::for_employee(
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
        ));
    }

    // Overtime the check-out path refused to rate is silently absent from this
    // run's figures — the employee simply sees a smaller payslip. A warning
    // rather than a blocker: nothing unworked is being paid, so the run is safe
    // to commit; what is owed is an HR correction to the affected records.
    let ceiling = inputs.ot_settings.max_overtime_hours_per_day;
    let employee_ids: Vec<Uuid> = inputs.employees.iter().map(|emp| emp.id).collect();
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
        warnings.push(PayrollDiagnostic::for_employee(
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
        ));
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

    let inputs = gather_run_inputs(&mut tx, pool, company_id, payroll_group_id, &period).await?;
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

    let mut totals = RunTotals::default();
    for (emp, payslip) in inputs.employees.iter().zip(&computed) {
        let emp_span = info_span!("payroll.employee", employee_id = %emp.id);
        let item = persist_payslip(&mut tx, run_id, emp, &period, payslip)
            .instrument(emp_span)
            .await?;
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
    // settings change leaves the run's numbers unreproducible.
    payroll_runs::set_calculation_snapshot(
        &mut *tx,
        run_id,
        serde_json::json!({
            "effective_date": effective_date,
            "statutory_rule_sets": statutory.rule_sets(),
            "overtime_settings": ot_settings,
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
    let epf_category = emp.epf_category.clone().unwrap_or_else(|| "A".to_string());

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
        (Decimal::from(emp.basic_salary) * Decimal::from(days_worked) / Decimal::from(period_days))
            .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
            .to_i64()
            .unwrap_or(emp.basic_salary)
    } else {
        emp.basic_salary
    };

    let allowances_total = *bulk.recurring_allowances.get(&emp.id).unwrap_or(&0);
    let monthly_allowances = *bulk.monthly_allowances.get(&emp.id).unwrap_or(&0);
    let variable_earnings = *bulk.variable_earnings.get(&emp.id).unwrap_or(&0);
    let (total_bonus, total_commission) = *bulk.bonus_commission.get(&emp.id).unwrap_or(&(0, 0));
    let variable_deductions = *bulk.variable_deductions.get(&emp.id).unwrap_or(&0);
    let recurring_deductions = *bulk.recurring_deductions.get(&emp.id).unwrap_or(&0);
    let attendance_ot_hours = *bulk.attendance_ot_hours.get(&emp.id).unwrap_or(&0.0);

    // Hourly rate in Decimal, from company settings rather than hardcoded 26/8.
    // The old `basic / 26 / 8` truncated twice (RM5,000 gave 2403 sen instead of
    // 2403.85), and the f64 multiply below truncated again â€” always against the
    // employee. Keep the rate unrounded and round only the final amount.
    let ot = &bulk.ot_settings;
    let hourly_rate = match emp.hourly_rate {
        Some(rate) => Decimal::from(rate),
        None => {
            Decimal::from(emp.basic_salary) / ot.working_days_per_month / ot.effective_hours_per_day
        }
    };

    let round_sen = |amount: Decimal| -> i64 {
        amount
            .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
            .to_i64()
            .unwrap_or(0)
    };

    // Overtime lines are built alongside the figures: an OT amount is a product
    // of hours, an hourly rate derived from company settings, and a type
    // multiplier, none of which survive in `total_overtime` alone.
    let mut overtime_lines: Vec<PayslipLine> = Vec::new();

    // Attendance-based OT (records without approved OT applications)
    let attendance_ot_pay = if attendance_ot_hours > 0.0 {
        let hours = Decimal::try_from(attendance_ot_hours).unwrap_or_default();
        let amount = round_sen(hourly_rate * ot.multiplier_normal * hours);
        overtime_lines.push(PayslipLine::earning(
            "overtime",
            format!(
                "Overtime (attendance) â€” {} h @ {}x",
                trim_decimal(hours),
                trim_decimal(ot.multiplier_normal)
            ),
            amount,
        ));
        amount
    } else {
        0
    };

    // Approved OT applications with type-based rate multipliers
    let approved_ot_pay = if let Some(ot_entries) = bulk.approved_ot.get(&emp.id) {
        let mut total = 0i64;
        for (ot_type, hours) in ot_entries {
            let hours = Decimal::try_from(*hours).unwrap_or_default();
            let multiplier = ot.multiplier_for(ot_type);
            let amount = round_sen(hourly_rate * multiplier * hours);
            overtime_lines.push(PayslipLine::earning(
                "overtime",
                format!(
                    "Overtime ({}) â€” {} h @ {}x",
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
    let total_allowances = allowances_total + monthly_allowances;

    // EPF / SOCSO / EIS â€” resolved from the run's rule snapshot, no I/O.
    let epf = epf_service::calculate_epf_with(statutory, epf_wage, &epf_category)?;
    let socso = socso_service::calculate_socso_with(statutory, gross, age, is_foreigner)?;
    let eis = eis_service::calculate_eis_with(statutory, gross, age, is_foreigner)?;

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
    let pcb_input = PcbInput {
        monthly_gross: gross,
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
        is_bonus_month: false,
        bonus_amount: 0,
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
        other_deductions: recurring_deductions + variable_deductions,
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
            "Basic salary (prorated â€” {} of {} days)",
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

    let source_lines = bulk
        .recurring_lines
        .get(&emp.id)
        .into_iter()
        .flatten()
        .chain(bulk.entry_lines.get(&emp.id).into_iter().flatten());

    let (mut earnings, mut deductions) = (Vec::new(), Vec::new());
    for line in source_lines {
        if line.category == "earning" {
            earnings.push(
                PayslipLine::earning("allowance", line.description.clone(), line.amount)
                    .taxable(line.is_taxable),
            );
        } else {
            deductions.push(PayslipLine::deduction(
                "other_deduction",
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
