use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub format_version: String,
    pub exported_at: DateTime<Utc>,
    pub source_company_id: Uuid,
    pub source_company_name: String,
    pub record_counts: HashMap<String, usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompanyBackup {
    pub metadata: BackupMetadata,
    pub company: CompanyExport,
    pub payroll_groups: Vec<PayrollGroupExport>,
    pub employees: Vec<EmployeeExport>,
    pub employee_allowances: Vec<EmployeeAllowanceExport>,
    pub salary_history: Vec<SalaryHistoryExport>,
    pub tp3_records: Vec<Tp3RecordExport>,
    pub leave_types: Vec<LeaveTypeExport>,
    pub leave_balances: Vec<LeaveBalanceExport>,
    pub leave_requests: Vec<LeaveRequestExport>,
    pub claims: Vec<ClaimExport>,
    pub overtime_applications: Vec<OvertimeExport>,
    pub payroll_runs: Vec<PayrollRunExport>,
    pub payroll_items: Vec<PayrollItemExport>,
    pub payroll_item_details: Vec<PayrollItemDetailExport>,
    pub payroll_entries: Vec<PayrollEntryExport>,
    pub document_categories: Vec<DocumentCategoryExport>,
    pub documents: Vec<DocumentExport>,
    pub teams: Vec<TeamExport>,
    pub team_members: Vec<TeamMemberExport>,
    pub holidays: Vec<HolidayExport>,
    pub working_day_config: Vec<WorkingDayConfigExport>,
    pub email_templates: Vec<EmailTemplateExport>,
    pub company_settings: Vec<CompanySettingExport>,
    #[serde(default)]
    pub files: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub new_company_id: Uuid,
    pub new_company_name: String,
    pub is_overwrite: bool,
    pub records_imported: HashMap<String, usize>,
    pub warnings: Vec<String>,
}

