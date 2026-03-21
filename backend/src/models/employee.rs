use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Employee {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_number: String,

    // Personal
    pub full_name: String,
    pub ic_number: Option<String>,
    pub passport_number: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub gender: Option<String>,
    pub nationality: Option<String>,
    pub race: Option<String>,
    pub residency_status: String,
    pub marital_status: Option<String>,

    // Contact
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postcode: Option<String>,

    // Employment
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

    // Salary (in sen)
    pub basic_salary: i64,
    pub hourly_rate: Option<i64>,
    pub daily_rate: Option<i64>,

    // Banking
    pub bank_name: Option<String>,
    pub bank_account_number: Option<String>,
    pub bank_account_type: Option<String>,

    // Statutory
    pub tax_identification_number: Option<String>,
    pub epf_number: Option<String>,
    pub socso_number: Option<String>,
    pub eis_number: Option<String>,

    // Tax factors
    pub working_spouse: Option<bool>,
    pub num_children: Option<i32>,
    pub epf_category: Option<String>,

    // Islamic / special
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
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateEmployeeRequest {
    pub employee_number: String,
    pub full_name: String,
    pub ic_number: Option<String>,
    pub passport_number: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub gender: Option<String>,
    pub nationality: Option<String>,
    pub race: Option<String>,
    pub residency_status: Option<String>,
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
    pub employment_type: Option<String>,
    pub date_joined: NaiveDate,
    pub probation_start: Option<NaiveDate>,
    pub probation_end: Option<NaiveDate>,
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
    pub payroll_group_id: Option<Uuid>,
    pub salary_group: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEmployeeRequest {
    pub full_name: Option<String>,
    pub ic_number: Option<String>,
    pub passport_number: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub gender: Option<String>,
    pub nationality: Option<String>,
    pub race: Option<String>,
    pub residency_status: Option<String>,
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
    pub employment_type: Option<String>,
    pub probation_start: Option<NaiveDate>,
    pub probation_end: Option<NaiveDate>,
    pub confirmation_date: Option<NaiveDate>,
    pub date_resigned: Option<NaiveDate>,
    pub resignation_reason: Option<String>,
    pub basic_salary: Option<i64>,
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
    pub payroll_group_id: Option<Uuid>,
    pub salary_group: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SalaryHistory {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub old_salary: i64,
    pub new_salary: i64,
    pub effective_date: NaiveDate,
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Tp3Record {
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
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTp3Request {
    pub tax_year: i32,
    pub previous_employer_name: Option<String>,
    pub previous_income_ytd: i64,
    pub previous_epf_ytd: i64,
    pub previous_pcb_ytd: i64,
    pub previous_socso_ytd: i64,
    pub previous_zakat_ytd: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmployeeAllowance {
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
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}
