//! Data access for the `payroll_items` table (per-employee payslip rows).

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::payroll::PayrollItem;

/// Insert a computed payslip row. Arguments are in the exact column order of the
/// INSERT; the engine computes every value and passes it positionally.
#[allow(clippy::too_many_arguments)]
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    payroll_run_id: Uuid,
    employee_id: Uuid,
    basic_salary: i64,
    gross_salary: i64,
    total_allowances: i64,
    total_overtime: i64,
    total_claims: i64,
    epf_employee: i64,
    epf_employer: i64,
    socso_employee: i64,
    socso_employer: i64,
    eis_employee: i64,
    eis_employer: i64,
    pcb_amount: i64,
    zakat_amount: i64,
    ptptn_amount: i64,
    tabung_haji_amount: i64,
    total_other_deductions: i64,
    total_deductions: i64,
    net_salary: i64,
    employer_cost: i64,
    ytd_gross: i64,
    ytd_epf_employee: i64,
    ytd_pcb: i64,
    ytd_socso_employee: i64,
    ytd_eis_employee: i64,
    ytd_zakat: i64,
    ytd_net: i64,
) -> AppResult<PayrollItem> {
    let item = sqlx::query_as!(
        PayrollItem,
        r#"INSERT INTO payroll_items (
            id, payroll_run_id, employee_id,
            basic_salary, gross_salary, total_allowances, total_overtime, total_claims,
            epf_employee, epf_employer, socso_employee, socso_employer,
            eis_employee, eis_employer, pcb_amount, zakat_amount,
            ptptn_amount, tabung_haji_amount,
            total_other_deductions, total_deductions, net_salary, employer_cost,
            ytd_gross, ytd_epf_employee, ytd_pcb, ytd_socso_employee,
            ytd_eis_employee, ytd_zakat, ytd_net
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
            $21, $22, $23, $24, $25, $26, $27, $28, $29
        ) RETURNING *"#,
        id,
        payroll_run_id,
        employee_id,
        basic_salary,
        gross_salary,
        total_allowances,
        total_overtime,
        total_claims,
        epf_employee,
        epf_employer,
        socso_employee,
        socso_employer,
        eis_employee,
        eis_employer,
        pcb_amount,
        zakat_amount,
        ptptn_amount,
        tabung_haji_amount,
        total_other_deductions,
        total_deductions,
        net_salary,
        employer_cost,
        ytd_gross,
        ytd_epf_employee,
        ytd_pcb,
        ytd_socso_employee,
        ytd_eis_employee,
        ytd_zakat,
        ytd_net,
    )
    .fetch_one(executor)
    .await?;
    Ok(item)
}
