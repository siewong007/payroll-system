//! Data access for the `payroll_items` table (per-employee payslip rows).

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::payroll::{PayrollItem, PayrollItemBatchInsert, PcbFields};

/// Insert a computed payslip row. The engine computes every value and passes it
/// positionally. New columns are appended to the end of both the column list and
/// the signature so existing placeholders keep their numbering.
#[allow(clippy::too_many_arguments)]
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    payroll_run_id: Uuid,
    employee_id: Uuid,
    basic_salary: i64,
    gross_salary: i64,
    taxable_gross: i64,
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
    total_bonus: i64,
    total_commission: i64,
    working_days: Option<i32>,
    days_worked: Option<rust_decimal::Decimal>,
    is_prorated: bool,
    unpaid_leave_deduction: i64,
    unpaid_leave_days: rust_decimal::Decimal,
    bank_name: Option<&str>,
    bank_account_number: Option<&str>,
    bank_account_type: Option<&str>,
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
            ytd_eis_employee, ytd_zakat, ytd_net,
            total_bonus, total_commission,
            working_days, days_worked, is_prorated,
            unpaid_leave_deduction, unpaid_leave_days,
            taxable_gross,
            bank_name, bank_account_number, bank_account_type
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
            $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31,
            $32, $33, $34, $35, $36, $37,
            $38, $39, $40
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
        total_bonus,
        total_commission,
        working_days,
        days_worked,
        is_prorated,
        unpaid_leave_deduction,
        unpaid_leave_days,
        taxable_gross,
        bank_name,
        bank_account_number,
        bank_account_type,
    )
    .fetch_one(executor)
    .await?;
    Ok(item)
}

