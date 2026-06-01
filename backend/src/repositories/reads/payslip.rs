//! Read model for payslip PDF generation: payroll_items ⋈ payroll_runs ⋈ employees,
//! plus company header details.
//!
//! NOTE: query indentation matches the byte-exact SQL in the offline `.sqlx` cache
//! (the single-payslip and bulk-loop variants were originally at different nesting).

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

#[derive(Debug, sqlx::FromRow)]
pub struct PayslipData {
    // Employee
    pub employee_name: String,
    pub employee_number: String,
    pub ic_number: Option<String>,
    pub department: Option<String>,
    pub designation: Option<String>,
    pub bank_name: Option<String>,
    pub bank_account_number: Option<String>,
    // Period
    pub period_year: i32,
    pub period_month: i32,
    pub period_start: chrono::NaiveDate,
    pub period_end: chrono::NaiveDate,
    pub pay_date: chrono::NaiveDate,
    // Earnings
    pub basic_salary: i64,
    pub gross_salary: i64,
    pub total_allowances: i64,
    pub total_overtime: i64,
    pub total_bonus: i64,
    pub total_commission: i64,
    pub total_claims: i64,
    // Deductions
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
    // YTD
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

/// Payslip data for an employee's own payslip — restricted to approved/paid runs.
pub async fn payslip_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    payslip_id: Uuid,
    employee_id: Uuid,
) -> AppResult<Option<PayslipData>> {
    let data = sqlx::query_as!(
        PayslipData,
        r#"SELECT
            e.full_name AS employee_name, e.employee_number, e.ic_number,
            e.department, e.designation, e.bank_name, e.bank_account_number,
            pr.period_year, pr.period_month, pr.period_start, pr.period_end, pr.pay_date,
            pi.basic_salary, pi.gross_salary, pi.total_allowances, pi.total_overtime,
            pi.total_bonus, pi.total_commission, pi.total_claims,
            pi.epf_employee, pi.epf_employer, pi.socso_employee, pi.socso_employer,
            pi.eis_employee, pi.eis_employer, pi.pcb_amount, pi.zakat_amount,
            pi.ptptn_amount, pi.tabung_haji_amount, pi.total_loan_deductions,
            pi.total_other_deductions, pi.unpaid_leave_deduction,
            pi.total_deductions, pi.net_salary, pi.employer_cost,
            pi.ytd_gross, pi.ytd_epf_employee, pi.ytd_pcb,
            pi.ytd_socso_employee, pi.ytd_eis_employee, pi.ytd_zakat, pi.ytd_net
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        JOIN employees e ON pi.employee_id = e.id
        WHERE pi.id = $1 AND pi.employee_id = $2
        AND pr.status::text IN ('approved', 'paid')"#,
        payslip_id,
        employee_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(data)
}

/// Payslip data by item id, without a status filter — bulk callers pre-filter at the
/// run level. Indentation matches the original bulk-loop query in the offline cache.
pub async fn payslip_for_run_item(
    executor: impl Executor<'_, Database = Postgres>,
    payslip_id: Uuid,
    employee_id: Uuid,
) -> AppResult<Option<PayslipData>> {
    let data = sqlx::query_as!(
        PayslipData,
        r#"SELECT
                e.full_name AS employee_name, e.employee_number, e.ic_number,
                e.department, e.designation, e.bank_name, e.bank_account_number,
                pr.period_year, pr.period_month, pr.period_start, pr.period_end, pr.pay_date,
                pi.basic_salary, pi.gross_salary, pi.total_allowances, pi.total_overtime,
                pi.total_bonus, pi.total_commission, pi.total_claims,
                pi.epf_employee, pi.epf_employer, pi.socso_employee, pi.socso_employer,
                pi.eis_employee, pi.eis_employer, pi.pcb_amount, pi.zakat_amount,
                pi.ptptn_amount, pi.tabung_haji_amount, pi.total_loan_deductions,
                pi.total_other_deductions, pi.unpaid_leave_deduction,
                pi.total_deductions, pi.net_salary, pi.employer_cost,
                pi.ytd_gross, pi.ytd_epf_employee, pi.ytd_pcb,
                pi.ytd_socso_employee, pi.ytd_eis_employee, pi.ytd_zakat, pi.ytd_net
            FROM payroll_items pi
            JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
            JOIN employees e ON pi.employee_id = e.id
            WHERE pi.id = $1 AND pi.employee_id = $2"#,
        payslip_id,
        employee_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(data)
}

/// Company header details for an employee's payslip.
pub async fn company_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
) -> AppResult<CompanyInfo> {
    let company = sqlx::query_as!(
        CompanyInfo,
        r#"SELECT name, registration_number, address_line1, address_line2, city, state, postcode
        FROM companies WHERE id = (SELECT company_id FROM employees WHERE id = $1)"#,
        employee_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(company)
}

/// The (id, employee_id) of each payslip in an approved/paid run, ordered by employee.
pub async fn run_payslip_item_refs(
    executor: impl Executor<'_, Database = Postgres>,
    payroll_run_id: Uuid,
    company_id: Uuid,
) -> AppResult<Vec<PayslipItemRef>> {
    let items = sqlx::query_as!(
        PayslipItemRef,
        r#"SELECT pi.id, pi.employee_id
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        WHERE pr.id = $1 AND pr.company_id = $2
        AND pr.status::text IN ('approved', 'paid')
        ORDER BY (SELECT employee_number FROM employees WHERE id = pi.employee_id)"#,
        payroll_run_id,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(items)
}
