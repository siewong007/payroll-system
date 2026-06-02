//! Read model for payslip PDF generation: payroll_items ⋈ payroll_runs ⋈ employees,
//! plus company header details.
//!
//! NOTE: query indentation matches the byte-exact SQL in the offline `.sqlx` cache
//! (the single-payslip and bulk-loop variants were originally at different nesting).

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::payslip::{CompanyInfo, PayslipData, PayslipItemRef};

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