/// Bulk-insert all computed payslips for a run in chunked UNNEST statements.
pub async fn insert_batch(
    executor: &mut sqlx::PgConnection,
    payroll_run_id: Uuid,
    rows: &[PayrollItemBatchInsert],
) -> AppResult<()> {
    if rows.is_empty() {
        return Ok(());
    }

    for chunk in rows.chunks(500) {
        let len = chunk.len();
        let mut ids = Vec::with_capacity(len);
        let mut employee_ids = Vec::with_capacity(len);
        let mut basic_salaries = Vec::with_capacity(len);
        let mut gross_salaries = Vec::with_capacity(len);
        let mut total_allowances = Vec::with_capacity(len);
        let mut total_overtimes = Vec::with_capacity(len);
        let mut total_claims = Vec::with_capacity(len);
        let mut epf_employees = Vec::with_capacity(len);
        let mut epf_employers = Vec::with_capacity(len);
        let mut socso_employees = Vec::with_capacity(len);
        let mut socso_employers = Vec::with_capacity(len);
        let mut eis_employees = Vec::with_capacity(len);
        let mut eis_employers = Vec::with_capacity(len);
        let mut pcb_amounts = Vec::with_capacity(len);
        let mut zakat_amounts = Vec::with_capacity(len);
        let mut ptptn_amounts = Vec::with_capacity(len);
        let mut tabung_haji_amounts = Vec::with_capacity(len);
        let mut total_other_deductions = Vec::with_capacity(len);
        let mut total_deductions = Vec::with_capacity(len);
        let mut net_salaries = Vec::with_capacity(len);
        let mut employer_costs = Vec::with_capacity(len);
        let mut ytd_gross = Vec::with_capacity(len);
        let mut ytd_epf_employee = Vec::with_capacity(len);
        let mut ytd_pcb = Vec::with_capacity(len);
        let mut ytd_socso_employee = Vec::with_capacity(len);
        let mut ytd_eis_employee = Vec::with_capacity(len);
        let mut ytd_zakat = Vec::with_capacity(len);
        let mut ytd_net = Vec::with_capacity(len);
        let mut total_bonus = Vec::with_capacity(len);
        let mut total_commission = Vec::with_capacity(len);
        let mut working_days: Vec<Option<i32>> = Vec::with_capacity(len);
        let mut days_worked: Vec<Option<rust_decimal::Decimal>> = Vec::with_capacity(len);
        let mut is_prorated = Vec::with_capacity(len);
        let mut unpaid_leave_deduction = Vec::with_capacity(len);
        let mut unpaid_leave_days = Vec::with_capacity(len);
        let mut taxable_gross = Vec::with_capacity(len);
        let mut bank_names: Vec<Option<String>> = Vec::with_capacity(len);
        let mut bank_account_numbers: Vec<Option<String>> = Vec::with_capacity(len);
        let mut bank_account_types: Vec<Option<String>> = Vec::with_capacity(len);

        for row in chunk {
            ids.push(row.id);
            employee_ids.push(row.employee_id);
            basic_salaries.push(row.basic_salary);
            gross_salaries.push(row.gross_salary);
            total_allowances.push(row.total_allowances);
            total_overtimes.push(row.total_overtime);
            total_claims.push(row.total_claims);
            epf_employees.push(row.epf_employee);
            epf_employers.push(row.epf_employer);
            socso_employees.push(row.socso_employee);
            socso_employers.push(row.socso_employer);
            eis_employees.push(row.eis_employee);
            eis_employers.push(row.eis_employer);
            pcb_amounts.push(row.pcb_amount);
            zakat_amounts.push(row.zakat_amount);
            ptptn_amounts.push(row.ptptn_amount);
            tabung_haji_amounts.push(row.tabung_haji_amount);
            total_other_deductions.push(row.total_other_deductions);
            total_deductions.push(row.total_deductions);
            net_salaries.push(row.net_salary);
            employer_costs.push(row.employer_cost);
            ytd_gross.push(row.ytd_gross);
            ytd_epf_employee.push(row.ytd_epf_employee);
            ytd_pcb.push(row.ytd_pcb);
            ytd_socso_employee.push(row.ytd_socso_employee);
            ytd_eis_employee.push(row.ytd_eis_employee);
            ytd_zakat.push(row.ytd_zakat);
            ytd_net.push(row.ytd_net);
            total_bonus.push(row.total_bonus);
            total_commission.push(row.total_commission);
            working_days.push(row.working_days);
            days_worked.push(row.days_worked);
            is_prorated.push(row.is_prorated);
            unpaid_leave_deduction.push(row.unpaid_leave_deduction);
            unpaid_leave_days.push(row.unpaid_leave_days);
            taxable_gross.push(row.taxable_gross);
            bank_names.push(row.bank_name.clone());
            bank_account_numbers.push(row.bank_account_number.clone());
            bank_account_types.push(row.bank_account_type.clone());
        }

        sqlx::query!(
            r#"INSERT INTO payroll_items (
                id, payroll_run_id, employee_id,
                basic_salary, gross_salary, total_allowances, total_overtime, total_claims,
                epf_employee, epf_employer, socso_employee, socso_employer,
                eis_employee, eis_employer, pcb_amount, zakat_amount,
                ptptn_amount, tabung_haji_amount,
                total_other_deductions, total_deductions, net_salary, employer_cost,
                ytd_gross, ytd_epf_employee, ytd_pcb, ytd_socso_employee,
                ytd_eis_employee, ytd_zakat, ytd_net,
                total_bonus, total_commission,
                working_days, days_worked, is_prorated,
                unpaid_leave_deduction, unpaid_leave_days,
                taxable_gross,
                bank_name, bank_account_number, bank_account_type
            )
            SELECT
                t.id, $1, t.employee_id,
                t.basic_salary, t.gross_salary, t.total_allowances, t.total_overtime, t.total_claims,
                t.epf_employee, t.epf_employer, t.socso_employee, t.socso_employer,
                t.eis_employee, t.eis_employer, t.pcb_amount, t.zakat_amount,
                t.ptptn_amount, t.tabung_haji_amount,
                t.total_other_deductions, t.total_deductions, t.net_salary, t.employer_cost,
                t.ytd_gross, t.ytd_epf_employee, t.ytd_pcb, t.ytd_socso_employee,
                t.ytd_eis_employee, t.ytd_zakat, t.ytd_net,
                t.total_bonus, t.total_commission,
                t.working_days, t.days_worked, t.is_prorated,
                t.unpaid_leave_deduction, t.unpaid_leave_days,
                t.taxable_gross,
                t.bank_name, t.bank_account_number, t.bank_account_type
            FROM UNNEST(
                $2::uuid[], $3::uuid[],
                $4::bigint[], $5::bigint[], $6::bigint[], $7::bigint[], $8::bigint[],
                $9::bigint[], $10::bigint[], $11::bigint[], $12::bigint[],
                $13::bigint[], $14::bigint[], $15::bigint[], $16::bigint[],
                $17::bigint[], $18::bigint[],
                $19::bigint[], $20::bigint[], $21::bigint[], $22::bigint[],
                $23::bigint[], $24::bigint[], $25::bigint[], $26::bigint[],
                $27::bigint[], $28::bigint[], $29::bigint[],
                $30::bigint[], $31::bigint[],
                $32::int4[], $33::numeric[], $34::bool[],
                $35::bigint[], $36::numeric[],
                $37::bigint[],
                $38::text[], $39::text[], $40::text[]
            ) AS t(
                id, employee_id,
                basic_salary, gross_salary, total_allowances, total_overtime, total_claims,
                epf_employee, epf_employer, socso_employee, socso_employer,
                eis_employee, eis_employer, pcb_amount, zakat_amount,
                ptptn_amount, tabung_haji_amount,
                total_other_deductions, total_deductions, net_salary, employer_cost,
                ytd_gross, ytd_epf_employee, ytd_pcb, ytd_socso_employee,
                ytd_eis_employee, ytd_zakat, ytd_net,
                total_bonus, total_commission,
                working_days, days_worked, is_prorated,
                unpaid_leave_deduction, unpaid_leave_days,
                taxable_gross,
                bank_name, bank_account_number, bank_account_type
            )"#,
            payroll_run_id,
            &ids,
            &employee_ids,
            &basic_salaries,
            &gross_salaries,
            &total_allowances,
            &total_overtimes,
            &total_claims,
            &epf_employees,
            &epf_employers,
            &socso_employees,
            &socso_employers,
            &eis_employees,
            &eis_employers,
            &pcb_amounts,
            &zakat_amounts,
            &ptptn_amounts,
            &tabung_haji_amounts,
            &total_other_deductions,
            &total_deductions,
            &net_salaries,
            &employer_costs,
            &ytd_gross,
            &ytd_epf_employee,
            &ytd_pcb,
            &ytd_socso_employee,
            &ytd_eis_employee,
            &ytd_zakat,
            &ytd_net,
            &total_bonus,
            &total_commission,
            &working_days as &[Option<i32>],
            &days_worked as &[Option<rust_decimal::Decimal>],
            &is_prorated,
            &unpaid_leave_deduction,
            &unpaid_leave_days,
            &taxable_gross,
            &bank_names as &[Option<String>],
            &bank_account_numbers as &[Option<String>],
            &bank_account_types as &[Option<String>],
        )
        .execute(&mut *executor)
        .await?;
    }
    Ok(())
}

