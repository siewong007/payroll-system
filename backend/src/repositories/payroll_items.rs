//! Data access for the `payroll_items` table (per-employee payslip rows).

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::payroll::{PayrollItem, PcbFields};

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

/// All payslip rows for a run, oldest first.
pub async fn list_for_run(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
) -> AppResult<Vec<PayrollItem>> {
    let items = sqlx::query_as!(
        PayrollItem,
        "SELECT * FROM payroll_items WHERE payroll_run_id = $1 ORDER BY created_at",
        run_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(items)
}

pub async fn delete_for_run(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM payroll_items WHERE payroll_run_id = $1",
        run_id
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn get_pcb_fields_locked(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    employee_id: Uuid,
) -> AppResult<Option<PcbFields>> {
    let row = sqlx::query_as!(
        PcbFields,
        r#"SELECT pcb_amount, total_deductions, net_salary, ytd_pcb
        FROM payroll_items
        WHERE payroll_run_id = $1 AND employee_id = $2
        FOR UPDATE"#,
        run_id,
        employee_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

#[allow(clippy::too_many_arguments)]
pub async fn update_pcb(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    employee_id: Uuid,
    pcb_amount: i64,
    total_deductions: i64,
    net_salary: i64,
    ytd_pcb: i64,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE payroll_items
        SET pcb_amount = $3,
            total_deductions = $4,
            net_salary = $5,
            ytd_pcb = $6,
            updated_at = NOW()
        WHERE payroll_run_id = $1 AND employee_id = $2"#,
        run_id,
        employee_id,
        pcb_amount,
        total_deductions,
        net_salary,
        ytd_pcb,
    )
    .execute(executor)
    .await?;
    Ok(())
}
