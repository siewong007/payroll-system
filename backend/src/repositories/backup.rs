//! Data access for whole-company backup export/import. Unlike the per-table
//! modules this is a use-case module (like `clock`): it spans every company-owned
//! table, reading/writing the bespoke `*Export` projections in `models::backup`.
//! All ID remapping, file (de)serialization, and the import transaction stay in
//! `services::backup_service`; this module only holds the SQL.

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::backup::*;

// ─── Export reads (one company-scoped projection per table) ───

pub async fn company(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Option<CompanyExport>> {
    let company = sqlx::query_as!(
        CompanyExport,
        r#"SELECT id, name, registration_number, tax_number, epf_number, socso_code, eis_code,
                  hrdf_number, address_line1, address_line2, city, state, postcode, country,
                  phone, email, logo_url, hrdf_enabled, unpaid_leave_divisor, is_active,
                  created_at, updated_at
           FROM companies WHERE id = $1"#,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(company)
}

pub async fn payroll_groups(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<PayrollGroupExport>> {
    let rows = sqlx::query_as!(
        PayrollGroupExport,
        r#"SELECT id, company_id, name, description, cutoff_day, payment_day, is_active,
                  created_at, updated_at
           FROM payroll_groups WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn employees(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<EmployeeExport>> {
    let rows = sqlx::query_as!(
        EmployeeExport,
        r#"SELECT id, company_id, employee_number, full_name, ic_number, passport_number,
                  date_of_birth, gender::text AS "gender?", nationality, race::text AS "race?", residency_status::text AS "residency_status!", marital_status::text AS "marital_status?",
                  email, phone, address_line1, address_line2, city, state, postcode,
                  department, designation, cost_centre, branch, employment_type::text AS "employment_type!",
                  date_joined, probation_start, probation_end, confirmation_date,
                  date_resigned, resignation_reason,
                  basic_salary, hourly_rate, daily_rate,
                  bank_name, bank_account_number, bank_account_type,
                  tax_identification_number, epf_number, socso_number, eis_number,
                  working_spouse, num_children, epf_category,
                  is_muslim, zakat_eligible, zakat_monthly_amount, ptptn_monthly_amount, tabung_haji_amount,
                  hrdf_contribution, payroll_group_id, salary_group,
                  is_active, deleted_at, created_at, updated_at
           FROM employees WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn employee_allowances(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<EmployeeAllowanceExport>> {
    let rows = sqlx::query_as!(
        EmployeeAllowanceExport,
        r#"SELECT ea.id, ea.employee_id, ea.category, ea.name, ea.description, ea.amount,
                  ea.is_taxable, ea.is_recurring, ea.effective_from, ea.effective_to,
                  ea.is_active, ea.created_at, ea.updated_at
           FROM employee_allowances ea
           JOIN employees e ON ea.employee_id = e.id
           WHERE e.company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn salary_history(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<SalaryHistoryExport>> {
    let rows = sqlx::query_as!(
        SalaryHistoryExport,
        r#"SELECT sh.id, sh.employee_id, sh.old_salary, sh.new_salary,
                  sh.effective_date, sh.reason, sh.created_at
           FROM salary_history sh
           JOIN employees e ON sh.employee_id = e.id
           WHERE e.company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn tp3_records(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<Tp3RecordExport>> {
    let rows = sqlx::query_as!(
        Tp3RecordExport,
        r#"SELECT t.id, t.employee_id, t.tax_year, t.previous_employer_name,
                  t.previous_income_ytd, t.previous_epf_ytd, t.previous_pcb_ytd,
                  t.previous_socso_ytd, t.previous_zakat_ytd, t.created_at
           FROM tp3_records t
           JOIN employees e ON t.employee_id = e.id
           WHERE e.company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn leave_types(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<LeaveTypeExport>> {
    let rows = sqlx::query_as!(
        LeaveTypeExport,
        r#"SELECT id, company_id, name, description, default_days, is_paid, is_active,
                  created_at, updated_at
           FROM leave_types WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn leave_balances(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<LeaveBalanceExport>> {
    let rows = sqlx::query_as!(
        LeaveBalanceExport,
        r#"SELECT lb.id, lb.employee_id, lb.leave_type_id, lb.year,
                  lb.entitled_days, lb.taken_days, lb.pending_days, lb.carried_forward,
                  lb.created_at, lb.updated_at
           FROM leave_balances lb
           JOIN employees e ON lb.employee_id = e.id
           WHERE e.company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn leave_requests(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<LeaveRequestExport>> {
    let rows = sqlx::query_as!(
        LeaveRequestExport,
        r#"SELECT id, employee_id, company_id, leave_type_id, start_date, end_date, days,
                  reason, status, review_notes, attachment_url, attachment_name,
                  created_at, updated_at
           FROM leave_requests WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn claims(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<ClaimExport>> {
    let rows = sqlx::query_as!(
        ClaimExport,
        r#"SELECT id, employee_id, company_id, title, description, amount, category,
                  receipt_url, receipt_file_name, expense_date, status,
                  submitted_at, review_notes, created_at, updated_at
           FROM claims WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn overtime_applications(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<OvertimeExport>> {
    let rows = sqlx::query_as!(
        OvertimeExport,
        r#"SELECT id, employee_id, company_id, ot_date, start_time, end_time, hours,
                  ot_type, reason, status, review_notes, created_at, updated_at
           FROM overtime_applications WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn payroll_runs(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<PayrollRunExport>> {
    let rows = sqlx::query_as!(
        PayrollRunExport,
        r#"SELECT id, company_id, payroll_group_id, period_year, period_month,
                  period_start, period_end, pay_date, status::text AS "status!",
                  total_gross, total_net, total_employer_cost,
                  total_epf_employee, total_epf_employer,
                  total_socso_employee, total_socso_employer,
                  total_eis_employee, total_eis_employer,
                  total_pcb, total_zakat, employee_count, version, notes,
                  created_at, updated_at
           FROM payroll_runs WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn payroll_items(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<PayrollItemExport>> {
    let rows = sqlx::query_as!(
        PayrollItemExport,
        r#"SELECT pi.id, pi.payroll_run_id, pi.employee_id,
                  pi.basic_salary, pi.gross_salary, pi.total_allowances, pi.total_overtime,
                  pi.total_bonus, pi.total_commission, pi.total_claims,
                  pi.epf_employee, pi.epf_employer, pi.socso_employee, pi.socso_employer,
                  pi.eis_employee, pi.eis_employer, pi.pcb_amount, pi.zakat_amount,
                  pi.ptptn_amount, pi.tabung_haji_amount,
                  pi.total_loan_deductions, pi.total_other_deductions,
                  pi.unpaid_leave_deduction, pi.unpaid_leave_days,
                  pi.total_deductions, pi.net_salary, pi.employer_cost,
                  pi.ytd_gross, pi.ytd_epf_employee, pi.ytd_pcb,
                  pi.ytd_socso_employee, pi.ytd_eis_employee, pi.ytd_zakat, pi.ytd_net,
                  pi.working_days, pi.days_worked, pi.is_prorated,
                  pi.created_at, pi.updated_at
           FROM payroll_items pi
           JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
           WHERE pr.company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn payroll_item_details(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<PayrollItemDetailExport>> {
    let rows = sqlx::query_as!(
        PayrollItemDetailExport,
        r#"SELECT pid.id, pid.payroll_item_id, pid.category, pid.item_type,
                  pid.description, pid.amount, pid.is_taxable, pid.is_statutory, pid.created_at
           FROM payroll_item_details pid
           JOIN payroll_items pi ON pid.payroll_item_id = pi.id
           JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
           WHERE pr.company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn payroll_entries(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<PayrollEntryExport>> {
    let rows = sqlx::query_as!(
        PayrollEntryExport,
        r#"SELECT id, employee_id, company_id, period_year, period_month,
                  category, item_type, description AS "description?", amount, quantity, rate,
                  is_taxable, is_processed, payroll_run_id, created_at, updated_at
           FROM payroll_entries WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn document_categories(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<DocumentCategoryExport>> {
    let rows = sqlx::query_as!(
        DocumentCategoryExport,
        r#"SELECT id, company_id, name, description, is_active, created_at
           FROM document_categories WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn documents(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<DocumentExport>> {
    let rows = sqlx::query_as!(
        DocumentExport,
        r#"SELECT id, company_id, employee_id, category_id, title, description,
                  file_name, file_url, file_size, mime_type, status::text AS "status!",
                  issue_date, expiry_date, is_confidential, tags,
                  deleted_at, created_at, updated_at
           FROM documents WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn teams(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<TeamExport>> {
    let rows = sqlx::query_as!(
        TeamExport,
        r#"SELECT id, company_id, name, description, tag, is_active, created_at, updated_at
           FROM teams WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn team_members(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<TeamMemberExport>> {
    let rows = sqlx::query_as!(
        TeamMemberExport,
        r#"SELECT tm.id, tm.team_id, tm.employee_id, tm.role, tm.joined_at
           FROM team_members tm
           JOIN teams t ON tm.team_id = t.id
           WHERE t.company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn holidays(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<HolidayExport>> {
    let rows = sqlx::query_as!(
        HolidayExport,
        r#"SELECT id, company_id, name, date, holiday_type, description, is_recurring, state,
                  created_at, updated_at
           FROM holidays WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn working_day_config(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<WorkingDayConfigExport>> {
    let rows = sqlx::query_as!(
        WorkingDayConfigExport,
        r#"SELECT id, company_id, day_of_week, is_working_day, created_at, updated_at
           FROM working_day_config WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn email_templates(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<EmailTemplateExport>> {
    let rows = sqlx::query_as!(
        EmailTemplateExport,
        r#"SELECT id, company_id, name, letter_type, subject, body_html, is_active AS "is_active?",
                  created_at, updated_at
           FROM email_templates WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn company_settings(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<CompanySettingExport>> {
    let rows = sqlx::query_as!(
        CompanySettingExport,
        r#"SELECT id, company_id, category, key, value, label, description, updated_at
           FROM company_settings WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

// ─── Import writes ───
//
// NOTE: the INSERT/UPDATE bodies below keep their original (over-)indentation so
// their text is byte-identical to the offline `.sqlx` cache. The service owns ID
// remapping, the `now` timestamp, file restore, and the import transaction.

/// Return the explicit import target's name. Restore callers must choose this
/// target rather than deriving one from untrusted backup metadata.
pub async fn company_name(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Option<String>> {
    let name = sqlx::query_scalar::<_, String>("SELECT name FROM companies WHERE id = $1")
        .bind(company_id)
        .fetch_optional(executor)
        .await?;
    Ok(name)
}

/// Whether a company name is already in use, case-insensitively. A new-company
/// restore must not silently turn into an overwrite just because names match.
pub async fn company_name_exists(
    executor: impl Executor<'_, Database = Postgres>,
    name: &str,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM companies WHERE LOWER(name) = LOWER($1))",
    )
    .bind(name)
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// Wipe all data for a company in FK-safe order (import overwrite). Runs many
/// statements, so it takes the caller's transaction connection directly.
pub async fn delete_company_cascade(
    conn: &mut sqlx::PgConnection,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM company_settings WHERE company_id = $1",
        company_id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "DELETE FROM email_templates WHERE company_id = $1",
        company_id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "DELETE FROM working_day_config WHERE company_id = $1",
        company_id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM holidays WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "DELETE FROM team_members WHERE team_id IN (SELECT id FROM teams WHERE company_id = $1)",
        company_id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM teams WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!("DELETE FROM documents WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "DELETE FROM document_categories WHERE company_id = $1",
        company_id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "DELETE FROM payroll_entries WHERE company_id = $1",
        company_id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM payroll_item_details WHERE payroll_item_id IN (SELECT pi.id FROM payroll_items pi JOIN payroll_runs pr ON pi.payroll_run_id = pr.id WHERE pr.company_id = $1)", company_id).execute(&mut *conn).await?;
    sqlx::query!("DELETE FROM payroll_items WHERE payroll_run_id IN (SELECT id FROM payroll_runs WHERE company_id = $1)", company_id).execute(&mut *conn).await?;
    sqlx::query!("DELETE FROM payroll_runs WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "DELETE FROM overtime_applications WHERE company_id = $1",
        company_id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM claims WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "DELETE FROM leave_requests WHERE company_id = $1",
        company_id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM leave_balances WHERE employee_id IN (SELECT id FROM employees WHERE company_id = $1)", company_id).execute(&mut *conn).await?;
    sqlx::query!("DELETE FROM leave_types WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!("DELETE FROM tp3_records WHERE employee_id IN (SELECT id FROM employees WHERE company_id = $1)", company_id).execute(&mut *conn).await?;
    sqlx::query!("DELETE FROM salary_history WHERE employee_id IN (SELECT id FROM employees WHERE company_id = $1)", company_id).execute(&mut *conn).await?;
    sqlx::query!("DELETE FROM employee_allowances WHERE employee_id IN (SELECT id FROM employees WHERE company_id = $1)", company_id).execute(&mut *conn).await?;
    sqlx::query!("DELETE FROM employees WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "DELETE FROM payroll_groups WHERE company_id = $1",
        company_id
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn update_company(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    c: &CompanyExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE companies SET registration_number=$2, tax_number=$3, epf_number=$4, socso_code=$5,
               eis_code=$6, hrdf_number=$7, address_line1=$8, address_line2=$9, city=$10, state=$11,
               postcode=$12, country=$13, phone=$14, email=$15, logo_url=$16, hrdf_enabled=$17,
               unpaid_leave_divisor=$18, is_active=$19, updated_at=$20
               WHERE id = $1"#,
        id,
        c.registration_number,
        c.tax_number,
        c.epf_number,
        c.socso_code,
        c.eis_code,
        c.hrdf_number,
        c.address_line1,
        c.address_line2,
        c.city,
        c.state,
        c.postcode,
        c.country,
        c.phone,
        c.email,
        c.logo_url,
        c.hrdf_enabled,
        c.unpaid_leave_divisor,
        c.is_active,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_company(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    c: &CompanyExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO companies (id, name, registration_number, tax_number, epf_number, socso_code,
               eis_code, hrdf_number, address_line1, address_line2, city, state, postcode, country,
               phone, email, logo_url, hrdf_enabled, unpaid_leave_divisor, is_active, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22)"#,
        id,
        c.name,
        c.registration_number,
        c.tax_number,
        c.epf_number,
        c.socso_code,
        c.eis_code,
        c.hrdf_number,
        c.address_line1,
        c.address_line2,
        c.city,
        c.state,
        c.postcode,
        c.country,
        c.phone,
        c.email,
        c.logo_url,
        c.hrdf_enabled,
        c.unpaid_leave_divisor,
        c.is_active,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_payroll_group(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    pg: &PayrollGroupExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO payroll_groups (id, company_id, name, description, cutoff_day, payment_day,
               is_active, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
        id,
        company_id,
        pg.name,
        pg.description,
        pg.cutoff_day,
        pg.payment_day,
        pg.is_active,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_employee(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    payroll_group_id: Option<Uuid>,
    e: &EmployeeExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO employees (id, company_id, employee_number, full_name, ic_number, passport_number,
               date_of_birth, gender, nationality, race, residency_status, marital_status,
               email, phone, address_line1, address_line2, city, state, postcode,
               department, designation, cost_centre, branch, employment_type,
               date_joined, probation_start, probation_end, confirmation_date,
               date_resigned, resignation_reason,
               basic_salary, hourly_rate, daily_rate,
               bank_name, bank_account_number, bank_account_type,
               tax_identification_number, epf_number, socso_number, eis_number,
               working_spouse, num_children, epf_category,
               is_muslim, zakat_eligible, zakat_monthly_amount, ptptn_monthly_amount, tabung_haji_amount,
               hrdf_contribution, payroll_group_id, salary_group,
               is_active, deleted_at, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8::text::gender_type,$9,$10::text::race_type,$11::text::residency_status,$12::text::marital_status,$13,$14,$15,$16,$17,$18,$19,$20,
                       $21,$22,$23,$24::text::employment_type,$25,$26,$27,$28,$29,$30,$31,$32,$33,$34,$35,$36,$37,$38,
                       $39,$40,$41,$42,$43,$44,$45,$46,$47,$48,$49,$50,$51,$52,$53,$54,$55)"#,
        id,
        company_id,
        e.employee_number,
        e.full_name,
        e.ic_number,
        e.passport_number,
        e.date_of_birth,
        e.gender,
        e.nationality,
        e.race,
        e.residency_status,
        e.marital_status,
        e.email,
        e.phone,
        e.address_line1,
        e.address_line2,
        e.city,
        e.state,
        e.postcode,
        e.department,
        e.designation,
        e.cost_centre,
        e.branch,
        e.employment_type,
        e.date_joined,
        e.probation_start,
        e.probation_end,
        e.confirmation_date,
        e.date_resigned,
        e.resignation_reason,
        e.basic_salary,
        e.hourly_rate,
        e.daily_rate,
        e.bank_name,
        e.bank_account_number,
        e.bank_account_type,
        e.tax_identification_number,
        e.epf_number,
        e.socso_number,
        e.eis_number,
        e.working_spouse,
        e.num_children,
        e.epf_category,
        e.is_muslim,
        e.zakat_eligible,
        e.zakat_monthly_amount,
        e.ptptn_monthly_amount,
        e.tabung_haji_amount,
        e.hrdf_contribution,
        payroll_group_id,
        e.salary_group,
        e.is_active,
        e.deleted_at,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_employee_allowance(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    a: &EmployeeAllowanceExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO employee_allowances (id, employee_id, category, name, description, amount,
               is_taxable, is_recurring, effective_from, effective_to, is_active, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)"#,
        id,
        employee_id,
        a.category,
        a.name,
        a.description,
        a.amount,
        a.is_taxable,
        a.is_recurring,
        a.effective_from,
        a.effective_to,
        a.is_active,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_salary_history(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    s: &SalaryHistoryExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO salary_history (id, employee_id, old_salary, new_salary, effective_date, reason, created_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7)"#,
        id,
        employee_id,
        s.old_salary,
        s.new_salary,
        s.effective_date,
        s.reason,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_tp3_record(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    t: &Tp3RecordExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO tp3_records (id, employee_id, tax_year, previous_employer_name,
               previous_income_ytd, previous_epf_ytd, previous_pcb_ytd, previous_socso_ytd,
               previous_zakat_ytd, created_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)"#,
        id,
        employee_id,
        t.tax_year,
        t.previous_employer_name,
        t.previous_income_ytd,
        t.previous_epf_ytd,
        t.previous_pcb_ytd,
        t.previous_socso_ytd,
        t.previous_zakat_ytd,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_leave_type(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    lt: &LeaveTypeExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO leave_types (id, company_id, name, description, default_days, is_paid, is_active,
               created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
        id,
        company_id,
        lt.name,
        lt.description,
        lt.default_days,
        lt.is_paid,
        lt.is_active,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_leave_balance(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    leave_type_id: Uuid,
    lb: &LeaveBalanceExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO leave_balances (id, employee_id, leave_type_id, year,
               entitled_days, taken_days, pending_days, carried_forward, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)"#,
        id,
        employee_id,
        leave_type_id,
        lb.year,
        lb.entitled_days,
        lb.taken_days,
        lb.pending_days,
        lb.carried_forward,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_leave_request(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    company_id: Uuid,
    leave_type_id: Uuid,
    lr: &LeaveRequestExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO leave_requests (id, employee_id, company_id, leave_type_id,
               start_date, end_date, days, reason, status, review_notes,
               attachment_url, attachment_name, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)"#,
        id,
        employee_id,
        company_id,
        leave_type_id,
        lr.start_date,
        lr.end_date,
        lr.days,
        lr.reason,
        lr.status,
        lr.review_notes,
        lr.attachment_url,
        lr.attachment_name,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_claim(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    company_id: Uuid,
    cl: &ClaimExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO claims (id, employee_id, company_id, title, description, amount, category,
               receipt_url, receipt_file_name, expense_date, status, submitted_at,
               review_notes, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)"#,
        id,
        employee_id,
        company_id,
        cl.title,
        cl.description,
        cl.amount,
        cl.category,
        cl.receipt_url,
        cl.receipt_file_name,
        cl.expense_date,
        cl.status,
        cl.submitted_at,
        cl.review_notes,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_overtime(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    company_id: Uuid,
    ot: &OvertimeExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO overtime_applications (id, employee_id, company_id, ot_date, start_time,
               end_time, hours, ot_type, reason, status, review_notes, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)"#,
        id,
        employee_id,
        company_id,
        ot.ot_date,
        ot.start_time,
        ot.end_time,
        ot.hours,
        ot.ot_type,
        ot.reason,
        ot.status,
        ot.review_notes,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_payroll_run(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    payroll_group_id: Uuid,
    pr: &PayrollRunExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO payroll_runs (id, company_id, payroll_group_id, period_year, period_month,
               period_start, period_end, pay_date, status,
               total_gross, total_net, total_employer_cost,
               total_epf_employee, total_epf_employer,
               total_socso_employee, total_socso_employer,
               total_eis_employee, total_eis_employer,
               total_pcb, total_zakat, employee_count, version, notes,
               created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9::text::payroll_status,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25)"#,
        id,
        company_id,
        payroll_group_id,
        pr.period_year,
        pr.period_month,
        pr.period_start,
        pr.period_end,
        pr.pay_date,
        pr.status,
        pr.total_gross,
        pr.total_net,
        pr.total_employer_cost,
        pr.total_epf_employee,
        pr.total_epf_employer,
        pr.total_socso_employee,
        pr.total_socso_employer,
        pr.total_eis_employee,
        pr.total_eis_employer,
        pr.total_pcb,
        pr.total_zakat,
        pr.employee_count,
        pr.version,
        pr.notes,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_payroll_item(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    payroll_run_id: Uuid,
    employee_id: Uuid,
    pi: &PayrollItemExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO payroll_items (id, payroll_run_id, employee_id,
               basic_salary, gross_salary, total_allowances, total_overtime,
               total_bonus, total_commission, total_claims,
               epf_employee, epf_employer, socso_employee, socso_employer,
               eis_employee, eis_employer, pcb_amount, zakat_amount,
               ptptn_amount, tabung_haji_amount,
               total_loan_deductions, total_other_deductions,
               unpaid_leave_deduction, unpaid_leave_days,
               total_deductions, net_salary, employer_cost,
               ytd_gross, ytd_epf_employee, ytd_pcb,
               ytd_socso_employee, ytd_eis_employee, ytd_zakat, ytd_net,
               working_days, days_worked, is_prorated,
               created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,
                       $21,$22,$23,$24,$25,$26,$27,$28,$29,$30,$31,$32,$33,$34,$35,$36,$37,$38,$39)"#,
        id,
        payroll_run_id,
        employee_id,
        pi.basic_salary,
        pi.gross_salary,
        pi.total_allowances,
        pi.total_overtime,
        pi.total_bonus,
        pi.total_commission,
        pi.total_claims,
        pi.epf_employee,
        pi.epf_employer,
        pi.socso_employee,
        pi.socso_employer,
        pi.eis_employee,
        pi.eis_employer,
        pi.pcb_amount,
        pi.zakat_amount,
        pi.ptptn_amount,
        pi.tabung_haji_amount,
        pi.total_loan_deductions,
        pi.total_other_deductions,
        pi.unpaid_leave_deduction,
        pi.unpaid_leave_days,
        pi.total_deductions,
        pi.net_salary,
        pi.employer_cost,
        pi.ytd_gross,
        pi.ytd_epf_employee,
        pi.ytd_pcb,
        pi.ytd_socso_employee,
        pi.ytd_eis_employee,
        pi.ytd_zakat,
        pi.ytd_net,
        pi.working_days,
        pi.days_worked,
        pi.is_prorated,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_payroll_item_detail(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    payroll_item_id: Uuid,
    pid: &PayrollItemDetailExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO payroll_item_details (id, payroll_item_id, category, item_type,
               description, amount, is_taxable, is_statutory, created_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
        id,
        payroll_item_id,
        pid.category,
        pid.item_type,
        pid.description,
        pid.amount,
        pid.is_taxable,
        pid.is_statutory,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_payroll_entry(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    company_id: Uuid,
    payroll_run_id: Option<Uuid>,
    pe: &PayrollEntryExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO payroll_entries (id, employee_id, company_id, period_year, period_month,
               category, item_type, description, amount, quantity, rate,
               is_taxable, is_processed, payroll_run_id, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)"#,
        id,
        employee_id,
        company_id,
        pe.period_year,
        pe.period_month,
        pe.category,
        pe.item_type,
        pe.description,
        pe.amount,
        pe.quantity,
        pe.rate,
        pe.is_taxable,
        pe.is_processed,
        payroll_run_id,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_document_category(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    dc: &DocumentCategoryExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO document_categories (id, company_id, name, description, is_active, created_at)
               VALUES ($1,$2,$3,$4,$5,$6)"#,
        id,
        company_id,
        dc.name,
        dc.description,
        dc.is_active,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_document(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    employee_id: Option<Uuid>,
    category_id: Option<Uuid>,
    d: &DocumentExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO documents (id, company_id, employee_id, category_id, title, description,
               file_name, file_url, file_size, mime_type, status,
               issue_date, expiry_date, is_confidential, tags,
               deleted_at, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11::text::document_status,$12,$13,$14,$15,$16,$17,$18)"#,
        id,
        company_id,
        employee_id,
        category_id,
        d.title,
        d.description,
        d.file_name,
        d.file_url,
        d.file_size,
        d.mime_type,
        d.status,
        d.issue_date,
        d.expiry_date,
        d.is_confidential,
        d.tags,
        d.deleted_at,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_team(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    t: &TeamExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO teams (id, company_id, name, description, tag, is_active, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#,
        id,
        company_id,
        t.name,
        t.description,
        t.tag,
        t.is_active,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_team_member(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    team_id: Uuid,
    employee_id: Uuid,
    tm: &TeamMemberExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO team_members (id, team_id, employee_id, role, joined_at)
               VALUES ($1,$2,$3,$4,$5)"#,
        id,
        team_id,
        employee_id,
        tm.role,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_holiday(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    h: &HolidayExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO holidays (id, company_id, name, date, holiday_type, description,
               is_recurring, state, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)"#,
        id,
        company_id,
        h.name,
        h.date,
        h.holiday_type,
        h.description,
        h.is_recurring,
        h.state,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_working_day_config(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    w: &WorkingDayConfigExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO working_day_config (id, company_id, day_of_week, is_working_day, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6)"#,
        id,
        company_id,
        w.day_of_week,
        w.is_working_day,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_email_template(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    et: &EmailTemplateExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO email_templates (id, company_id, name, letter_type, subject, body_html,
               is_active, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
        id,
        company_id,
        et.name,
        et.letter_type,
        et.subject,
        et.body_html,
        et.is_active,
        now,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_company_setting(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    cs: &CompanySettingExport,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO company_settings (id, company_id, category, key, value, label, description, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#,
        id,
        company_id,
        cs.category,
        cs.key,
        cs.value,
        cs.label,
        cs.description,
        now,
    )
    .execute(executor)
    .await?;
    Ok(())
}
