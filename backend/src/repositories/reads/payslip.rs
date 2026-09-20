//! Read model for payslip PDF generation: payroll_items ⋈ payroll_runs ⋈ employees,
//! plus company header details.
//!
//! NOTE: query indentation matches the byte-exact SQL in the offline `.sqlx` cache.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::payslip::{CompanyInfo, PayslipData};

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

/// Every payslip in an approved/paid run in one round trip, ordered by
/// employee number — the bulk PDF path's data source.
///
/// Each row pairs the item id (the key the breakdown lines are fetched by)
/// with the full `PayslipData`. The per-item variant this replaced cost three
/// queries per employee — run row, company, lines — which is ~1,500 round
/// trips on a 500-headcount run inside one request.
pub async fn payslips_for_run(
    executor: impl Executor<'_, Database = Postgres>,
    payroll_run_id: Uuid,
    company_id: Uuid,
) -> AppResult<Vec<(Uuid, PayslipData)>> {
    let rows = sqlx::query!(
        r#"SELECT
            pi.id AS item_id,
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
        WHERE pr.id = $1 AND pr.company_id = $2
        AND pr.status::text IN ('approved', 'paid')
        ORDER BY e.employee_number"#,
        payroll_run_id,
        company_id,
    )
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            (
                r.item_id,
                PayslipData {
                    employee_name: r.employee_name,
                    employee_number: r.employee_number,
                    ic_number: r.ic_number,
                    department: r.department,
                    designation: r.designation,
                    bank_name: r.bank_name,
                    bank_account_number: r.bank_account_number,
                    period_year: r.period_year,
                    period_month: r.period_month,
                    period_start: r.period_start,
                    period_end: r.period_end,
                    pay_date: r.pay_date,
                    basic_salary: r.basic_salary,
                    gross_salary: r.gross_salary,
                    total_allowances: r.total_allowances,
                    total_overtime: r.total_overtime,
                    total_bonus: r.total_bonus,
                    total_commission: r.total_commission,
                    total_claims: r.total_claims,
                    epf_employee: r.epf_employee,
                    epf_employer: r.epf_employer,
                    socso_employee: r.socso_employee,
                    socso_employer: r.socso_employer,
                    eis_employee: r.eis_employee,
                    eis_employer: r.eis_employer,
                    pcb_amount: r.pcb_amount,
                    zakat_amount: r.zakat_amount,
                    ptptn_amount: r.ptptn_amount,
                    tabung_haji_amount: r.tabung_haji_amount,
                    total_loan_deductions: r.total_loan_deductions,
                    total_other_deductions: r.total_other_deductions,
                    unpaid_leave_deduction: r.unpaid_leave_deduction,
                    total_deductions: r.total_deductions,
                    net_salary: r.net_salary,
                    employer_cost: r.employer_cost,
                    ytd_gross: r.ytd_gross,
                    ytd_epf_employee: r.ytd_epf_employee,
                    ytd_pcb: r.ytd_pcb,
                    ytd_socso_employee: r.ytd_socso_employee,
                    ytd_eis_employee: r.ytd_eis_employee,
                    ytd_zakat: r.ytd_zakat,
                    ytd_net: r.ytd_net,
                },
            )
        })
        .collect())
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

/// Company header details by id — the bulk path knows the company up front, so
/// the per-employee self-join above is not needed.
pub async fn company_by_id(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<CompanyInfo> {
    let company = sqlx::query_as!(
        CompanyInfo,
        r#"SELECT name, registration_number, address_line1, address_line2, city, state, postcode
        FROM companies WHERE id = $1"#,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(company)
}
