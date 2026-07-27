use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PayrollGroup {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub cutoff_day: i32,
    pub payment_day: i32,
    pub is_active: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PayrollRun {
    pub id: Uuid,
    pub company_id: Uuid,
    pub payroll_group_id: Uuid,
    pub period_year: i32,
    pub period_month: i32,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub pay_date: NaiveDate,
    pub status: String,

    pub total_gross: i64,
    pub total_net: i64,
    pub total_employer_cost: i64,
    pub total_epf_employee: i64,
    pub total_epf_employer: i64,
    pub total_socso_employee: i64,
    pub total_socso_employer: i64,
    pub total_eis_employee: i64,
    pub total_eis_employer: i64,
    pub total_pcb: i64,
    pub total_zakat: i64,
    pub employee_count: i32,

    pub version: i32,

    pub processed_by: Option<Uuid>,
    pub processed_at: Option<DateTime<Utc>>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub locked_at: Option<DateTime<Utc>>,
    pub locked_by: Option<Uuid>,

    pub notes: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PayrollItem {
    pub id: Uuid,
    pub payroll_run_id: Uuid,
    pub employee_id: Uuid,

    pub basic_salary: i64,
    pub gross_salary: i64,
    pub total_allowances: i64,
    pub total_overtime: i64,
    pub total_bonus: i64,
    pub total_commission: i64,
    pub total_claims: i64,

    pub epf_employee: i64,
    pub epf_employer: i64,
    pub socso_employee: i64,
    pub socso_employer: i64,
    pub eis_employee: i64,
    pub eis_employer: i64,
    pub pcb_amount: i64,
    pub zakat_amount: i64,

    pub ptptn_amount: i64,
    pub tabung_haji_amount: i64,
    pub total_loan_deductions: i64,
    pub total_other_deductions: i64,
    pub unpaid_leave_deduction: i64,
    pub unpaid_leave_days: rust_decimal::Decimal,

    pub total_deductions: i64,
    pub net_salary: i64,
    pub employer_cost: i64,

    pub ytd_gross: i64,
    pub ytd_epf_employee: i64,
    pub ytd_pcb: i64,
    pub ytd_socso_employee: i64,
    pub ytd_eis_employee: i64,
    pub ytd_zakat: i64,
    pub ytd_net: i64,

    pub working_days: Option<i32>,
    pub days_worked: Option<rust_decimal::Decimal>,
    pub is_prorated: Option<bool>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PayrollItemDetail {
    pub id: Uuid,
    pub payroll_item_id: Uuid,
    pub category: String,
    pub item_type: String,
    pub description: String,
    pub amount: i64,
    pub is_taxable: Option<bool>,
    pub is_statutory: Option<bool>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PayrollEntry {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub company_id: Uuid,
    pub period_year: i32,
    pub period_month: i32,
    pub category: String,
    pub item_type: String,
    pub description: String,
    pub amount: i64,
    pub quantity: Option<rust_decimal::Decimal>,
    pub rate: Option<i64>,
    pub is_taxable: Option<bool>,
    pub is_processed: Option<bool>,
    pub payroll_run_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PayrollEntryWithEmployee {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub company_id: Uuid,
    pub period_year: i32,
    pub period_month: i32,
    pub category: String,
    pub item_type: String,
    pub description: String,
    pub amount: i64,
    pub quantity: Option<rust_decimal::Decimal>,
    pub rate: Option<i64>,
    pub is_taxable: Option<bool>,
    pub is_processed: Option<bool>,
    pub payroll_run_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub employee_name: Option<String>,
    pub employee_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePayrollEntryRequest {
    pub employee_id: Uuid,
    pub period_year: i32,
    pub period_month: i32,
    pub category: String,
    pub item_type: String,
    pub description: String,
    pub amount: i64,
    pub quantity: Option<rust_decimal::Decimal>,
    pub rate: Option<i64>,
    pub is_taxable: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePayrollEntryRequest {
    pub employee_id: Option<Uuid>,
    pub period_year: Option<i32>,
    pub period_month: Option<i32>,
    pub category: Option<String>,
    pub item_type: Option<String>,
    pub description: Option<String>,
    pub amount: Option<i64>,
    pub quantity: Option<rust_decimal::Decimal>,
    pub rate: Option<i64>,
    pub is_taxable: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePayrollPcbRequest {
    pub pcb_amount: i64,
}

#[derive(Debug, Deserialize)]
pub struct ProcessPayrollRequest {
    pub payroll_group_id: Uuid,
    pub period_year: i32,
    pub period_month: i32,
    pub pay_date: Option<NaiveDate>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReturnPayrollRunRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PayrollEntryQuery {
    pub period_year: Option<i32>,
    pub period_month: Option<i32>,
    pub employee_id: Option<Uuid>,
    pub item_type: Option<String>,
    pub include_processed: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct PayrollSummary {
    pub payroll_run: PayrollRun,
    pub items: Vec<PayrollItemSummary>,
    /// The statutory rule sets and overtime configuration this run was computed
    /// from. `None` for runs processed before provenance was recorded.
    pub calculation_snapshot: Option<serde_json::Value>,
}

/// One payslip with the stored lines that explain each of its figures.
#[derive(Debug, Serialize)]
pub struct PayslipBreakdown {
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_number: String,
    pub item: PayrollItem,
    pub lines: Vec<PayrollItemDetail>,
}

/// An approved, not-yet-paid claim a run will reimburse.
///
/// Selection is carry-forward (`expense_date <= period_end` AND
/// `payroll_run_id IS NULL`) rather than period-bounded, so a claim approved
/// after its own expense month closed is swept by the next run instead of
/// falling into a hole no run's window covers.
#[derive(Debug, Clone)]
pub struct PayableClaim {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub title: String,
    pub amount: i64,
    pub expense_date: NaiveDate,
}

#[derive(Debug)]
pub struct OrphanedEntryEmployee {
    pub employee_id: Uuid,
    pub employee_number: String,
    pub employee_name: String,
    pub entry_count: i64,
    pub total_amount: i64,
}

/// Closed attendance records in the period whose derived overtime was left
/// unrated, or predates the ceiling and still exceeds it.
///
/// Unrated hours are excluded from pay by construction (`SUM` skips NULL), so
/// the only way an employee learns a forgotten check-out cost them is if the
/// preview says so before the run commits.
#[derive(Debug)]
pub struct EmployeeUnratedOvertime {
    pub employee_id: Uuid,
    pub employee_number: String,
    pub employee_name: String,
    pub record_count: i64,
    pub max_hours_worked: Option<Decimal>,
}

/// A dry run of `process_payroll` — the same calculation, nothing written.
///
/// Processing used to be all-or-nothing and opaque: the operator picked a group
/// and a period and either got a committed run or a single error about the first
/// employee that failed, with no way to see who was included or what they would
/// be paid. Every employee is computed here, so all problems surface together.
#[derive(Debug, Serialize)]
pub struct PayrollPreview {
    pub payroll_group_id: Uuid,
    pub period_year: i32,
    pub period_month: i32,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub pay_date: NaiveDate,

    pub employee_count: i32,
    pub payable_count: i32,
    pub total_gross: i64,
    pub total_net: i64,
    pub total_employer_cost: i64,
    pub total_epf_employee: i64,
    pub total_epf_employer: i64,
    pub total_socso_employee: i64,
    pub total_socso_employer: i64,
    pub total_eis_employee: i64,
    pub total_eis_employer: i64,
    pub total_pcb: i64,
    pub total_zakat: i64,

    /// Whether `process_payroll` would succeed as things stand.
    pub can_process: bool,
    /// Problems that stop the run.
    pub blocking: Vec<PayrollDiagnostic>,
    /// Problems worth reviewing that do not stop the run.
    pub warnings: Vec<PayrollDiagnostic>,
    pub employees: Vec<PayrollPreviewEmployee>,
}

#[derive(Debug, Serialize)]
pub struct PayrollDiagnostic {
    /// Stable machine-readable identifier, so the UI can group or link without
    /// matching on prose.
    pub code: String,
    pub message: String,
    pub employee_id: Option<Uuid>,
    pub employee_number: Option<String>,
    pub employee_name: Option<String>,
}

impl PayrollDiagnostic {
    pub fn run(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            employee_id: None,
            employee_number: None,
            employee_name: None,
        }
    }

    pub fn for_employee(
        code: &str,
        message: impl Into<String>,
        employee_id: Uuid,
        employee_number: impl Into<String>,
        employee_name: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            employee_id: Some(employee_id),
            employee_number: Some(employee_number.into()),
            employee_name: Some(employee_name.into()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PayrollPreviewEmployee {
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_number: String,
    pub basic_salary: i64,
    pub total_allowances: i64,
    pub total_overtime: i64,
    pub total_claims: i64,
    pub gross_salary: i64,
    pub epf_employee: i64,
    pub socso_employee: i64,
    pub eis_employee: i64,
    pub pcb_amount: i64,
    pub total_deductions: i64,
    pub net_salary: i64,
    pub employer_cost: i64,
    pub working_days: i32,
    pub days_worked: i64,
    pub is_prorated: bool,
    /// Why this employee could not be computed, if they could not be. The
    /// remaining figures are zero when set.
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PayrollItemSummary {
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_number: String,
    pub basic_salary: i64,
    pub total_allowances: i64,
    pub total_overtime: i64,
    pub total_claims: i64,
    pub gross_salary: i64,
    pub total_deductions: i64,
    pub net_salary: i64,
    pub epf_employee: i64,
    pub socso_employee: i64,
    pub eis_employee: i64,
    pub pcb_amount: i64,
}

/// Year-to-date `(gross, pcb, epf, socso, eis, zakat, net)` from prior committed
/// runs in the same tax year.
pub(crate) type YtdTotals = (i64, i64, i64, i64, i64, i64, i64);

/// Prior-employer `(income, epf, pcb, socso, zakat)` declared on a TP3 form.
pub(crate) type Tp3Totals = (i64, i64, i64, i64, i64);

pub(crate) struct BulkPayrollData {
    /// Every configured recurring allowance/deduction line overlapping the
    /// period. There is deliberately no pre-summed total beside them: each line
    /// is prorated against its own effective window, so the engine derives the
    /// totals from these rather than from a second query that could disagree.
    pub(crate) recurring_lines: HashMap<Uuid, Vec<PayslipSourceLine>>,
    /// Individual staged entries behind `variable_earnings`/`variable_deductions`.
    pub(crate) entry_lines: HashMap<Uuid, Vec<PayslipSourceLine>>,
    pub(crate) variable_earnings: HashMap<Uuid, i64>,
    /// `variable_earnings` narrowed to the rows flagged taxable, for the PCB base.
    pub(crate) taxable_variable_earnings: HashMap<Uuid, i64>,
    pub(crate) variable_deductions: HashMap<Uuid, i64>,
    /// Staged unpaid-leave deductions as `(amount, working_days)`. Already inside
    /// `variable_deductions`; carried separately so the payslip can report them
    /// under their own name instead of anonymously inside other deductions.
    pub(crate) unpaid_leave: HashMap<Uuid, (i64, Decimal)>,
    pub(crate) attendance_ot_hours: HashMap<Uuid, f64>,
    pub(crate) approved_ot: HashMap<Uuid, Vec<(String, f64)>>,
    /// The individual claims this run will reimburse, not a per-employee sum.
    /// The engine used to SUM inside its transaction and then re-run the same
    /// predicate to mark them paid, so a claim approved between the two was
    /// marked paid without being paid. Marking exactly the rows that were summed
    /// makes that unrepresentable, and the payslip can name each expense.
    pub(crate) approved_claims: HashMap<Uuid, Vec<PayableClaim>>,
    pub(crate) tp3: HashMap<Uuid, Tp3Totals>,
    pub(crate) ytd: HashMap<Uuid, YtdTotals>,
    pub(crate) monthly_allowances: HashMap<Uuid, i64>,
    pub(crate) bonus_commission: HashMap<Uuid, (i64, i64)>,
    pub(crate) ot_settings: OvertimeSettings,
}

/// Round a money figure to whole sen, half away from zero.
///
/// The one rounding rule for every derived amount in the engine and in the
/// approval paths that stage money for it. It was a closure inside
/// `compute_payslip`, so the approval path rounded a different way (it did not
/// round at all — it truncated, four times) and quoted an employee less than the
/// run would pay them.
pub(crate) fn round_sen(amount: Decimal) -> i64 {
    use rust_decimal::RoundingStrategy;
    use rust_decimal::prelude::ToPrimitive;

    amount
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
        .unwrap_or(0)
}

/// One overtime figure: what it is paid at, and what it comes to.
///
/// `amount_sen` is derived from the UNROUNDED hourly rate. `rate_sen` exists
/// only to be displayed and stored on `payroll_entries.rate`; deriving the
/// amount from it instead is what made the approval quote diverge from the run.
pub(crate) struct OvertimeRating {
    pub(crate) rate_sen: i64,
    pub(crate) amount_sen: i64,
}

/// Company overtime configuration, read once per run.
///
/// The engine previously hardcoded 1.5/2.0/3.0 and `basic / 26 / 8` while the
/// approval path parsed the same settings as `i64`/`f64` and truncated at four
/// separate points, so the amount quoted on approval was lower than the amount
/// the run paid and an `effective_hours_per_day` of `"7.5"` was silently read as
/// 8. Both paths now call `rate_overtime` below — that shared function, not a
/// comment, is what makes the two agree.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct OvertimeSettings {
    pub(crate) effective_hours_per_day: Decimal,
    pub(crate) working_days_per_month: Decimal,
    pub(crate) multiplier_normal: Decimal,
    pub(crate) multiplier_rest_day: Decimal,
    pub(crate) multiplier_public_holiday: Decimal,
    /// Above this, a check-out's derived overtime is left unrated (NULL) for HR
    /// review instead of paid. Not a statutory figure: it is an anomaly
    /// threshold separating a plausible long day from a forgotten check-out,
    /// which the 24h check-out match window can otherwise turn into a whole
    /// day of overtime. Companies running genuine long shifts raise it.
    pub(crate) max_overtime_hours_per_day: Decimal,
}

impl OvertimeSettings {
    /// Employment Act multipliers, a 26-day month and an 8-hour day — what a
    /// company that has configured nothing is rated on.
    pub(crate) fn statutory_defaults() -> Self {
        Self {
            effective_hours_per_day: Decimal::from(8),
            working_days_per_month: Decimal::from(26),
            multiplier_normal: Decimal::new(15, 1),
            multiplier_rest_day: Decimal::from(2),
            multiplier_public_holiday: Decimal::from(3),
            max_overtime_hours_per_day: Decimal::from(4),
        }
    }

    pub(crate) fn multiplier_for(&self, ot_type: &str) -> Decimal {
        match ot_type {
            "rest_day" => self.multiplier_rest_day,
            "public_holiday" => self.multiplier_public_holiday,
            _ => self.multiplier_normal,
        }
    }

    /// The employee's ordinary hourly rate, unrounded.
    ///
    /// An explicit `hourly_rate` on the employee record wins; otherwise it is
    /// derived from the basic salary and the company's divisors. Kept unrounded
    /// so the multiplier and the hours apply to the exact figure and only the
    /// final amount is rounded once.
    ///
    /// A zero divisor returns zero rather than dividing: `rust_decimal` panics on
    /// division by zero, and while `settings_service` filters zeros out on read,
    /// a guard here makes the panic unrepresentable instead of merely unreachable.
    pub(crate) fn hourly_rate(&self, hourly_rate: Option<i64>, basic_salary: i64) -> Decimal {
        if let Some(rate) = hourly_rate {
            return Decimal::from(rate);
        }
        if self.working_days_per_month.is_zero() || self.effective_hours_per_day.is_zero() {
            return Decimal::ZERO;
        }
        Decimal::from(basic_salary) / self.working_days_per_month / self.effective_hours_per_day
    }

    /// Rate one overtime claim. The single authority both the payroll engine and
    /// overtime approval price from, so a quoted amount and a paid amount cannot
    /// disagree.
    pub(crate) fn rate_overtime(
        &self,
        hourly_rate: Option<i64>,
        basic_salary: i64,
        ot_type: &str,
        hours: Decimal,
    ) -> OvertimeRating {
        let base = self.hourly_rate(hourly_rate, basic_salary);
        let multiplier = self.multiplier_for(ot_type);
        OvertimeRating {
            rate_sen: round_sen(base * multiplier),
            amount_sen: round_sen(base * multiplier * hours),
        }
    }
}

/// One line of a payslip's stored breakdown, ready to persist.
///
/// The engine builds these while it computes a payslip so `payroll_item_details`
/// records *why* each figure is what it is. Before this the table was never
/// written, so a payslip's `total_allowances` or `total_overtime` could not be
/// explained after the fact — the staged entries behind them are mutable and the
/// statutory rates are effective-dated.
#[derive(Debug, Clone)]
pub struct PayslipLine {
    pub category: String,
    pub item_type: String,
    pub description: String,
    pub amount: i64,
    pub is_taxable: bool,
    pub is_statutory: bool,
}

impl PayslipLine {
    pub fn earning(item_type: &str, description: impl Into<String>, amount: i64) -> Self {
        Self {
            category: "earning".into(),
            item_type: item_type.into(),
            description: description.into(),
            amount,
            is_taxable: true,
            is_statutory: false,
        }
    }

    pub fn deduction(item_type: &str, description: impl Into<String>, amount: i64) -> Self {
        Self {
            category: "deduction".into(),
            item_type: item_type.into(),
            description: description.into(),
            amount,
            is_taxable: false,
            is_statutory: false,
        }
    }

    pub fn taxable(mut self, is_taxable: bool) -> Self {
        self.is_taxable = is_taxable;
        self
    }

    pub fn statutory(mut self) -> Self {
        self.is_statutory = true;
        self
    }
}

/// One contributing line behind an employee's payslip figures, as read from its
/// source table (a recurring allowance or a staged payroll entry).
///
/// The effective window is the allowance's own, and is what lets the engine
/// prorate a line that only covers part of the period. Both are `None` for a
/// staged entry, which is keyed by period and never prorated.
#[derive(Debug, Clone)]
pub struct PayslipSourceLine {
    pub employee_id: Uuid,
    pub category: String,
    /// What the stored breakdown line is labelled. Real for a staged entry;
    /// derived from the category for a recurring allowance, which has no such
    /// column of its own.
    pub item_type: String,
    pub description: String,
    pub amount: i64,
    pub is_taxable: bool,
    pub effective_from: Option<NaiveDate>,
    pub effective_to: Option<NaiveDate>,
}

#[derive(Debug)]
pub struct EmployeeCategoryTotal {
    pub employee_id: Uuid,
    pub category: String,
    pub total: i64,
    /// `total` narrowed to the rows flagged taxable. Feeds the PCB base only —
    /// `is_taxable` is an income-tax flag, and EPF/SOCSO/EIS have their own,
    /// different statutory exclusion lists.
    pub taxable: i64,
}

/// Staged unpaid-leave deduction for one employee: amount and working days.
#[derive(Debug)]
pub struct EmployeeUnpaidLeave {
    pub employee_id: Uuid,
    pub amount: i64,
    pub days: Decimal,
}

/// An employee already paid by a run in a later period than the one being
/// created, with the earliest such period as `YYYYMM`.
#[derive(Debug)]
pub struct LaterCommittedRun {
    pub employee_id: Uuid,
    pub employee_number: String,
    pub employee_name: String,
    pub earliest_later_period: i32,
}

#[derive(Debug)]
pub struct EmployeeBonusCommission {
    pub employee_id: Uuid,
    pub bonus: i64,
    pub commission: i64,
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

#[derive(Debug)]
pub struct RunStatusRow {
    pub status: String,
    pub period_year: i32,
    pub period_month: i32,
}

#[derive(Debug)]
pub struct PcbFields {
    /// The `payroll_items` row id. Carried so a PCB edit can also replace the
    /// stored `payroll_item_details` PCB line, which is keyed by item rather
    /// than by `(run, employee)`.
    pub id: Uuid,
    pub pcb_amount: i64,
    pub total_deductions: i64,
    pub net_salary: i64,
    pub ytd_pcb: i64,
}
