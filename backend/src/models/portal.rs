use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Leave
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LeaveType {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub default_days: rust_decimal::Decimal,
    pub is_paid: bool,
    pub is_active: bool,
    pub max_carry_forward: rust_decimal::Decimal,
    pub carry_forward_expiry_months: i32,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LeaveBalance {
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

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct LeaveBalanceWithType {
    pub id: Uuid,
    pub leave_type_id: Uuid,
    pub leave_type_name: String,
    pub is_paid: bool,
    pub year: i32,
    pub entitled_days: rust_decimal::Decimal,
    pub taken_days: rust_decimal::Decimal,
    pub pending_days: rust_decimal::Decimal,
    pub carried_forward: rust_decimal::Decimal,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct LeaveRequest {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub company_id: Uuid,
    pub leave_type_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub days: rust_decimal::Decimal,
    pub reason: Option<String>,
    pub status: String,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub review_notes: Option<String>,
    pub attachment_url: Option<String>,
    pub attachment_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // joined
    pub leave_type_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLeaveRequest {
    pub leave_type_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub days: rust_decimal::Decimal,
    pub reason: Option<String>,
    pub attachment_url: Option<String>,
    pub attachment_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLeaveRequest {
    pub leave_type_id: Option<Uuid>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub days: Option<rust_decimal::Decimal>,
    pub reason: Option<String>,
    pub attachment_url: Option<String>,
    pub attachment_name: Option<String>,
}

// Claims
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Claim {
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
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub review_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateClaimRequest {
    pub title: String,
    pub description: Option<String>,
    pub amount: i64,
    pub category: Option<String>,
    pub receipt_url: Option<String>,
    pub receipt_file_name: Option<String>,
    pub expense_date: NaiveDate,
}

#[derive(Debug, Deserialize)]
pub struct UpdateClaimRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub amount: Option<i64>,
    pub category: Option<String>,
    pub receipt_url: Option<String>,
    pub receipt_file_name: Option<String>,
    pub expense_date: Option<NaiveDate>,
}

// Overtime Applications
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct OvertimeApplication {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub company_id: Uuid,
    pub ot_date: NaiveDate,
    pub start_time: chrono::NaiveTime,
    pub end_time: chrono::NaiveTime,
    pub hours: rust_decimal::Decimal,
    pub ot_type: String,
    pub reason: Option<String>,
    pub status: String,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub review_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateOvertimeRequest {
    pub ot_date: NaiveDate,
    pub start_time: String,
    pub end_time: String,
    pub hours: rust_decimal::Decimal,
    pub ot_type: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOvertimeRequest {
    pub ot_date: Option<NaiveDate>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub hours: Option<rust_decimal::Decimal>,
    pub ot_type: Option<String>,
    pub reason: Option<String>,
}

// Employee self-update (limited fields)
#[derive(Debug, Deserialize)]
pub struct UpdateMyProfileRequest {
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postcode: Option<String>,
    pub marital_status: Option<String>,
    pub bank_name: Option<String>,
    pub bank_account_number: Option<String>,
}

// Payslip view for employee
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MyPayslip {
    pub id: Uuid,
    pub payroll_run_id: Uuid,
    pub period_year: i32,
    pub period_month: i32,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub pay_date: NaiveDate,

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
