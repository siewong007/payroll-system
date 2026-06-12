use std::collections::HashMap;

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use super::files;
use crate::core::error::{AppError, AppResult};
use crate::models::backup::{BackupMetadata, CompanyBackup};
use crate::repositories::backup as backup_repo;

pub async fn export_company(pool: &PgPool, company_id: Uuid) -> AppResult<CompanyBackup> {
    let company = backup_repo::company(pool, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Company not found".into()))?;

    let payroll_groups = backup_repo::payroll_groups(pool, company_id).await?;
    let employees = backup_repo::employees(pool, company_id).await?;
    let employee_allowances = backup_repo::employee_allowances(pool, company_id).await?;
    let salary_history = backup_repo::salary_history(pool, company_id).await?;
    let tp3_records = backup_repo::tp3_records(pool, company_id).await?;
    let leave_types = backup_repo::leave_types(pool, company_id).await?;
    let leave_balances = backup_repo::leave_balances(pool, company_id).await?;
    let leave_requests = backup_repo::leave_requests(pool, company_id).await?;
    let claims = backup_repo::claims(pool, company_id).await?;
    let overtime_applications = backup_repo::overtime_applications(pool, company_id).await?;

    let payroll_runs = backup_repo::payroll_runs(pool, company_id).await?;
    let payroll_items = backup_repo::payroll_items(pool, company_id).await?;
    let payroll_item_details = backup_repo::payroll_item_details(pool, company_id).await?;
    let payroll_entries = backup_repo::payroll_entries(pool, company_id).await?;
    let document_categories = backup_repo::document_categories(pool, company_id).await?;
    let documents = backup_repo::documents(pool, company_id).await?;
    let teams = backup_repo::teams(pool, company_id).await?;
    let team_members = backup_repo::team_members(pool, company_id).await?;
    let holidays = backup_repo::holidays(pool, company_id).await?;
    let working_day_config = backup_repo::working_day_config(pool, company_id).await?;
    let email_templates = backup_repo::email_templates(pool, company_id).await?;
    let company_settings = backup_repo::company_settings(pool, company_id).await?;

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

    let files = files::collect_backup_files(&documents, &leave_requests, &claims);

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