// --- Export structs (flat, no joined fields) ---

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompanyExport {
    pub id: Uuid,
    pub name: String,
    pub registration_number: Option<String>,
    pub tax_number: Option<String>,
    pub epf_number: Option<String>,
    pub socso_code: Option<String>,
    pub eis_code: Option<String>,
    pub hrdf_number: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postcode: Option<String>,
    pub country: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub logo_url: Option<String>,
    pub hrdf_enabled: Option<bool>,
    pub unpaid_leave_divisor: Option<i32>,
    pub is_active: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PayrollGroupExport {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub cutoff_day: i32,
    pub payment_day: i32,
    pub is_active: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmployeeExport {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_number: String,
    pub full_name: String,
    pub ic_number: Option<String>,
    pub passport_number: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub gender: Option<String>,
    pub nationality: Option<String>,
    pub race: Option<String>,
    pub residency_status: String,
    pub marital_status: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postcode: Option<String>,
    pub department: Option<String>,
    pub designation: Option<String>,
    pub cost_centre: Option<String>,
    pub branch: Option<String>,
    pub employment_type: String,
    pub date_joined: NaiveDate,
    pub probation_start: Option<NaiveDate>,
    pub probation_end: Option<NaiveDate>,
    pub confirmation_date: Option<NaiveDate>,
    pub date_resigned: Option<NaiveDate>,
    pub resignation_reason: Option<String>,
    pub basic_salary: i64,
    pub hourly_rate: Option<i64>,
    pub daily_rate: Option<i64>,
    pub bank_name: Option<String>,
    pub bank_account_number: Option<String>,
    pub bank_account_type: Option<String>,
    pub tax_identification_number: Option<String>,
    pub epf_number: Option<String>,
    pub socso_number: Option<String>,
    pub eis_number: Option<String>,
    pub working_spouse: Option<bool>,
    pub num_children: Option<i32>,
    pub epf_category: Option<String>,
    pub is_muslim: Option<bool>,
    pub zakat_eligible: Option<bool>,
    pub zakat_monthly_amount: Option<i64>,
    pub ptptn_monthly_amount: Option<i64>,
    pub tabung_haji_amount: Option<i64>,
    pub hrdf_contribution: Option<bool>,
    pub payroll_group_id: Option<Uuid>,
    pub salary_group: Option<String>,
    pub is_active: Option<bool>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmployeeAllowanceExport {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub category: String,
    pub name: String,
    pub description: Option<String>,
    pub amount: i64,
    pub is_taxable: Option<bool>,
    pub is_recurring: Option<bool>,
    pub effective_from: NaiveDate,
    pub effective_to: Option<NaiveDate>,
    pub is_active: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct SalaryHistoryExport {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub old_salary: i64,
    pub new_salary: i64,
    pub effective_date: NaiveDate,
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Tp3RecordExport {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub tax_year: i32,
    pub previous_employer_name: Option<String>,
    pub previous_income_ytd: i64,
    pub previous_epf_ytd: i64,
    pub previous_pcb_ytd: i64,
    pub previous_socso_ytd: i64,
    pub previous_zakat_ytd: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LeaveTypeExport {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub default_days: rust_decimal::Decimal,
    pub is_paid: bool,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LeaveBalanceExport {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub leave_type_id: Uuid,
    pub year: i32,
    pub entitled_days: rust_decimal::Decimal,
    pub taken_days: rust_decimal::Decimal,
    pub pending_days: rust_decimal::Decimal,
    pub carried_forward: rust_decimal::Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LeaveRequestExport {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub company_id: Uuid,
    pub leave_type_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub days: rust_decimal::Decimal,
    pub reason: Option<String>,
    pub status: String,
    pub review_notes: Option<String>,
    pub attachment_url: Option<String>,
    pub attachment_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ClaimExport {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub company_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub amount: i64,
    pub category: Option<String>,
    pub receipt_url: Option<String>,
    pub receipt_file_name: Option<String>,
    pub expense_date: NaiveDate,
    pub status: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub review_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct OvertimeExport {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub company_id: Uuid,
    pub ot_date: NaiveDate,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub hours: rust_decimal::Decimal,
    pub ot_type: String,
    pub reason: Option<String>,
    pub status: String,
    pub review_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PayrollRunExport {
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
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PayrollItemExport {
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

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PayrollItemDetailExport {
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

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PayrollEntryExport {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub company_id: Uuid,
    pub period_year: i32,
    pub period_month: i32,
    pub category: String,
    pub item_type: String,
    pub description: Option<String>,
    pub amount: i64,
    pub quantity: Option<rust_decimal::Decimal>,
    pub rate: Option<i64>,
    pub is_taxable: Option<bool>,
    pub is_processed: Option<bool>,
    pub payroll_run_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct DocumentCategoryExport {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_active: Option<bool>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct DocumentExport {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub file_name: String,
    pub file_url: String,
    pub file_size: Option<i64>,
    pub mime_type: Option<String>,
    pub status: String,
    pub issue_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub is_confidential: Option<bool>,
    pub tags: Option<String>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TeamExport {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub tag: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TeamMemberExport {
    pub id: Uuid,
    pub team_id: Uuid,
    pub employee_id: Uuid,
    pub role: String,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct HolidayExport {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub date: NaiveDate,
    pub holiday_type: String,
    pub description: Option<String>,
    pub is_recurring: bool,
    pub state: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkingDayConfigExport {
    pub id: Uuid,
    pub company_id: Uuid,
    pub day_of_week: i16,
    pub is_working_day: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailTemplateExport {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub letter_type: String,
    pub subject: String,
    pub body_html: String,
    pub is_active: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompanySettingExport {
    pub id: Uuid,
    pub company_id: Uuid,
    pub category: String,
    pub key: String,
    pub value: serde_json::Value,
    pub label: Option<String>,
    pub description: Option<String>,
    pub updated_at: DateTime<Utc>,
}
