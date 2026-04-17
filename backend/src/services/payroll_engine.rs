use chrono::{Datelike, NaiveDate};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::employee::Employee;
use crate::models::payroll::{PayrollItem, PayrollRun};
use crate::services::epf_service;
use crate::services::eis_service;
use crate::services::pcb_calculator::{self, PcbInput};
use crate::services::socso_service;

/// Process payroll for a group in a given period.
///
/// 1. Fetch all active employees in the payroll group
/// 2. For each employee, calculate gross, statutory deductions, net
/// 3. Create PayrollRun + PayrollItems in a transaction
#[allow(clippy::too_many_arguments)]
pub async fn process_payroll(
    pool: &PgPool,
    company_id: Uuid,
    payroll_group_id: Uuid,
    year: i32,
    month: i32,
    pay_date: NaiveDate,
    processed_by: Uuid,
    notes: Option<String>,
) -> AppResult<PayrollRun> {
    // Check for existing run
    let existing = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM payroll_runs
        WHERE company_id = $1 AND payroll_group_id = $2
        AND period_year = $3 AND period_month = $4
        AND status NOT IN ('cancelled')"#,
    )
    .bind(company_id)
    .bind(payroll_group_id)
    .bind(year)
    .bind(month)
    .fetch_one(pool)
    .await?;

    if existing > 0 {
        return Err(AppError::Conflict(
            "Payroll already exists for this period. Cancel the existing run first.".into(),
        ));
    }

    // Period dates
    let period_start = NaiveDate::from_ymd_opt(year, month as u32, 1)
        .ok_or_else(|| AppError::BadRequest("Invalid period".into()))?;
    let period_end = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, (month + 1) as u32, 1)
    }
    .and_then(|d| d.pred_opt())
    .ok_or_else(|| AppError::BadRequest("Invalid period".into()))?;

    let effective_date = period_end;

    // Fetch active employees in this payroll group
    let employees = sqlx::query_as::<_, Employee>(
        r#"SELECT id, company_id, employee_number, full_name, ic_number, passport_number,
            date_of_birth, gender::text, nationality, race::text, residency_status::text,
            marital_status::text, email, phone, address_line1, address_line2, city, state, postcode,
            department, designation, cost_centre, branch, employment_type::text,
            date_joined, probation_start, probation_end, confirmation_date,
            date_resigned, resignation_reason, basic_salary, hourly_rate, daily_rate,
            bank_name, bank_account_number, bank_account_type,
            tax_identification_number, epf_number, socso_number, eis_number,
            working_spouse, num_children, epf_category, is_muslim, zakat_eligible,
            zakat_monthly_amount, ptptn_monthly_amount, tabung_haji_amount,
            hrdf_contribution, payroll_group_id, salary_group, is_active,
            deleted_at, created_at, updated_at, created_by, updated_by
        FROM employees
        WHERE company_id = $1 AND payroll_group_id = $2
        AND is_active = TRUE AND deleted_at IS NULL
        AND date_joined <= $3
        AND (date_resigned IS NULL OR date_resigned >= $4)"#,
    )
    .bind(company_id)
    .bind(payroll_group_id)
    .bind(period_end)
    .bind(period_start)
    .fetch_all(pool)
    .await?;

    if employees.is_empty() {
        return Err(AppError::BadRequest(
            "No active employees found in this payroll group for the selected period".into(),
        ));
    }

    // Begin transaction
    let mut tx = pool.begin().await?;

    // Create payroll run
    let run_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO payroll_runs
        (id, company_id, payroll_group_id, period_year, period_month,
         period_start, period_end, pay_date, status, processed_by, processed_at, notes, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'processing', $9, NOW(), $10, $9)"#,
    )
    .bind(run_id)
    .bind(company_id)
    .bind(payroll_group_id)
    .bind(year)
    .bind(month)
    .bind(period_start)
    .bind(period_end)
    .bind(pay_date)
    .bind(processed_by)
    .bind(&notes)
    .execute(&mut *tx)
    .await?;

    let mut total_gross: i64 = 0;
    let mut total_net: i64 = 0;
    let mut total_employer_cost: i64 = 0;
    let mut total_epf_ee: i64 = 0;
    let mut total_epf_er: i64 = 0;
    let mut total_socso_ee: i64 = 0;
    let mut total_socso_er: i64 = 0;
    let mut total_eis_ee: i64 = 0;
    let mut total_eis_er: i64 = 0;
    let mut total_pcb: i64 = 0;
    let mut total_zakat: i64 = 0;

    for emp in &employees {
        let item = process_employee(pool, &mut tx, run_id, emp, year, month, effective_date).await?;

        total_gross += item.gross_salary;
        total_net += item.net_salary;
        total_employer_cost += item.employer_cost;
        total_epf_ee += item.epf_employee;
        total_epf_er += item.epf_employer;
        total_socso_ee += item.socso_employee;
        total_socso_er += item.socso_employer;
        total_eis_ee += item.eis_employee;
        total_eis_er += item.eis_employer;
        total_pcb += item.pcb_amount;
        total_zakat += item.zakat_amount;
    }

    // Update run totals
    sqlx::query(
        r#"UPDATE payroll_runs SET
        status = 'processed',
        total_gross = $2, total_net = $3, total_employer_cost = $4,
        total_epf_employee = $5, total_epf_employer = $6,
        total_socso_employee = $7, total_socso_employer = $8,
        total_eis_employee = $9, total_eis_employer = $10,
        total_pcb = $11, total_zakat = $12,
        employee_count = $13, updated_at = NOW()
        WHERE id = $1"#,
    )
    .bind(run_id)
    .bind(total_gross)
    .bind(total_net)
    .bind(total_employer_cost)
    .bind(total_epf_ee)
    .bind(total_epf_er)
    .bind(total_socso_ee)
    .bind(total_socso_er)
    .bind(total_eis_ee)
    .bind(total_eis_er)
    .bind(total_pcb)
    .bind(total_zakat)
    .bind(employees.len() as i32)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // Return the completed run
    let run = sqlx::query_as::<_, PayrollRun>(
        r#"SELECT id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text as status,
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by
        FROM payroll_runs WHERE id = $1"#,
    )
        .bind(run_id)
        .fetch_one(pool)
        .await?;

    Ok(run)
}

