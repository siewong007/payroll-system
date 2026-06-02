use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct YearQuery {
    pub year: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct YearMonthQuery {
    pub year: Option<i32>,
    pub month: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct DateRangeQuery {
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub struct EaFormQuery {
    pub year: Option<i32>,
    pub employee_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct YearMonthsOption {
    pub year: i32,
    pub months: Vec<i32>,
}

#[derive(Debug, Serialize)]
pub struct ReportPeriodsResponse {
    pub default_year: i32,
    pub default_month: i32,
    pub payroll_years: Vec<i32>,
    pub payroll_months: Vec<YearMonthsOption>,
    pub leave_years: Vec<i32>,
    pub claims_years: Vec<i32>,
    pub ea_form_years: Vec<i32>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PayrollSummaryRow {
    pub period: String,
    pub employee_count: i32,
    pub total_gross: i64,
    pub total_net: i64,
    pub total_epf_employee: i64,
    pub total_epf_employer: i64,
    pub total_socso_employee: i64,
    pub total_socso_employer: i64,
    pub total_eis_employee: i64,
    pub total_eis_employer: i64,
    pub total_pcb: i64,
    pub total_zakat: i64,
    pub total_employer_cost: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DepartmentPayrollRow {
    pub department: Option<String>,
    pub employee_count: i64,
    pub total_gross: i64,
    pub total_net: i64,
    pub total_employer_cost: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LeaveReportRow {
    pub employee_name: String,
    pub employee_number: String,
    pub department: Option<String>,
    pub gender: Option<String>,
    pub marital_status: Option<String>,
    pub num_children: Option<i32>,
    pub leave_type_name: String,
    pub entitled_days: rust_decimal::Decimal,
    pub taken_days: rust_decimal::Decimal,
    pub pending_days: rust_decimal::Decimal,
    pub balance: rust_decimal::Decimal,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ClaimsReportRow {
    pub employee_name: String,
    pub employee_number: String,
    pub department: Option<String>,
    pub total_claims: i64,
    pub total_amount: i64,
    pub approved_count: i64,
    pub approved_amount: i64,
    pub pending_count: i64,
    pub pending_amount: i64,
    pub rejected_count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct StatutoryReportRow {
    pub employee_name: String,
    pub employee_number: String,
    pub ic_number: Option<String>,
    pub epf_number: Option<String>,
    pub socso_number: Option<String>,
    pub basic_salary: i64,
    pub gross_salary: i64,
    pub epf_employee: i64,
    pub epf_employer: i64,
    pub socso_employee: i64,
    pub socso_employer: i64,
    pub eis_employee: i64,
    pub eis_employer: i64,
    pub pcb_amount: i64,
    pub zakat_amount: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct PayrollPeriodRow {
    pub period_year: i32,
    pub period_month: i32,
}
