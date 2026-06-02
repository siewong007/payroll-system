use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct EaFormData {
    pub company_name: String,
    pub company_reg_no: String,
    pub company_tax_no: String,
    pub company_epf_no: String,
    pub company_address: String,
    pub employee_name: String,
    pub employee_number: String,
    pub ic_number: String,
    pub tax_id: String,
    pub epf_number: String,
    pub socso_number: String,
    pub employee_address: String,
    pub date_joined: String,
    pub ytd_basic: i64,
    pub ytd_allowances: i64,
    pub ytd_bonus: i64,
    pub ytd_commission: i64,
    pub ytd_overtime: i64,
    pub ytd_gross: i64,
    pub ytd_epf_employee: i64,
    pub ytd_socso_employee: i64,
    pub ytd_eis_employee: i64,
    pub ytd_pcb: i64,
    pub ytd_zakat: i64,
    pub tax_year: i32,
    pub months_worked: i32,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct EaEmployeeSummary {
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_number: String,
    pub ic_number: Option<String>,
    pub ytd_gross: i64,
    pub months_worked: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct EaCompanyRow {
    pub name: String,
    pub registration_number: Option<String>,
    pub tax_number: Option<String>,
    pub epf_number: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postcode: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct EaEmployeeRow {
    pub full_name: String,
    pub employee_number: String,
    pub ic_number: Option<String>,
    pub tax_identification_number: Option<String>,
    pub epf_number: Option<String>,
    pub socso_number: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postcode: Option<String>,
    pub date_joined: chrono::NaiveDate,
}

#[derive(Debug, sqlx::FromRow)]
pub struct EaYtdTotals {
    pub ytd_basic: i64,
    pub ytd_allowances: i64,
    pub ytd_bonus: i64,
    pub ytd_commission: i64,
    pub ytd_overtime: i64,
    pub ytd_gross: i64,
    pub ytd_epf_employee: i64,
    pub ytd_socso_employee: i64,
    pub ytd_eis_employee: i64,
    pub ytd_pcb: i64,
    pub ytd_zakat: i64,
    pub months_worked: i64,
}
