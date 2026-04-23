use chrono::{DateTime, NaiveDate, Utc};
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

#[derive(Debug, Serialize)]
pub struct PayrollSummary {
    pub payroll_run: PayrollRun,
    pub items: Vec<PayrollItemSummary>,
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
