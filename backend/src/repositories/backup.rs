//! Data access for whole-company backup export/import. Unlike the per-table
//! modules this is a use-case module (like `clock`): it spans every company-owned
//! table, reading/writing the bespoke `*Export` projections in `models::backup`.
//! All ID remapping, file (de)serialization, and the import transaction stay in
//! `services::backup_service`; this module only holds the SQL.

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