/// Process a single employee's payroll
async fn process_employee(
    pool: &PgPool,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_id: Uuid,
    emp: &Employee,
    year: i32,
    month: i32,
    effective_date: NaiveDate,
) -> AppResult<PayrollItem> {
    // Calculate age
    let age = calculate_age(emp.date_of_birth, effective_date);
    let is_foreigner = emp.residency_status == "foreigner";
    let epf_category = emp.epf_category.clone().unwrap_or_else(|| "A".to_string());

    // Gross salary = basic + recurring allowances
    let basic = emp.basic_salary;

    // Fetch recurring allowances
    let allowances_total: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(amount), 0)::BIGINT FROM employee_allowances
        WHERE employee_id = $1 AND category = 'earning'
        AND is_active = TRUE AND is_recurring = TRUE
        AND effective_from <= $2
        AND (effective_to IS NULL OR effective_to >= $2)"#,
    )
    .bind(emp.id)
    .bind(effective_date)
    .fetch_one(&mut **tx)
    .await?;

    // Fetch staged variable entries for this period
    let variable_earnings: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(amount), 0)::BIGINT FROM payroll_entries
        WHERE employee_id = $1 AND period_year = $2 AND period_month = $3
        AND category = 'earning' AND is_processed = FALSE"#,
    )
    .bind(emp.id)
    .bind(year)
    .bind(month)
    .fetch_one(&mut **tx)
    .await?;

    // Fetch staged deductions
    let variable_deductions: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(amount), 0)::BIGINT FROM payroll_entries
        WHERE employee_id = $1 AND period_year = $2 AND period_month = $3
        AND category = 'deduction' AND is_processed = FALSE"#,
    )
    .bind(emp.id)
    .bind(year)
    .bind(month)
    .fetch_one(&mut **tx)
    .await?;

    let recurring_deductions: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(amount), 0)::BIGINT FROM employee_allowances
        WHERE employee_id = $1 AND category = 'deduction'
        AND is_active = TRUE AND is_recurring = TRUE
        AND effective_from <= $2
        AND (effective_to IS NULL OR effective_to >= $2)"#,
    )
    .bind(emp.id)
    .bind(effective_date)
    .fetch_one(&mut **tx)
    .await?;

    let gross = basic + allowances_total + variable_earnings;

    // EPF
    let epf = epf_service::calculate_epf(pool, gross, &epf_category, effective_date).await?;

    // SOCSO
    let socso = socso_service::calculate_socso(pool, gross, age, is_foreigner, effective_date).await?;

    // EIS
    let eis = eis_service::calculate_eis(pool, gross, age, is_foreigner, effective_date).await?;

    // Get YTD figures (from previous months this year)
    let (ytd_gross, ytd_pcb, ytd_epf, ytd_socso, ytd_eis, ytd_zakat, ytd_net) =
        get_ytd_figures(pool, emp.id, year, month).await?;

    // Get TP3 data if exists
    let tp3 = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        r#"SELECT previous_income_ytd, previous_epf_ytd, previous_pcb_ytd, previous_zakat_ytd
        FROM tp3_records WHERE employee_id = $1 AND tax_year = $2"#,
    )
    .bind(emp.id)
    .bind(year)
    .fetch_optional(pool)
    .await?;

    let (tp3_income, tp3_epf, tp3_pcb, tp3_zakat) = tp3.unwrap_or((0, 0, 0, 0));

    // Zakat
    let zakat = if emp.zakat_eligible.unwrap_or(false) {
        emp.zakat_monthly_amount.unwrap_or(0)
    } else {
        0
    };

    // PCB
    let pcb_input = PcbInput {
        monthly_gross: gross,
        epf_employee_monthly: epf.employee,
        socso_employee_monthly: socso.employee,
        eis_employee_monthly: eis.employee,
        zakat_monthly: zakat,
        marital_status: emp.marital_status.clone().unwrap_or_else(|| "single".into()),
        working_spouse: emp.working_spouse.unwrap_or(false),
        num_children: emp.num_children.unwrap_or(0),
        months_worked: month,
        ytd_gross: ytd_gross + tp3_income,
        ytd_pcb: ytd_pcb + tp3_pcb,
        ytd_epf: ytd_epf + tp3_epf,
        ytd_socso,
        ytd_eis,
        ytd_zakat: ytd_zakat + tp3_zakat,
        is_bonus_month: false,
        bonus_amount: 0,
    };

    let pcb = pcb_calculator::calculate_pcb(pool, &pcb_input, effective_date).await?;

    // PTPTN and Tabung Haji
    let ptptn = emp.ptptn_monthly_amount.unwrap_or(0);
    let tabung_haji = emp.tabung_haji_amount.unwrap_or(0);

    // Total deductions
    let total_deductions = epf.employee
        + socso.employee
        + eis.employee
        + pcb
        + zakat
        + ptptn
        + tabung_haji
        + recurring_deductions
        + variable_deductions;

    let net = gross - total_deductions;
    let employer_cost = gross + epf.employer + socso.employer + eis.employer;

    // New YTD
    let new_ytd_gross = ytd_gross + gross;
    let new_ytd_epf = ytd_epf + epf.employee;
    let new_ytd_pcb = ytd_pcb + pcb;
    let new_ytd_socso = ytd_socso + socso.employee;
    let new_ytd_eis = ytd_eis + eis.employee;
    let new_ytd_zakat = ytd_zakat + zakat;
    let new_ytd_net = ytd_net + net;

    // Insert payroll item
    let item_id = Uuid::new_v4();
    let item = sqlx::query_as::<_, PayrollItem>(
        r#"INSERT INTO payroll_items (
            id, payroll_run_id, employee_id,
            basic_salary, gross_salary, total_allowances,
            epf_employee, epf_employer, socso_employee, socso_employer,
            eis_employee, eis_employer, pcb_amount, zakat_amount,
            ptptn_amount, tabung_haji_amount,
            total_other_deductions, total_deductions, net_salary, employer_cost,
            ytd_gross, ytd_epf_employee, ytd_pcb, ytd_socso_employee,
            ytd_eis_employee, ytd_zakat, ytd_net
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
            $21, $22, $23, $24, $25, $26, $27
        ) RETURNING *"#,
    )
    .bind(item_id)
    .bind(run_id)
    .bind(emp.id)
    .bind(basic)
    .bind(gross)
    .bind(allowances_total)
    .bind(epf.employee)
    .bind(epf.employer)
    .bind(socso.employee)
    .bind(socso.employer)
    .bind(eis.employee)
    .bind(eis.employer)
    .bind(pcb)
    .bind(zakat)
    .bind(ptptn)
    .bind(tabung_haji)
    .bind(recurring_deductions + variable_deductions)
    .bind(total_deductions)
    .bind(net)
    .bind(employer_cost)
    .bind(new_ytd_gross)
    .bind(new_ytd_epf)
    .bind(new_ytd_pcb)
    .bind(new_ytd_socso)
    .bind(new_ytd_eis)
    .bind(new_ytd_zakat)
    .bind(new_ytd_net)
    .fetch_one(&mut **tx)
    .await?;

    // Mark staged entries as processed
    sqlx::query(
        r#"UPDATE payroll_entries SET is_processed = TRUE, payroll_run_id = $1
        WHERE employee_id = $2 AND period_year = $3 AND period_month = $4 AND is_processed = FALSE"#,
    )
    .bind(run_id)
    .bind(emp.id)
    .bind(year)
    .bind(month)
    .execute(&mut **tx)
    .await?;

    Ok(item)
}

