use std::collections::HashMap;

use base64::Engine;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::backup::*;

pub async fn export_company(pool: &PgPool, company_id: Uuid) -> AppResult<CompanyBackup> {
    let company = sqlx::query_as!(
        CompanyExport,
        r#"SELECT id, name, registration_number, tax_number, epf_number, socso_code, eis_code,
                  hrdf_number, address_line1, address_line2, city, state, postcode, country,
                  phone, email, logo_url, hrdf_enabled, unpaid_leave_divisor, is_active,
                  created_at, updated_at
           FROM companies WHERE id = $1"#,
        company_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Company not found".into()))?;

    let payroll_groups = sqlx::query_as!(
        PayrollGroupExport,
        r#"SELECT id, company_id, name, description, cutoff_day, payment_day, is_active,
                  created_at, updated_at
           FROM payroll_groups WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let employees = sqlx::query_as!(
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
    .fetch_all(pool)
    .await?;

    let employee_allowances = sqlx::query_as!(
        EmployeeAllowanceExport,
        r#"SELECT ea.id, ea.employee_id, ea.category, ea.name, ea.description, ea.amount,
                  ea.is_taxable, ea.is_recurring, ea.effective_from, ea.effective_to,
                  ea.is_active, ea.created_at, ea.updated_at
           FROM employee_allowances ea
           JOIN employees e ON ea.employee_id = e.id
           WHERE e.company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let salary_history = sqlx::query_as!(
        SalaryHistoryExport,
        r#"SELECT sh.id, sh.employee_id, sh.old_salary, sh.new_salary,
                  sh.effective_date, sh.reason, sh.created_at
           FROM salary_history sh
           JOIN employees e ON sh.employee_id = e.id
           WHERE e.company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let tp3_records = sqlx::query_as!(
        Tp3RecordExport,
        r#"SELECT t.id, t.employee_id, t.tax_year, t.previous_employer_name,
                  t.previous_income_ytd, t.previous_epf_ytd, t.previous_pcb_ytd,
                  t.previous_socso_ytd, t.previous_zakat_ytd, t.created_at
           FROM tp3_records t
           JOIN employees e ON t.employee_id = e.id
           WHERE e.company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let leave_types = sqlx::query_as!(
        LeaveTypeExport,
        r#"SELECT id, company_id, name, description, default_days, is_paid, is_active,
                  created_at, updated_at
           FROM leave_types WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let leave_balances = sqlx::query_as!(
        LeaveBalanceExport,
        r#"SELECT lb.id, lb.employee_id, lb.leave_type_id, lb.year,
                  lb.entitled_days, lb.taken_days, lb.pending_days, lb.carried_forward,
                  lb.created_at, lb.updated_at
           FROM leave_balances lb
           JOIN employees e ON lb.employee_id = e.id
           WHERE e.company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let leave_requests = sqlx::query_as!(
        LeaveRequestExport,
        r#"SELECT id, employee_id, company_id, leave_type_id, start_date, end_date, days,
                  reason, status, review_notes, attachment_url, attachment_name,
                  created_at, updated_at
           FROM leave_requests WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let claims = sqlx::query_as!(
        ClaimExport,
        r#"SELECT id, employee_id, company_id, title, description, amount, category,
                  receipt_url, receipt_file_name, expense_date, status,
                  submitted_at, review_notes, created_at, updated_at
           FROM claims WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let overtime_applications = sqlx::query_as!(
        OvertimeExport,
        r#"SELECT id, employee_id, company_id, ot_date, start_time, end_time, hours,
                  ot_type, reason, status, review_notes, created_at, updated_at
           FROM overtime_applications WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let payroll_runs = sqlx::query_as!(
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
    .fetch_all(pool)
    .await?;

    let payroll_items = sqlx::query_as!(
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
    .fetch_all(pool)
    .await?;

    let payroll_item_details = sqlx::query_as!(
        PayrollItemDetailExport,
        r#"SELECT pid.id, pid.payroll_item_id, pid.category, pid.item_type,
                  pid.description, pid.amount, pid.is_taxable, pid.is_statutory, pid.created_at
           FROM payroll_item_details pid
           JOIN payroll_items pi ON pid.payroll_item_id = pi.id
           JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
           WHERE pr.company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let payroll_entries = sqlx::query_as!(
        PayrollEntryExport,
        r#"SELECT id, employee_id, company_id, period_year, period_month,
                  category, item_type, description AS "description?", amount, quantity, rate,
                  is_taxable, is_processed, payroll_run_id, created_at, updated_at
           FROM payroll_entries WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let document_categories = sqlx::query_as!(
        DocumentCategoryExport,
        r#"SELECT id, company_id, name, description, is_active, created_at
           FROM document_categories WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let documents = sqlx::query_as!(
        DocumentExport,
        r#"SELECT id, company_id, employee_id, category_id, title, description,
                  file_name, file_url, file_size, mime_type, status::text AS "status!",
                  issue_date, expiry_date, is_confidential, tags,
                  deleted_at, created_at, updated_at
           FROM documents WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let teams = sqlx::query_as!(
        TeamExport,
        r#"SELECT id, company_id, name, description, tag, is_active, created_at, updated_at
           FROM teams WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let team_members = sqlx::query_as!(
        TeamMemberExport,
        r#"SELECT tm.id, tm.team_id, tm.employee_id, tm.role, tm.joined_at
           FROM team_members tm
           JOIN teams t ON tm.team_id = t.id
           WHERE t.company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let holidays = sqlx::query_as!(
        HolidayExport,
        r#"SELECT id, company_id, name, date, holiday_type, description, is_recurring, state,
                  created_at, updated_at
           FROM holidays WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let working_day_config = sqlx::query_as!(
        WorkingDayConfigExport,
        r#"SELECT id, company_id, day_of_week, is_working_day, created_at, updated_at
           FROM working_day_config WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let email_templates = sqlx::query_as!(
        EmailTemplateExport,
        r#"SELECT id, company_id, name, letter_type, subject, body_html, is_active AS "is_active?",
                  created_at, updated_at
           FROM email_templates WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let company_settings = sqlx::query_as!(
        CompanySettingExport,
        r#"SELECT id, company_id, category, key, value, label, description, updated_at
           FROM company_settings WHERE company_id = $1"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let mut record_counts = HashMap::new();
    record_counts.insert("payroll_groups".into(), payroll_groups.len());
    record_counts.insert("employees".into(), employees.len());
    record_counts.insert("employee_allowances".into(), employee_allowances.len());
    record_counts.insert("salary_history".into(), salary_history.len());
    record_counts.insert("tp3_records".into(), tp3_records.len());
    record_counts.insert("leave_types".into(), leave_types.len());
    record_counts.insert("leave_balances".into(), leave_balances.len());
    record_counts.insert("leave_requests".into(), leave_requests.len());
    record_counts.insert("claims".into(), claims.len());
    record_counts.insert("overtime_applications".into(), overtime_applications.len());
    record_counts.insert("payroll_runs".into(), payroll_runs.len());
    record_counts.insert("payroll_items".into(), payroll_items.len());
    record_counts.insert("payroll_item_details".into(), payroll_item_details.len());
    record_counts.insert("payroll_entries".into(), payroll_entries.len());
    record_counts.insert("document_categories".into(), document_categories.len());
    record_counts.insert("documents".into(), documents.len());
    record_counts.insert("teams".into(), teams.len());
    record_counts.insert("team_members".into(), team_members.len());
    record_counts.insert("holidays".into(), holidays.len());
    record_counts.insert("working_day_config".into(), working_day_config.len());
    record_counts.insert("email_templates".into(), email_templates.len());
    record_counts.insert("company_settings".into(), company_settings.len());

    let metadata = BackupMetadata {
        format_version: "1.0".into(),
        exported_at: Utc::now(),
        source_company_id: company_id,
        source_company_name: company.name.clone(),
        record_counts,
    };

    // Collect uploaded files (documents, leave attachments, claim receipts)
    let mut files: HashMap<String, String> = HashMap::new();
    let upload_dir = std::path::Path::new("uploads");
    let b64 = base64::engine::general_purpose::STANDARD;

    let mut collect_file = |url: Option<&String>| {
        if let Some(u) = url
            && let Some(filename) = u.strip_prefix("/api/uploads/")
        {
            let path = upload_dir.join(filename);
            if let Ok(data) = std::fs::read(&path) {
                files.insert(u.clone(), b64.encode(&data));
            }
        }
    };

    for d in &documents {
        collect_file(Some(&d.file_url));
    }
    for lr in &leave_requests {
        collect_file(lr.attachment_url.as_ref());
    }
    for c in &claims {
        collect_file(c.receipt_url.as_ref());
    }

    Ok(CompanyBackup {
        metadata,
        company,
        payroll_groups,
        employees,
        employee_allowances,
        salary_history,
        tp3_records,
        leave_types,
        leave_balances,
        leave_requests,
        claims,
        overtime_applications,
        payroll_runs,
        payroll_items,
        payroll_item_details,
        payroll_entries,
        document_categories,
        documents,
        teams,
        team_members,
        holidays,
        working_day_config,
        email_templates,
        company_settings,
        files,
    })
}

pub async fn import_company(
    pool: &PgPool,
    backup: CompanyBackup,
    _importing_user_id: Uuid,
) -> AppResult<ImportResult> {
    if backup.metadata.format_version != "1.0" {
        return Err(AppError::BadRequest(format!(
            "Unsupported backup format version: {}. Expected 1.0",
            backup.metadata.format_version
        )));
    }

    // Check if company with same name already exists
    let existing_company = sqlx::query_scalar!(
        "SELECT id FROM companies WHERE LOWER(name) = LOWER($1)",
        backup.company.name,
    )
    .fetch_optional(pool)
    .await?;

    let is_overwrite = existing_company.is_some();

    // Build UUID remap table
    let mut remap: HashMap<Uuid, Uuid> = HashMap::new();

    let new_company_id = existing_company.unwrap_or_else(Uuid::new_v4);
    remap.insert(backup.company.id, new_company_id);

    for pg in &backup.payroll_groups {
        remap.insert(pg.id, Uuid::now_v7());
    }
    for e in &backup.employees {
        remap.insert(e.id, Uuid::now_v7());
    }
    for a in &backup.employee_allowances {
        remap.insert(a.id, Uuid::now_v7());
    }
    for s in &backup.salary_history {
        remap.insert(s.id, Uuid::now_v7());
    }
    for t in &backup.tp3_records {
        remap.insert(t.id, Uuid::now_v7());
    }
    for lt in &backup.leave_types {
        remap.insert(lt.id, Uuid::now_v7());
    }
    for lb in &backup.leave_balances {
        remap.insert(lb.id, Uuid::now_v7());
    }
    for lr in &backup.leave_requests {
        remap.insert(lr.id, Uuid::now_v7());
    }
    for c in &backup.claims {
        remap.insert(c.id, Uuid::now_v7());
    }
    for o in &backup.overtime_applications {
        remap.insert(o.id, Uuid::now_v7());
    }
    for pr in &backup.payroll_runs {
        remap.insert(pr.id, Uuid::now_v7());
    }
    for pi in &backup.payroll_items {
        remap.insert(pi.id, Uuid::now_v7());
    }
    for pid in &backup.payroll_item_details {
        remap.insert(pid.id, Uuid::now_v7());
    }
    for pe in &backup.payroll_entries {
        remap.insert(pe.id, Uuid::now_v7());
    }
    for dc in &backup.document_categories {
        remap.insert(dc.id, Uuid::now_v7());
    }
    for d in &backup.documents {
        remap.insert(d.id, Uuid::now_v7());
    }
    for t in &backup.teams {
        remap.insert(t.id, Uuid::now_v7());
    }
    for tm in &backup.team_members {
        remap.insert(tm.id, Uuid::now_v7());
    }
    for h in &backup.holidays {
        remap.insert(h.id, Uuid::now_v7());
    }
    for w in &backup.working_day_config {
        remap.insert(w.id, Uuid::now_v7());
    }
    for et in &backup.email_templates {
        remap.insert(et.id, Uuid::now_v7());
    }
    for cs in &backup.company_settings {
        remap.insert(cs.id, Uuid::now_v7());
    }

    let r = |old: Uuid| -> Uuid { *remap.get(&old).unwrap_or(&old) };
    let ro = |old: Option<Uuid>| -> Option<Uuid> { old.map(&r) };

    let mut tx = pool.begin().await?;
    let mut warnings = Vec::new();
    let now = Utc::now();

    // If overwriting, delete all existing data for this company (order matters for FK constraints)
    if is_overwrite {
        warnings.push(format!(
            "Existing company \"{}\" data was overwritten.",
            backup.company.name
        ));

        // Delete in reverse dependency order
        sqlx::query!(
            "DELETE FROM company_settings WHERE company_id = $1",
            new_company_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "DELETE FROM email_templates WHERE company_id = $1",
            new_company_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "DELETE FROM working_day_config WHERE company_id = $1",
            new_company_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!("DELETE FROM holidays WHERE company_id = $1", new_company_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query!("DELETE FROM team_members WHERE team_id IN (SELECT id FROM teams WHERE company_id = $1)", new_company_id).execute(&mut *tx).await?;
        sqlx::query!("DELETE FROM teams WHERE company_id = $1", new_company_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query!(
            "DELETE FROM documents WHERE company_id = $1",
            new_company_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "DELETE FROM document_categories WHERE company_id = $1",
            new_company_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "DELETE FROM payroll_entries WHERE company_id = $1",
            new_company_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!("DELETE FROM payroll_item_details WHERE payroll_item_id IN (SELECT pi.id FROM payroll_items pi JOIN payroll_runs pr ON pi.payroll_run_id = pr.id WHERE pr.company_id = $1)", new_company_id).execute(&mut *tx).await?;
        sqlx::query!("DELETE FROM payroll_items WHERE payroll_run_id IN (SELECT id FROM payroll_runs WHERE company_id = $1)", new_company_id).execute(&mut *tx).await?;
        sqlx::query!(
            "DELETE FROM payroll_runs WHERE company_id = $1",
            new_company_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "DELETE FROM overtime_applications WHERE company_id = $1",
            new_company_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!("DELETE FROM claims WHERE company_id = $1", new_company_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query!(
            "DELETE FROM leave_requests WHERE company_id = $1",
            new_company_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!("DELETE FROM leave_balances WHERE employee_id IN (SELECT id FROM employees WHERE company_id = $1)", new_company_id).execute(&mut *tx).await?;
        sqlx::query!(
            "DELETE FROM leave_types WHERE company_id = $1",
            new_company_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!("DELETE FROM tp3_records WHERE employee_id IN (SELECT id FROM employees WHERE company_id = $1)", new_company_id).execute(&mut *tx).await?;
        sqlx::query!("DELETE FROM salary_history WHERE employee_id IN (SELECT id FROM employees WHERE company_id = $1)", new_company_id).execute(&mut *tx).await?;
        sqlx::query!("DELETE FROM employee_allowances WHERE employee_id IN (SELECT id FROM employees WHERE company_id = $1)", new_company_id).execute(&mut *tx).await?;
        sqlx::query!(
            "DELETE FROM employees WHERE company_id = $1",
            new_company_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "DELETE FROM payroll_groups WHERE company_id = $1",
            new_company_id
        )
        .execute(&mut *tx)
        .await?;

        // Update the company record
        let c = &backup.company;
        sqlx::query!(
            r#"UPDATE companies SET registration_number=$2, tax_number=$3, epf_number=$4, socso_code=$5,
               eis_code=$6, hrdf_number=$7, address_line1=$8, address_line2=$9, city=$10, state=$11,
               postcode=$12, country=$13, phone=$14, email=$15, logo_url=$16, hrdf_enabled=$17,
               unpaid_leave_divisor=$18, is_active=$19, updated_at=$20
               WHERE id = $1"#,
            new_company_id,
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
        .execute(&mut *tx)
        .await?;
    } else {
        // 1. Create new company
        let c = &backup.company;
        sqlx::query!(
            r#"INSERT INTO companies (id, name, registration_number, tax_number, epf_number, socso_code,
               eis_code, hrdf_number, address_line1, address_line2, city, state, postcode, country,
               phone, email, logo_url, hrdf_enabled, unpaid_leave_divisor, is_active, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22)"#,
            new_company_id,
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
        .execute(&mut *tx)
        .await?;
    }

    // 2. Payroll groups
    for pg in &backup.payroll_groups {
        sqlx::query!(
            r#"INSERT INTO payroll_groups (id, company_id, name, description, cutoff_day, payment_day,
               is_active, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
            r(pg.id),
            new_company_id,
            pg.name,
            pg.description,
            pg.cutoff_day,
            pg.payment_day,
            pg.is_active,
            now,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }

    // 3. Employees
    for e in &backup.employees {
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
            r(e.id),
            new_company_id,
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
            ro(e.payroll_group_id),
            e.salary_group,
            e.is_active,
            e.deleted_at,
            now,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }

    // 4. Employee allowances
    for a in &backup.employee_allowances {
        sqlx::query!(
            r#"INSERT INTO employee_allowances (id, employee_id, category, name, description, amount,
               is_taxable, is_recurring, effective_from, effective_to, is_active, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)"#,
            r(a.id),
            r(a.employee_id),
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
        .execute(&mut *tx)
        .await?;
    }

    // 5. Salary history
    for s in &backup.salary_history {
        sqlx::query!(
            r#"INSERT INTO salary_history (id, employee_id, old_salary, new_salary, effective_date, reason, created_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7)"#,
            r(s.id),
            r(s.employee_id),
            s.old_salary,
            s.new_salary,
            s.effective_date,
            s.reason,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }

    // 6. TP3 records
    for t in &backup.tp3_records {
        sqlx::query!(
            r#"INSERT INTO tp3_records (id, employee_id, tax_year, previous_employer_name,
               previous_income_ytd, previous_epf_ytd, previous_pcb_ytd, previous_socso_ytd,
               previous_zakat_ytd, created_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)"#,
            r(t.id),
            r(t.employee_id),
            t.tax_year,
            t.previous_employer_name,
            t.previous_income_ytd,
            t.previous_epf_ytd,
            t.previous_pcb_ytd,
            t.previous_socso_ytd,
            t.previous_zakat_ytd,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }

    // 7. Leave types
    for lt in &backup.leave_types {
        sqlx::query!(
            r#"INSERT INTO leave_types (id, company_id, name, description, default_days, is_paid, is_active,
               created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
            r(lt.id),
            new_company_id,
            lt.name,
            lt.description,
            lt.default_days,
            lt.is_paid,
            lt.is_active,
            now,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }

    // 8. Leave balances
    for lb in &backup.leave_balances {
        sqlx::query!(
            r#"INSERT INTO leave_balances (id, employee_id, leave_type_id, year,
               entitled_days, taken_days, pending_days, carried_forward, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)"#,
            r(lb.id),
            r(lb.employee_id),
            r(lb.leave_type_id),
            lb.year,
            lb.entitled_days,
            lb.taken_days,
            lb.pending_days,
            lb.carried_forward,
            now,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }

    // 9. Leave requests
    for lr in &backup.leave_requests {
        sqlx::query!(
            r#"INSERT INTO leave_requests (id, employee_id, company_id, leave_type_id,
               start_date, end_date, days, reason, status, review_notes,
               attachment_url, attachment_name, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)"#,
            r(lr.id),
            r(lr.employee_id),
            new_company_id,
            r(lr.leave_type_id),
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
        .execute(&mut *tx)
        .await?;
    }

    // 10. Claims
    for cl in &backup.claims {
        sqlx::query!(
            r#"INSERT INTO claims (id, employee_id, company_id, title, description, amount, category,
               receipt_url, receipt_file_name, expense_date, status, submitted_at,
               review_notes, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)"#,
            r(cl.id),
            r(cl.employee_id),
            new_company_id,
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
        .execute(&mut *tx)
        .await?;
    }

    // 11. Overtime applications
    for ot in &backup.overtime_applications {
        sqlx::query!(
            r#"INSERT INTO overtime_applications (id, employee_id, company_id, ot_date, start_time,
               end_time, hours, ot_type, reason, status, review_notes, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)"#,
            r(ot.id),
            r(ot.employee_id),
            new_company_id,
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
        .execute(&mut *tx)
        .await?;
    }

    // 12. Payroll runs
    for pr in &backup.payroll_runs {
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
            r(pr.id),
            new_company_id,
            r(pr.payroll_group_id),
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
        .execute(&mut *tx)
        .await?;
    }

    // 13. Payroll items
    for pi in &backup.payroll_items {
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
            r(pi.id),
            r(pi.payroll_run_id),
            r(pi.employee_id),
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
        .execute(&mut *tx)
        .await?;
    }

    // 14. Payroll item details
    for pid in &backup.payroll_item_details {
        sqlx::query!(
            r#"INSERT INTO payroll_item_details (id, payroll_item_id, category, item_type,
               description, amount, is_taxable, is_statutory, created_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
            r(pid.id),
            r(pid.payroll_item_id),
            pid.category,
            pid.item_type,
            pid.description,
            pid.amount,
            pid.is_taxable,
            pid.is_statutory,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }

    // 15. Payroll entries
    for pe in &backup.payroll_entries {
        sqlx::query!(
            r#"INSERT INTO payroll_entries (id, employee_id, company_id, period_year, period_month,
               category, item_type, description, amount, quantity, rate,
               is_taxable, is_processed, payroll_run_id, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)"#,
            r(pe.id),
            r(pe.employee_id),
            new_company_id,
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
            ro(pe.payroll_run_id),
            now,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }

    // 16. Document categories
    for dc in &backup.document_categories {
        sqlx::query!(
            r#"INSERT INTO document_categories (id, company_id, name, description, is_active, created_at)
               VALUES ($1,$2,$3,$4,$5,$6)"#,
            r(dc.id),
            new_company_id,
            dc.name,
            dc.description,
            dc.is_active,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }

    // 17. Documents
    for d in &backup.documents {
        sqlx::query!(
            r#"INSERT INTO documents (id, company_id, employee_id, category_id, title, description,
               file_name, file_url, file_size, mime_type, status,
               issue_date, expiry_date, is_confidential, tags,
               deleted_at, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11::text::document_status,$12,$13,$14,$15,$16,$17,$18)"#,
            r(d.id),
            new_company_id,
            ro(d.employee_id),
            ro(d.category_id),
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
        .execute(&mut *tx)
        .await?;
    }

    // 18. Teams
    for t in &backup.teams {
        sqlx::query!(
            r#"INSERT INTO teams (id, company_id, name, description, tag, is_active, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#,
            r(t.id),
            new_company_id,
            t.name,
            t.description,
            t.tag,
            t.is_active,
            now,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }

    // 19. Team members
    for tm in &backup.team_members {
        sqlx::query!(
            r#"INSERT INTO team_members (id, team_id, employee_id, role, joined_at)
               VALUES ($1,$2,$3,$4,$5)"#,
            r(tm.id),
            r(tm.team_id),
            r(tm.employee_id),
            tm.role,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }

    // 20. Holidays
    for h in &backup.holidays {
        sqlx::query!(
            r#"INSERT INTO holidays (id, company_id, name, date, holiday_type, description,
               is_recurring, state, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)"#,
            r(h.id),
            new_company_id,
            h.name,
            h.date,
            h.holiday_type,
            h.description,
            h.is_recurring,
            h.state,
            now,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }

    // 21. Working day config
    for w in &backup.working_day_config {
        sqlx::query!(
            r#"INSERT INTO working_day_config (id, company_id, day_of_week, is_working_day, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6)"#,
            r(w.id),
            new_company_id,
            w.day_of_week,
            w.is_working_day,
            now,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }

    // 22. Email templates
    for et in &backup.email_templates {
        sqlx::query!(
            r#"INSERT INTO email_templates (id, company_id, name, letter_type, subject, body_html,
               is_active, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
            r(et.id),
            new_company_id,
            et.name,
            et.letter_type,
            et.subject,
            et.body_html,
            et.is_active,
            now,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }

    // 23. Company settings
    for cs in &backup.company_settings {
        sqlx::query!(
            r#"INSERT INTO company_settings (id, company_id, category, key, value, label, description, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#,
            r(cs.id),
            new_company_id,
            cs.category,
            cs.key,
            cs.value,
            cs.label,
            cs.description,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    // Restore uploaded files from backup
    if !backup.files.is_empty() {
        let upload_dir = std::path::Path::new("uploads");
        let _ = tokio::fs::create_dir_all(upload_dir).await;
        let b64 = base64::engine::general_purpose::STANDARD;
        let mut files_restored = 0usize;
        for (url, data_b64) in &backup.files {
            if let Some(filename) = url.strip_prefix("/api/uploads/")
                && let Ok(data) = b64.decode(data_b64)
            {
                let path = upload_dir.join(filename);
                if tokio::fs::write(&path, &data).await.is_ok() {
                    files_restored += 1;
                }
            }
        }
        if files_restored > 0 {
            warnings.push(format!(
                "{} file(s) restored to uploads directory.",
                files_restored
            ));
        }
    }

    let mut records_imported = HashMap::new();
    records_imported.insert("company".into(), 1usize);
    records_imported.insert("payroll_groups".into(), backup.payroll_groups.len());
    records_imported.insert("employees".into(), backup.employees.len());
    records_imported.insert(
        "employee_allowances".into(),
        backup.employee_allowances.len(),
    );
    records_imported.insert("salary_history".into(), backup.salary_history.len());
    records_imported.insert("tp3_records".into(), backup.tp3_records.len());
    records_imported.insert("leave_types".into(), backup.leave_types.len());
    records_imported.insert("leave_balances".into(), backup.leave_balances.len());
    records_imported.insert("leave_requests".into(), backup.leave_requests.len());
    records_imported.insert("claims".into(), backup.claims.len());
    records_imported.insert(
        "overtime_applications".into(),
        backup.overtime_applications.len(),
    );
    records_imported.insert("payroll_runs".into(), backup.payroll_runs.len());
    records_imported.insert("payroll_items".into(), backup.payroll_items.len());
    records_imported.insert(
        "payroll_item_details".into(),
        backup.payroll_item_details.len(),
    );
    records_imported.insert("payroll_entries".into(), backup.payroll_entries.len());
    records_imported.insert(
        "document_categories".into(),
        backup.document_categories.len(),
    );
    records_imported.insert("documents".into(), backup.documents.len());
    records_imported.insert("teams".into(), backup.teams.len());
    records_imported.insert("team_members".into(), backup.team_members.len());
    records_imported.insert("holidays".into(), backup.holidays.len());
    records_imported.insert("working_day_config".into(), backup.working_day_config.len());
    records_imported.insert("email_templates".into(), backup.email_templates.len());
    records_imported.insert("company_settings".into(), backup.company_settings.len());

    Ok(ImportResult {
        new_company_id,
        new_company_name: backup.company.name.clone(),
        is_overwrite,
        records_imported,
        warnings,
    })
}
