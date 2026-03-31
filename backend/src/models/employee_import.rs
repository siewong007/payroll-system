use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRowRaw {
    pub row_number: usize,
    pub employee_number: Option<String>,
    pub full_name: Option<String>,
    pub ic_number: Option<String>,
    pub passport_number: Option<String>,
    pub date_of_birth: Option<String>,
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
    pub date_joined: Option<String>,
    pub probation_start: Option<String>,
    pub probation_end: Option<String>,
    pub basic_salary: Option<String>,
    pub hourly_rate: Option<String>,
    pub daily_rate: Option<String>,
    pub bank_name: Option<String>,
    pub bank_account_number: Option<String>,
    pub bank_account_type: Option<String>,
    pub tax_identification_number: Option<String>,
    pub epf_number: Option<String>,
    pub socso_number: Option<String>,
    pub eis_number: Option<String>,
    pub working_spouse: Option<String>,
    pub num_children: Option<String>,
    pub epf_category: Option<String>,
    pub is_muslim: Option<String>,
    pub zakat_eligible: Option<String>,
    pub zakat_monthly_amount: Option<String>,
    pub ptptn_monthly_amount: Option<String>,
    pub tabung_haji_amount: Option<String>,
    pub payroll_group_id: Option<String>,
    pub salary_group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRowValidation {
    pub row_number: usize,
    pub status: RowStatus,
    pub errors: Vec<FieldError>,
    pub data: ImportRowRaw,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RowStatus {
    Valid,
    Error,
    Duplicate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ImportValidationResponse {
    pub session_id: Uuid,
    pub total_rows: usize,
    pub valid_rows: usize,
    pub error_rows: usize,
    pub duplicate_rows: usize,
    pub rows: Vec<ImportRowValidation>,
}

#[derive(Debug, Deserialize)]
pub struct ImportConfirmRequest {
    pub session_id: Uuid,
    pub skip_invalid: bool,
}

#[derive(Debug, Serialize)]
pub struct ImportConfirmResponse {
    pub imported_count: usize,
    pub skipped_count: usize,
    pub errors: Vec<ImportRowValidation>,
}

#[derive(Debug, Deserialize)]
pub struct TemplateQuery {
    pub format: Option<String>,
}