/// Get YTD figures from previous payroll items in the same tax year
async fn get_ytd_figures(
    pool: &PgPool,
    employee_id: Uuid,
    year: i32,
    current_month: i32,
) -> AppResult<(i64, i64, i64, i64, i64, i64, i64)> {
    let result = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64)>(
        r#"SELECT
            COALESCE(SUM(pi.gross_salary), 0)::BIGINT,
            COALESCE(SUM(pi.pcb_amount), 0)::BIGINT,
            COALESCE(SUM(pi.epf_employee), 0)::BIGINT,
            COALESCE(SUM(pi.socso_employee), 0)::BIGINT,
            COALESCE(SUM(pi.eis_employee), 0)::BIGINT,
            COALESCE(SUM(pi.zakat_amount), 0)::BIGINT,
            COALESCE(SUM(pi.net_salary), 0)::BIGINT
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        WHERE pi.employee_id = $1
          AND pr.period_year = $2
          AND pr.period_month < $3
          AND pr.status NOT IN ('cancelled', 'draft')"#,
    )
    .bind(employee_id)
    .bind(year)
    .bind(current_month)
    .fetch_one(pool)
    .await?;

    Ok(result)
}

fn calculate_age(dob: Option<NaiveDate>, as_of: NaiveDate) -> i32 {
    match dob {
        Some(dob) => {
            let mut age = as_of.year() - dob.year();
            if as_of.ordinal() < dob.ordinal() {
                age -= 1;
            }
            age
        }
        None => 30, // default assumption if DOB not provided
    }
}
