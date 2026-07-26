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

#[derive(Debug)]
pub struct OrphanedEntryEmployee {
    pub employee_id: Uuid,
    pub employee_number: String,
    pub employee_name: String,
    pub entry_count: i64,
    pub total_amount: i64,
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
    pub(crate) recurring_allowances: HashMap<Uuid, i64>,
    pub(crate) recurring_deductions: HashMap<Uuid, i64>,
    /// Individual lines behind the two totals above, for the payslip breakdown.
    pub(crate) recurring_lines: HashMap<Uuid, Vec<PayslipSourceLine>>,
    /// Individual staged entries behind `variable_earnings`/`variable_deductions`.
    pub(crate) entry_lines: HashMap<Uuid, Vec<PayslipSourceLine>>,
    pub(crate) variable_earnings: HashMap<Uuid, i64>,
    pub(crate) variable_deductions: HashMap<Uuid, i64>,
    pub(crate) attendance_ot_hours: HashMap<Uuid, f64>,
    pub(crate) approved_ot: HashMap<Uuid, Vec<(String, f64)>>,
    pub(crate) approved_claims: HashMap<Uuid, i64>,
    pub(crate) tp3: HashMap<Uuid, Tp3Totals>,
    pub(crate) ytd: HashMap<Uuid, YtdTotals>,
    pub(crate) monthly_allowances: HashMap<Uuid, i64>,
    pub(crate) bonus_commission: HashMap<Uuid, (i64, i64)>,
    pub(crate) ot_settings: OvertimeSettings,
}

/// Company overtime configuration, read once per run.
///
/// The engine previously hardcoded 1.5/2.0/3.0 and `basic / 26 / 8` while the
/// approval path read these same settings, so a company configured for (say) a
/// 9-hour day or a 2.5x holiday rate had its attendance overtime paid at the
/// wrong rate. Decimal throughout, per the repo's money rule — the old `f64`
/// cast and double integer division truncated, always against the employee.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct OvertimeSettings {
    pub(crate) effective_hours_per_day: Decimal,
    pub(crate) working_days_per_month: Decimal,
    pub(crate) multiplier_normal: Decimal,
    pub(crate) multiplier_rest_day: Decimal,
    pub(crate) multiplier_public_holiday: Decimal,
}

impl OvertimeSettings {
    pub(crate) fn multiplier_for(&self, ot_type: &str) -> Decimal {
        match ot_type {
            "rest_day" => self.multiplier_rest_day,
            "public_holiday" => self.multiplier_public_holiday,
            _ => self.multiplier_normal,
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
#[derive(Debug, Clone)]
pub struct PayslipSourceLine {
    pub employee_id: Uuid,
    pub category: String,
    pub description: String,
    pub amount: i64,
    pub is_taxable: bool,
}

#[derive(Debug)]
pub struct EmployeeCategoryTotal {
    pub employee_id: Uuid,
    pub category: String,
    pub total: i64,
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
    pub pcb_amount: i64,
    pub total_deductions: i64,
    pub net_salary: i64,
    pub ytd_pcb: i64,
}
