use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct PayslipData {
    pub employee_name: String,
    pub employee_number: String,
    pub ic_number: Option<String>,
    pub department: Option<String>,
    pub designation: Option<String>,
    pub bank_name: Option<String>,
    pub bank_account_number: Option<String>,
    pub period_year: i32,
    pub period_month: i32,
    pub period_start: chrono::NaiveDate,
    pub period_end: chrono::NaiveDate,
    pub pay_date: chrono::NaiveDate,
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
}

#[derive(Debug, sqlx::FromRow)]
pub struct CompanyInfo {
    pub name: String,
    pub registration_number: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postcode: Option<String>,
}

#[derive(Debug)]
pub struct PayslipItemRef {
    pub id: Uuid,
    pub employee_id: Uuid,
}