/// All payslip rows for a run, oldest first.
///
/// `id` breaks ties on `created_at`. Ties are the normal case here rather than a
/// restore artefact: `now()` is transaction-start time, so every payslip written
/// by one run already carries the same instant.
pub async fn list_for_run(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
) -> AppResult<Vec<PayrollItem>> {
    let items = sqlx::query_as!(
        PayrollItem,
        "SELECT * FROM payroll_items WHERE payroll_run_id = $1 ORDER BY created_at, id",
        run_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(items)
}

/// One employee's payslip row within a run.
pub async fn get_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    employee_id: Uuid,
) -> AppResult<Option<PayrollItem>> {
    let item = sqlx::query_as!(
        PayrollItem,
        "SELECT * FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2",
        run_id,
        employee_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(item)
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
        r#"SELECT id, pcb_amount, total_deductions, net_salary, ytd_pcb, ytd_net
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
    ytd_net: i64,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE payroll_items
        SET pcb_amount = $3,
            total_deductions = $4,
            net_salary = $5,
            ytd_pcb = $6,
            ytd_net = $7,
            updated_at = NOW()
        WHERE payroll_run_id = $1 AND employee_id = $2"#,
        run_id,
        employee_id,
        pcb_amount,
        total_deductions,
        net_salary,
        ytd_pcb,
        ytd_net,
    )
    .execute(executor)
    .await?;
    Ok(())
}
