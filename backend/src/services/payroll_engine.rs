use chrono::{Datelike, NaiveDate};
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{Instrument, info, info_span};
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::employee::Employee;
use crate::models::payroll::{PayrollItem, PayrollRun};
use crate::services::audit_service::AuditRequestMeta;
use crate::services::eis_service;
use crate::services::epf_service;
use crate::services::pcb_calculator::{self, PcbInput};
use crate::services::socso_service;

/// Process payroll for a group in a given period.
///
/// 1. Fetch all active employees in the payroll group
/// 2. For each employee, calculate gross, statutory deductions, net
/// 3. Create PayrollRun + PayrollItems in a transaction
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[tracing::instrument(
    name = "payroll.process",
    skip(pool, notes),
    fields(
        company_id = %company_id,
        payroll_group_id = %payroll_group_id,
        year,
        month,
        run_id = tracing::field::Empty,
        employee_count = tracing::field::Empty,
    ),
)]
pub async fn process_payroll(
    pool: &PgPool,
    company_id: Uuid,
    payroll_group_id: Uuid,
    year: i32,
    month: i32,
    pay_date: NaiveDate,
    processed_by: Uuid,
    notes: Option<String>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollRun> {
    // Check for existing run
    let existing = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM payroll_runs
        WHERE company_id = $1 AND payroll_group_id = $2
        AND period_year = $3 AND period_month = $4
        AND status NOT IN ('cancelled')"#,
        company_id,
        payroll_group_id,
        year,
        month,
    )
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
    let employees = sqlx::query_as!(
        Employee,
        r#"SELECT id, company_id, employee_number, full_name, ic_number, passport_number,
            date_of_birth, gender::text AS "gender?", nationality, race::text AS "race?",
            residency_status::text AS "residency_status!", marital_status::text AS "marital_status?",
            email, phone, address_line1, address_line2, city, state, postcode,
            department, designation, cost_centre, branch, employment_type::text AS "employment_type!",
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
        company_id,
        payroll_group_id,
        period_end,
        period_start,
    )
    .fetch_all(pool)
    .await?;

    if employees.is_empty() {
        return Err(AppError::BadRequest(
            "No active employees found in this payroll group for the selected period".into(),
        ));
    }

    tracing::Span::current().record("employee_count", employees.len());
    info!(employees = employees.len(), "starting payroll run");

    // Begin transaction
    let mut tx = pool.begin().await?;

    // Create payroll run
    let run_id = Uuid::now_v7();
    tracing::Span::current().record("run_id", tracing::field::display(run_id));
    sqlx::query!(
        r#"INSERT INTO payroll_runs
        (id, company_id, payroll_group_id, period_year, period_month,
         period_start, period_end, pay_date, status, processed_by, processed_at, notes, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'processing', $9, NOW(), $10, $9)"#,
        run_id,
        company_id,
        payroll_group_id,
        year,
        month,
        period_start,
        period_end,
        pay_date,
        processed_by,
        notes,
    )
    .execute(&mut *tx)
    .await?;

    let employee_ids: Vec<Uuid> = employees.iter().map(|e| e.id).collect();

    // 1. Batch fetch recurring allowances and deductions
    let mut recurring_allowances_map = HashMap::new();
    let mut recurring_deductions_map = HashMap::new();
    let allowances = sqlx::query!(
        r#"SELECT employee_id, category, SUM(amount)::BIGINT AS "total!"
           FROM employee_allowances
           WHERE employee_id = ANY($1) AND is_active = TRUE AND is_recurring = TRUE
             AND effective_from <= $2 AND (effective_to IS NULL OR effective_to >= $2)
           GROUP BY employee_id, category"#,
        &employee_ids,
        effective_date,
    )
    .fetch_all(&mut *tx)
    .await?;

    for row in allowances {
        if row.category == "earning" {
            recurring_allowances_map.insert(row.employee_id, row.total);
        } else {
            recurring_deductions_map.insert(row.employee_id, row.total);
        }
    }

    // 2. Batch fetch staged payroll entries
    let mut monthly_allowances_map = HashMap::new();
    let mut variable_earnings_map = HashMap::new();
    let mut variable_deductions_map = HashMap::new();
    let entries = sqlx::query!(
        r#"SELECT employee_id, category, SUM(amount)::BIGINT AS "total!"
           FROM payroll_entries
           WHERE employee_id = ANY($1) AND period_year = $2 AND period_month = $3
             AND is_processed = FALSE
           GROUP BY employee_id, category"#,
        &employee_ids,
        year,
        month,
    )
    .fetch_all(&mut *tx)
    .await?;

    for row in entries {
        if row.category == "earning" {
            variable_earnings_map.insert(row.employee_id, row.total);
        } else {
            variable_deductions_map.insert(row.employee_id, row.total);
        }
    }

    let monthly_allowances = sqlx::query!(
        r#"SELECT employee_id, SUM(amount)::BIGINT AS "total!"
           FROM payroll_entries
           WHERE employee_id = ANY($1) AND period_year = $2 AND period_month = $3
             AND is_processed = FALSE
             AND category = 'earning'
             AND item_type IN ('allowance', 'monthly_allowance')
           GROUP BY employee_id"#,
        &employee_ids,
        year,
        month,
    )
    .fetch_all(&mut *tx)
    .await?;

    for row in monthly_allowances {
        monthly_allowances_map.insert(row.employee_id, row.total);
    }

    // 3. Batch fetch attendance OT hours
    let ot_hours = sqlx::query!(
        r#"SELECT ar.employee_id, SUM(ar.overtime_hours)::FLOAT AS "hours!"
           FROM attendance_records ar
           LEFT JOIN overtime_applications oa
               ON ar.employee_id = oa.employee_id
               AND DATE(ar.check_in_at) = oa.ot_date
               AND oa.status = 'approved'
           WHERE ar.employee_id = ANY($1)
             AND ar.check_in_at >= $2::date AND ar.check_in_at <= $3::date + INTERVAL '1 day'
             AND oa.id IS NULL
           GROUP BY ar.employee_id"#,
        &employee_ids,
        period_start,
        period_end,
    )
    .fetch_all(&mut *tx)
    .await?;
    let attendance_ot_map: HashMap<Uuid, f64> = ot_hours
        .into_iter()
        .map(|r| (r.employee_id, r.hours))
        .collect();

    // 3b. Batch fetch approved overtime applications
    let approved_ot = sqlx::query!(
        r#"SELECT employee_id, ot_type, SUM(hours)::FLOAT AS "hours!"
           FROM overtime_applications
           WHERE employee_id = ANY($1)
             AND ot_date >= $2 AND ot_date <= $3
             AND status = 'approved'
           GROUP BY employee_id, ot_type"#,
        &employee_ids,
        period_start,
        period_end,
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut approved_ot_map: HashMap<Uuid, Vec<(String, f64)>> = HashMap::new();
    for row in approved_ot {
        approved_ot_map
            .entry(row.employee_id)
            .or_default()
            .push((row.ot_type, row.hours));
    }

    // 3c. Batch fetch approved claims
    let approved_claims = sqlx::query!(
        r#"SELECT employee_id, SUM(amount)::BIGINT AS "total!"
           FROM claims
           WHERE employee_id = ANY($1)
             AND company_id = $2
             AND status = 'approved'
             AND expense_date >= $3 AND expense_date <= $4
           GROUP BY employee_id"#,
        &employee_ids,
        company_id,
        period_start,
        period_end,
    )
    .fetch_all(&mut *tx)
    .await?;
    let claims_map: HashMap<Uuid, i64> = approved_claims
        .into_iter()
        .map(|r| (r.employee_id, r.total))
        .collect();

    // 4. Batch fetch TP3 data
    let tp3_data = sqlx::query!(
        r#"SELECT employee_id, previous_income_ytd, previous_epf_ytd, previous_pcb_ytd, previous_zakat_ytd
           FROM tp3_records WHERE employee_id = ANY($1) AND tax_year = $2"#,
        &employee_ids,
        year,
    )
    .fetch_all(&mut *tx)
    .await?;
    let tp3_map: HashMap<Uuid, (i64, i64, i64, i64)> = tp3_data
        .into_iter()
        .map(|r| {
            (
                r.employee_id,
                (
                    r.previous_income_ytd,
                    r.previous_epf_ytd,
                    r.previous_pcb_ytd,
                    r.previous_zakat_ytd,
                ),
            )
        })
        .collect();

    // 5. Batch fetch YTD figures
    let ytd_data = sqlx::query!(
        r#"SELECT
            pi.employee_id,
            COALESCE(SUM(pi.gross_salary), 0)::BIGINT AS "gross!",
            COALESCE(SUM(pi.pcb_amount), 0)::BIGINT AS "pcb!",
            COALESCE(SUM(pi.epf_employee), 0)::BIGINT AS "epf!",
            COALESCE(SUM(pi.socso_employee), 0)::BIGINT AS "socso!",
            COALESCE(SUM(pi.eis_employee), 0)::BIGINT AS "eis!",
            COALESCE(SUM(pi.zakat_amount), 0)::BIGINT AS "zakat!",
            COALESCE(SUM(pi.net_salary), 0)::BIGINT AS "net!"
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        WHERE pi.employee_id = ANY($1) AND pr.period_year = $2 AND pr.period_month < $3
        AND pr.status::text IN ('processed', 'pending_approval', 'approved', 'paid')
        GROUP BY pi.employee_id"#,
        &employee_ids,
        year,
        month,
    )
    .fetch_all(&mut *tx)
    .await?;
    let ytd_map: HashMap<Uuid, (i64, i64, i64, i64, i64, i64, i64)> = ytd_data
        .into_iter()
        .map(|r| {
            (
                r.employee_id,
                (r.gross, r.pcb, r.epf, r.socso, r.eis, r.zakat, r.net),
            )
        })
        .collect();

    let bulk_data = BulkPayrollData {
        recurring_allowances: recurring_allowances_map,
        recurring_deductions: recurring_deductions_map,
        variable_earnings: variable_earnings_map,
        variable_deductions: variable_deductions_map,
        attendance_ot_hours: attendance_ot_map,
        approved_ot: approved_ot_map,
        approved_claims: claims_map,
        tp3: tp3_map,
        ytd: ytd_map,
        monthly_allowances: monthly_allowances_map,
    };

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
        let emp_span = info_span!("payroll.employee", employee_id = %emp.id);
        let item = process_employee(
            pool,
            &mut tx,
            run_id,
            emp,
            year,
            month,
            period_start,
            period_end,
            effective_date,
            &bulk_data,
        )
        .instrument(emp_span)
        .await?;

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
    sqlx::query!(
        r#"UPDATE payroll_runs SET
        status = 'processed',
        total_gross = $2, total_net = $3, total_employer_cost = $4,
        total_epf_employee = $5, total_epf_employer = $6,
        total_socso_employee = $7, total_socso_employer = $8,
        total_eis_employee = $9, total_eis_employer = $10,
        total_pcb = $11, total_zakat = $12,
        employee_count = $13, updated_at = NOW()
        WHERE id = $1"#,
        run_id,
        total_gross,
        total_net,
        total_employer_cost,
        total_epf_ee,
        total_epf_er,
        total_socso_ee,
        total_socso_er,
        total_eis_ee,
        total_eis_er,
        total_pcb,
        total_zakat,
        employees.len() as i32,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    info!(
        total_gross,
        total_net, total_pcb, total_employer_cost, "payroll run committed"
    );

    // Audit Log
    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(processed_by),
        "process",
        "payroll_run",
        Some(run_id),
        None,
        Some(serde_json::json!({
            "year": year,
            "month": month,
            "total_gross": total_gross,
            "total_net": total_net,
            "employee_count": employees.len()
        })),
        Some(&format!("Processed payroll for {:02}/{}", month, year)),
        audit_meta,
    )
    .await;

    // Return the completed run
    let run = sqlx::query_as!(
        PayrollRun,
        r#"SELECT id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text AS "status!",
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by
        FROM payroll_runs WHERE id = $1"#,
        run_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(run)
}

/// Process a single employee's payroll
#[allow(clippy::too_many_arguments)]
async fn process_employee(
    pool: &PgPool,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_id: Uuid,
    emp: &Employee,
    year: i32,
    month: i32,
    _period_start: NaiveDate,
    _period_end: NaiveDate,
    effective_date: NaiveDate,
    bulk: &BulkPayrollData,
) -> AppResult<PayrollItem> {
    // Calculate age
    let age = calculate_age(emp.date_of_birth, effective_date);
    let is_foreigner = emp.residency_status == "foreigner";
    let epf_category = emp.epf_category.clone().unwrap_or_else(|| "A".to_string());

    // Gross salary = basic + recurring allowances + overtime
    let basic = emp.basic_salary;

    let allowances_total = *bulk.recurring_allowances.get(&emp.id).unwrap_or(&0);
    let monthly_allowances = *bulk.monthly_allowances.get(&emp.id).unwrap_or(&0);
    let variable_earnings = *bulk.variable_earnings.get(&emp.id).unwrap_or(&0);
    let variable_deductions = *bulk.variable_deductions.get(&emp.id).unwrap_or(&0);
    let recurring_deductions = *bulk.recurring_deductions.get(&emp.id).unwrap_or(&0);
    let attendance_ot_hours = *bulk.attendance_ot_hours.get(&emp.id).unwrap_or(&0.0);

    // Calculate hourly rate once for OT computations
    let hourly_rate = emp.hourly_rate.unwrap_or({
        // Default calculation: basic / 26 days / 8 hours
        emp.basic_salary / 26 / 8
    });

    // Attendance-based OT (records without approved OT applications)
    let attendance_ot_pay = if attendance_ot_hours > 0.0 {
        (hourly_rate as f64 * 1.5 * attendance_ot_hours) as i64
    } else {
        0
    };

    // Approved OT applications with type-based rate multipliers
    let approved_ot_pay = if let Some(ot_entries) = bulk.approved_ot.get(&emp.id) {
        let mut total = 0i64;
        for (ot_type, hours) in ot_entries {
            let multiplier = match ot_type.as_str() {
                "rest_day" => 2.0,
                "public_holiday" => 3.0,
                _ => 1.5, // normal
            };
            total += (hourly_rate as f64 * multiplier * hours) as i64;
        }
        total
    } else {
        0
    };

    let total_overtime = attendance_ot_pay + approved_ot_pay;

    // Approved claims (reimbursements, not part of gross — added to net)
    let total_claims = *bulk.approved_claims.get(&emp.id).unwrap_or(&0);

    let gross = basic + allowances_total + variable_earnings + total_overtime;
    let total_allowances = allowances_total + monthly_allowances;

    // EPF
    let epf = epf_service::calculate_epf(pool, gross, &epf_category, effective_date).await?;

    // SOCSO
    let socso =
        socso_service::calculate_socso(pool, gross, age, is_foreigner, effective_date).await?;

    // EIS
    let eis = eis_service::calculate_eis(pool, gross, age, is_foreigner, effective_date).await?;

    // Get YTD figures (from previous months this year)
    let (ytd_gross, ytd_pcb, ytd_epf, ytd_socso, ytd_eis, ytd_zakat, ytd_net) =
        *bulk.ytd.get(&emp.id).unwrap_or(&(0, 0, 0, 0, 0, 0, 0));

    // Get TP3 data if exists
    let (tp3_income, tp3_epf, tp3_pcb, tp3_zakat) = *bulk.tp3.get(&emp.id).unwrap_or(&(0, 0, 0, 0));

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
        marital_status: emp
            .marital_status
            .clone()
            .unwrap_or_else(|| "single".into()),
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

    let net = gross - total_deductions + total_claims;
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
    let item_id = Uuid::now_v7();
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
        item_id,
        run_id,
        emp.id,
        basic,
        gross,
        total_allowances,
        total_overtime,
        total_claims,
        epf.employee,
        epf.employer,
        socso.employee,
        socso.employer,
        eis.employee,
        eis.employer,
        pcb,
        zakat,
        ptptn,
        tabung_haji,
        recurring_deductions + variable_deductions,
        total_deductions,
        net,
        employer_cost,
        new_ytd_gross,
        new_ytd_epf,
        new_ytd_pcb,
        new_ytd_socso,
        new_ytd_eis,
        new_ytd_zakat,
        new_ytd_net,
    )
    .fetch_one(&mut **tx)
    .await?;

    // Mark staged entries as processed
    sqlx::query!(
        r#"UPDATE payroll_entries SET is_processed = TRUE, payroll_run_id = $1
        WHERE employee_id = $2 AND period_year = $3 AND period_month = $4 AND is_processed = FALSE"#,
        run_id,
        emp.id,
        year,
        month,
    )
    .execute(&mut **tx)
    .await?;

    // Mark approved claims as processed
    if total_claims > 0 {
        sqlx::query!(
            r#"UPDATE claims SET status = 'processed', updated_at = NOW()
            WHERE employee_id = $1 AND company_id = $2
              AND status = 'approved'
              AND expense_date >= $3 AND expense_date <= $4"#,
            emp.id,
            emp.company_id,
            _period_start,
            _period_end,
        )
        .execute(&mut **tx)
        .await?;
    }

    Ok(item)
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
#[allow(clippy::type_complexity)]
struct BulkPayrollData {
    recurring_allowances: HashMap<Uuid, i64>,
    recurring_deductions: HashMap<Uuid, i64>,
    variable_earnings: HashMap<Uuid, i64>,
    variable_deductions: HashMap<Uuid, i64>,
    attendance_ot_hours: HashMap<Uuid, f64>,
    approved_ot: HashMap<Uuid, Vec<(String, f64)>>,
    approved_claims: HashMap<Uuid, i64>,
    tp3: HashMap<Uuid, (i64, i64, i64, i64)>,
    ytd: HashMap<Uuid, (i64, i64, i64, i64, i64, i64, i64)>,
    monthly_allowances: HashMap<Uuid, i64>,
}
