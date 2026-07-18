use chrono::{Datelike, NaiveDate};
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{Instrument, info, info_span};
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::employee::Employee;
use crate::models::payroll::{BulkPayrollData, PayrollItem, PayrollRun};
use crate::models::statutory::PcbInput;
use crate::repositories::reads::payroll as payroll_reads;
use crate::repositories::{
    claims, employees as employee_repo, payroll_entries, payroll_items, payroll_runs, tp3_records,
};
use crate::services::audit_service::AuditRequestMeta;
use crate::services::eis_service;
use crate::services::epf_service;
use crate::services::pcb_calculator;
use crate::services::socso_service;
use crate::services::statutory_rules;

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
    let existing =
        payroll_runs::count_active_for_period(pool, company_id, payroll_group_id, year, month)
            .await?;

    if existing > 0 {
        return Err(AppError::Conflict(
            "Payroll already exists for this period. Delete the eligible existing run first."
                .into(),
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
    let employees = employee_repo::list_for_payroll_run(
        pool,
        company_id,
        payroll_group_id,
        period_end,
        period_start,
    )
    .await?;

    if employees.is_empty() {
        return Err(AppError::BadRequest(
            "No active employees found in this payroll group for the selected period".into(),
        ));
    }

    // Validate the four statutory domains once per run. Individual lookups
    // remain linked to verified rule-set IDs, so this avoids four extra
    // metadata queries for every employee without weakening fail-closed use.
    statutory_rules::require_all_verified(pool, effective_date).await?;

    tracing::Span::current().record("employee_count", employees.len());
    info!(employees = employees.len(), "starting payroll run");

    // Begin transaction
    let mut tx = pool.begin().await?;

    // Create payroll run
    let run_id = Uuid::now_v7();
    tracing::Span::current().record("run_id", tracing::field::display(run_id));
    let insert_result = payroll_runs::insert_processing(
        &mut *tx,
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
    .await;

    if let Err(err) = insert_result {
        let duplicate_period = matches!(
            &err,
            AppError::Database(sqlx::Error::Database(db_err))
                if matches!(
                    db_err.constraint(),
                    Some("payroll_runs_one_active_period")
                        | Some("payroll_runs_company_id_payroll_group_id_period_year_period_key")
                )
        );
        if duplicate_period {
            return Err(AppError::Conflict(
                "Payroll already exists for this period. Delete the eligible existing run first."
                    .into(),
            ));
        }
        return Err(err);
    }

    let employee_ids: Vec<Uuid> = employees.iter().map(|e| e.id).collect();

    // 1. Batch fetch recurring allowances and deductions
    let mut recurring_allowances_map = HashMap::new();
    let mut recurring_deductions_map = HashMap::new();
    let allowances =
        payroll_reads::recurring_allowance_totals(&mut *tx, &employee_ids, effective_date).await?;

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
    let entries =
        payroll_reads::entry_category_totals(&mut *tx, &employee_ids, year, month).await?;

    for row in entries {
        if row.category == "earning" {
            variable_earnings_map.insert(row.employee_id, row.total);
        } else {
            variable_deductions_map.insert(row.employee_id, row.total);
        }
    }

    let monthly_allowances =
        payroll_reads::monthly_allowance_totals(&mut *tx, &employee_ids, year, month).await?;

    for row in monthly_allowances {
        monthly_allowances_map.insert(row.employee_id, row.total);
    }

    // 3. Batch fetch attendance OT hours
    let ot_hours =
        payroll_reads::attendance_ot_hours(&mut *tx, &employee_ids, period_start, period_end)
            .await?;
    let attendance_ot_map: HashMap<Uuid, f64> = ot_hours
        .into_iter()
        .map(|r| (r.employee_id, r.hours))
        .collect();

    // 3b. Batch fetch approved overtime applications
    let approved_ot =
        payroll_reads::approved_ot_totals(&mut *tx, &employee_ids, period_start, period_end)
            .await?;
    let mut approved_ot_map: HashMap<Uuid, Vec<(String, f64)>> = HashMap::new();
    for row in approved_ot {
        approved_ot_map
            .entry(row.employee_id)
            .or_default()
            .push((row.ot_type, row.hours));
    }

    // 3c. Batch fetch approved claims
    let approved_claims = payroll_reads::approved_claim_totals(
        &mut *tx,
        &employee_ids,
        company_id,
        period_start,
        period_end,
    )
    .await?;
    let claims_map: HashMap<Uuid, i64> = approved_claims
        .into_iter()
        .map(|r| (r.employee_id, r.total))
        .collect();

    // 4. Batch fetch TP3 data
    let tp3_data = tp3_records::list_ytd_for_employees(&mut *tx, &employee_ids, year).await?;
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
    let ytd_data = payroll_reads::payroll_ytd(&mut *tx, &employee_ids, year, month).await?;
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
    payroll_runs::update_totals(
        &mut *tx,
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
    let run = payroll_runs::get_by_id(pool, run_id)
        .await?
        .ok_or_else(|| AppError::Internal("Payroll run not found after creation".into()))?;

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
    let epf =
        epf_service::calculate_epf_after_preflight(pool, gross, &epf_category, effective_date)
            .await?;

    // SOCSO
    let socso = socso_service::calculate_socso_after_preflight(
        pool,
        gross,
        age,
        is_foreigner,
        effective_date,
    )
    .await?;

    // EIS
    let eis =
        eis_service::calculate_eis_after_preflight(pool, gross, age, is_foreigner, effective_date)
            .await?;

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

    let pcb =
        pcb_calculator::calculate_pcb_after_preflight(pool, &pcb_input, effective_date).await?;

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
    let item = payroll_items::insert(
        &mut **tx,
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
    .await?;

    // Mark staged entries as processed
    payroll_entries::mark_processed(&mut **tx, run_id, emp.id, year, month).await?;

    // Mark approved claims as processed
    if total_claims > 0 {
        claims::mark_processed(
            &mut **tx,
            emp.id,
            emp.company_id,
            _period_start,
            _period_end,
        )
        .await?;
    }

    Ok(item)
}

fn calculate_age(dob: Option<NaiveDate>, as_of: NaiveDate) -> i32 {
    match dob {
        Some(dob) => {
            let mut age = as_of.year() - dob.year();
            if (as_of.month(), as_of.day()) < (dob.month(), dob.day()) {
                age -= 1;
            }
            age
        }
        None => 30, // default assumption if DOB not provided
    }
}

#[cfg(test)]
mod tests {
    use super::calculate_age;
    use chrono::NaiveDate;

    #[test]
    fn calculates_age_on_and_before_birthday() {
        let dob = NaiveDate::from_ymd_opt(1990, 7, 1);
        assert_eq!(
            calculate_age(dob, NaiveDate::from_ymd_opt(2023, 6, 30).unwrap()),
            32
        );
        assert_eq!(
            calculate_age(dob, NaiveDate::from_ymd_opt(2023, 7, 1).unwrap()),
            33
        );
    }

    #[test]
    fn leap_year_day_offset_does_not_advance_age_early() {
        let dob = NaiveDate::from_ymd_opt(1990, 7, 1);
        let leap_year_day_before_birthday = NaiveDate::from_ymd_opt(2024, 6, 30).unwrap();

        assert_eq!(calculate_age(dob, leap_year_day_before_birthday), 33);
    }

    #[test]
    fn february_29_birthday_advances_on_march_1_in_non_leap_year() {
        let dob = NaiveDate::from_ymd_opt(2000, 2, 29);
        assert_eq!(
            calculate_age(dob, NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()),
            25
        );
        assert_eq!(
            calculate_age(dob, NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()),
            26
        );
    }

    #[test]
    fn missing_birth_date_keeps_documented_default_age() {
        assert_eq!(
            calculate_age(None, NaiveDate::from_ymd_opt(2026, 7, 14).unwrap()),
            30
        );
    }
}
