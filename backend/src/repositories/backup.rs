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
//
// Every list read is ordered by primary key. `id` is uuidv7, so the order is
// stable and time-correlated, which makes a re-export of an unchanged company
// byte-comparable and makes the ids a restore mints reproducible. Without it the
// row order — and hence the restore's id assignment — was whatever the planner
// happened to return.

pub async fn company(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Option<CompanyExport>> {
    // `timezone` and `geofence_mode` are NOT NULL, but the export projects them
    // as nullable: the importer reads `None` as "this archive predates capture"
    // and must be able to express that for an archive it did not write.
    let company = sqlx::query_as!(
        CompanyExport,
        r#"SELECT id, name, registration_number, tax_number, epf_number, socso_code, eis_code,
                  hrdf_number, address_line1, address_line2, city, state, postcode, country,
                  phone, email, logo_url, hrdf_enabled, unpaid_leave_divisor, is_active,
                  created_at, updated_at,
                  attendance_method, timezone AS "timezone?", geofence_mode AS "geofence_mode?"
           FROM companies WHERE id = $1"#,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(company)
}

pub async fn company_work_schedules(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<CompanyWorkScheduleExport>> {
    let rows = sqlx::query_as!(
        CompanyWorkScheduleExport,
        r#"SELECT id, company_id, name, start_time, end_time, grace_minutes, half_day_hours,
                  timezone, is_default, created_at, updated_at
           FROM company_work_schedules WHERE company_id = $1 ORDER BY id"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn company_locations(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<CompanyLocationExport>> {
    let rows = sqlx::query_as!(
        CompanyLocationExport,
        r#"SELECT id, company_id, name, latitude, longitude, radius_meters, is_active,
                  created_at, updated_at
           FROM company_locations WHERE company_id = $1 ORDER BY id"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn payroll_groups(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<PayrollGroupExport>> {
    let rows = sqlx::query_as!(
        PayrollGroupExport,
        r#"SELECT id, company_id, name, description, cutoff_day, payment_day, is_active,
                  created_at, updated_at
           FROM payroll_groups WHERE company_id = $1 ORDER BY id"#,
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
           FROM employees WHERE company_id = $1 ORDER BY id"#,
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
           WHERE e.company_id = $1 ORDER BY ea.id"#,
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
           WHERE e.company_id = $1 ORDER BY sh.id"#,
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
           WHERE e.company_id = $1 ORDER BY t.id"#,
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
           FROM leave_types WHERE company_id = $1 ORDER BY id"#,
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
           WHERE e.company_id = $1 ORDER BY lb.id"#,
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
           FROM leave_requests WHERE company_id = $1 ORDER BY id"#,
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
           FROM claims WHERE company_id = $1 ORDER BY id"#,
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
           FROM overtime_applications WHERE company_id = $1 ORDER BY id"#,
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
           FROM payroll_runs WHERE company_id = $1 ORDER BY id"#,
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
           WHERE pr.company_id = $1 ORDER BY pi.id"#,
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
           WHERE pr.company_id = $1 ORDER BY pid.id"#,
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
           FROM payroll_entries WHERE company_id = $1 ORDER BY id"#,
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
           FROM document_categories WHERE company_id = $1 ORDER BY id"#,
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
           FROM documents WHERE company_id = $1 ORDER BY id"#,
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
           FROM teams WHERE company_id = $1 ORDER BY id"#,
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
           WHERE t.company_id = $1 ORDER BY tm.id"#,
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
           FROM holidays WHERE company_id = $1 ORDER BY id"#,
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
           FROM working_day_config WHERE company_id = $1 ORDER BY id"#,
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
           FROM email_templates WHERE company_id = $1 ORDER BY id"#,
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
           FROM company_settings WHERE company_id = $1 ORDER BY id"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

// ─── Import writes ───
//
// A restored row is a faithful copy of the archived row, so it carries the
// archived row's own `created_at`/`updated_at` (and `joined_at`). Stamping the
// restore instant on every row instead — which is what these used to do — made
// each `ORDER BY created_at … LIMIT n` list return an arbitrary subset of a
// fully tied set, and left every restored claim `submitted_at` years before its
// own `created_at`. The one exception is the live `companies` row on an
// overwrite: that row genuinely is being modified now, so `update_company`
// still takes the restore instant.
//
// The service owns ID remapping, file restore, and the import transaction.

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

/// Overwrite the target company's own row from the archive.
///
/// The three attendance columns are COALESCEd rather than assigned: `NULL` here
/// means the archive predates their capture, and a restore that cannot speak to
/// a setting must not overwrite it. Assigning them unconditionally would move a
/// live Jakarta tenant onto MYT the moment an old archive was restored over it.
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
               unpaid_leave_divisor=$18, is_active=$19, updated_at=$20,
               attendance_method=COALESCE($21, attendance_method),
               timezone=COALESCE($22, timezone),
               geofence_mode=COALESCE($23, geofence_mode)
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
        c.attendance_method,
        c.timezone,
        c.geofence_mode,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Create the restored company. `timezone`/`geofence_mode` are NOT NULL, so an
/// archive that predates their capture falls back to the same values
/// `provision_company_defaults` would have used.
pub async fn insert_company(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    c: &CompanyExport,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO companies (id, name, registration_number, tax_number, epf_number, socso_code,
               eis_code, hrdf_number, address_line1, address_line2, city, state, postcode, country,
               phone, email, logo_url, hrdf_enabled, unpaid_leave_divisor, is_active, created_at, updated_at,
               attendance_method, timezone, geofence_mode)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,
                       $23, COALESCE($24::varchar, 'Asia/Kuala_Lumpur'), COALESCE($25::varchar, 'none'))"#,
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
        c.created_at,
        c.updated_at,
        c.attendance_method,
        c.timezone,
        c.geofence_mode,
    )
    .execute(executor)
    .await?;
    Ok(())
}

// ─── Attendance configuration (restore-specific replace) ───
//
// `company_work_schedules` and `company_locations` are ON DELETE CASCADE from
// `companies`, so an overwrite restore — which keeps the `companies` row — does
// not reach them through `companies::delete_company_data`, and they are
// deliberately absent from that list (adding every cascade table there would
// also destroy the target's `audit_logs`). Replacing them is restore-specific
// behaviour, so these two deletes live here with the rest of the restore SQL.

pub async fn delete_company_work_schedules(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM company_work_schedules WHERE company_id = $1",
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn delete_company_locations(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM company_locations WHERE company_id = $1",
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_company_work_schedule(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    ws: &CompanyWorkScheduleExport,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO company_work_schedules (id, company_id, name, start_time, end_time,
               grace_minutes, half_day_hours, timezone, is_default, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)"#,
        id,
        company_id,
        ws.name,
        ws.start_time,
        ws.end_time,
        ws.grace_minutes,
        ws.half_day_hours,
        ws.timezone,
        ws.is_default,
        ws.created_at,
        ws.updated_at,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_company_location(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    cl: &CompanyLocationExport,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO company_locations (id, company_id, name, latitude, longitude,
               radius_meters, is_active, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
        id,
        company_id,
        cl.name,
        cl.latitude,
        cl.longitude,
        cl.radius_meters,
        cl.is_active,
        cl.created_at,
        cl.updated_at,
    )
    .execute(executor)
    .await?;
    Ok(())
}

// ─── Orphaned employee logins (overwrite restore) ───
//
// An overwrite hard-deletes `employees`, and `users_employee_tenant_fkey` is a
// column-scoped ON DELETE SET NULL: it nulls `users.employee_id` and leaves
// `company_id`, `roles` and `is_active` alone. `auth_service::linked_employee_active`
// treats a NULL link as "no employee to check", so anyone hired after the backup
// was taken keeps a working login with no employee record — and a leaver whose
// only block was `employees.is_active = false` becomes *unblocked*, because the
// row that made the guard fire is gone.
//
// The sweep is capture-then-deactivate rather than one blanket UPDATE:
// `update_user` can legitimately demote an administrator to `roles=['employee']`
// with no employee link at all, and a blanket sweep would lock that account out.
// Capturing first means only rows that held a link when the restore began can
// ever be named.

/// Every `employee`-role login this company's employees are linked to, captured
/// before the wipe.
///
/// `roles <@ ARRAY['employee']` is the same backstop `users::soft_delete_by_employee`
/// uses: an account holding an administrative role is never retired by employee
/// lifecycle, however its `employee_id` came to be set.
pub async fn employee_linked_login_ids(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<Uuid>> {
    let ids = sqlx::query_scalar!(
        r#"SELECT id FROM users
        WHERE company_id = $1
          AND employee_id IS NOT NULL
          AND deleted_at IS NULL
          AND roles <@ ARRAY['employee']::VARCHAR(50)[]"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(ids)
}

/// Deactivate the captured logins the restore did not re-link, returning the
/// ids actually deactivated so the caller can revoke their live access.
///
/// `is_active = FALSE` and deliberately not `deleted_at`: a tombstone is what
/// `provision_imported_employee_account` refuses to resurrect, so it would
/// permanently block a later restore of a *newer* backup from handing the
/// employee their account back. An inactive account is re-activatable from
/// Users.
pub async fn deactivate_unlinked_logins(
    executor: impl Executor<'_, Database = Postgres>,
    ids: &[Uuid],
) -> AppResult<Vec<Uuid>> {
    let deactivated = sqlx::query_scalar!(
        r#"UPDATE users SET is_active = FALSE, updated_at = NOW()
        WHERE id = ANY($1)
          AND employee_id IS NULL
          AND is_active = TRUE
          AND deleted_at IS NULL
        RETURNING id"#,
        ids,
    )
    .fetch_all(executor)
    .await?;
    Ok(deactivated)
}

pub async fn insert_payroll_group(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    pg: &PayrollGroupExport,
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
        pg.created_at,
        pg.updated_at,
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
        e.created_at,
        e.updated_at,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_employee_allowance(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    company_id: Uuid,
    a: &EmployeeAllowanceExport,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO employee_allowances (id, employee_id, company_id, category, name, description, amount,
               is_taxable, is_recurring, effective_from, effective_to, is_active, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)"#,
        id,
        employee_id,
        company_id,
        a.category,
        a.name,
        a.description,
        a.amount,
        a.is_taxable,
        a.is_recurring,
        a.effective_from,
        a.effective_to,
        a.is_active,
        a.created_at,
        a.updated_at,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_salary_history(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    company_id: Uuid,
    s: &SalaryHistoryExport,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO salary_history (id, employee_id, company_id, old_salary, new_salary, effective_date, reason, created_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#,
        id,
        employee_id,
        company_id,
        s.old_salary,
        s.new_salary,
        s.effective_date,
        s.reason,
        s.created_at,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_tp3_record(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    company_id: Uuid,
    t: &Tp3RecordExport,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO tp3_records (id, employee_id, company_id, tax_year, previous_employer_name,
               previous_income_ytd, previous_epf_ytd, previous_pcb_ytd, previous_socso_ytd,
               previous_zakat_ytd, created_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)"#,
        id,
        employee_id,
        company_id,
        t.tax_year,
        t.previous_employer_name,
        t.previous_income_ytd,
        t.previous_epf_ytd,
        t.previous_pcb_ytd,
        t.previous_socso_ytd,
        t.previous_zakat_ytd,
        t.created_at,
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
        lt.created_at,
        lt.updated_at,
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
        lb.created_at,
        lb.updated_at,
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
        lr.created_at,
        lr.updated_at,
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
        cl.created_at,
        cl.updated_at,
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
        ot.created_at,
        ot.updated_at,
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
        pr.created_at,
        pr.updated_at,
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
        pi.created_at,
        pi.updated_at,
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
        pid.created_at,
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
        pe.created_at,
        pe.updated_at,
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
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO document_categories (id, company_id, name, description, is_active, created_at)
               VALUES ($1,$2,$3,$4,$5,$6)"#,
        id,
        company_id,
        dc.name,
        dc.description,
        dc.is_active,
        dc.created_at,
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
        d.created_at,
        d.updated_at,
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
        t.created_at,
        t.updated_at,
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
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO team_members (id, team_id, employee_id, role, joined_at)
               VALUES ($1,$2,$3,$4,$5)"#,
        id,
        team_id,
        employee_id,
        tm.role,
        tm.joined_at,
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
        h.created_at,
        h.updated_at,
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
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO working_day_config (id, company_id, day_of_week, is_working_day, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6)"#,
        id,
        company_id,
        w.day_of_week,
        w.is_working_day,
        w.created_at,
        w.updated_at,
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
        et.created_at,
        et.updated_at,
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
        cs.updated_at,
    )
    .execute(executor)
    .await?;
    Ok(())
}
