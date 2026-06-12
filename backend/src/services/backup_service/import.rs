use std::collections::HashMap;

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use super::files;
use crate::core::error::{AppError, AppResult};
use crate::models::backup::{CompanyBackup, ImportResult};
use crate::repositories::backup as backup_repo;

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

    let existing_company = backup_repo::find_company_id_by_name(pool, &backup.company.name).await?;
    let is_overwrite = existing_company.is_some();

    let mut remap = HashMap::new();
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

    if is_overwrite {
        warnings.push(format!(
            "Existing company \"{}\" data was overwritten.",
            backup.company.name
        ));

        backup_repo::delete_company_cascade(&mut tx, new_company_id).await?;
        backup_repo::update_company(&mut *tx, new_company_id, &backup.company, now).await?;
    } else {
        backup_repo::insert_company(&mut *tx, new_company_id, &backup.company, now).await?;
    }

    for pg in &backup.payroll_groups {
        backup_repo::insert_payroll_group(&mut *tx, r(pg.id), new_company_id, pg, now).await?;
    }
    for e in &backup.employees {
        backup_repo::insert_employee(
            &mut *tx,
            r(e.id),
            new_company_id,
            ro(e.payroll_group_id),
            e,
            now,
        )
        .await?;
    }
    for a in &backup.employee_allowances {
        backup_repo::insert_employee_allowance(&mut *tx, r(a.id), r(a.employee_id), a, now).await?;
    }
    for s in &backup.salary_history {
        backup_repo::insert_salary_history(&mut *tx, r(s.id), r(s.employee_id), s, now).await?;
    }
    for t in &backup.tp3_records {
        backup_repo::insert_tp3_record(&mut *tx, r(t.id), r(t.employee_id), t, now).await?;
    }
    for lt in &backup.leave_types {
        backup_repo::insert_leave_type(&mut *tx, r(lt.id), new_company_id, lt, now).await?;
    }
    for lb in &backup.leave_balances {
        backup_repo::insert_leave_balance(
            &mut *tx,
            r(lb.id),
            r(lb.employee_id),
            r(lb.leave_type_id),
            lb,
            now,
        )
        .await?;
    }
    for lr in &backup.leave_requests {
        backup_repo::insert_leave_request(
            &mut *tx,
            r(lr.id),
            r(lr.employee_id),
            new_company_id,
            r(lr.leave_type_id),
            lr,
            now,
        )
        .await?;
    }
    for cl in &backup.claims {
        backup_repo::insert_claim(
            &mut *tx,
            r(cl.id),
            r(cl.employee_id),
            new_company_id,
            cl,
            now,
        )
        .await?;
    }
    for ot in &backup.overtime_applications {
        backup_repo::insert_overtime(
            &mut *tx,
            r(ot.id),
            r(ot.employee_id),
            new_company_id,
            ot,
            now,
        )
        .await?;
    }
    for pr in &backup.payroll_runs {
        backup_repo::insert_payroll_run(
            &mut *tx,
            r(pr.id),
            new_company_id,
            r(pr.payroll_group_id),
            pr,
            now,
        )
        .await?;
    }
    for pi in &backup.payroll_items {
        backup_repo::insert_payroll_item(
            &mut *tx,
            r(pi.id),
            r(pi.payroll_run_id),
            r(pi.employee_id),
            pi,
            now,
        )
        .await?;
    }
    for pid in &backup.payroll_item_details {
        backup_repo::insert_payroll_item_detail(
            &mut *tx,
            r(pid.id),
            r(pid.payroll_item_id),
            pid,
            now,
        )
        .await?;
    }
    for pe in &backup.payroll_entries {
        backup_repo::insert_payroll_entry(
            &mut *tx,
            r(pe.id),
            r(pe.employee_id),
            new_company_id,
            ro(pe.payroll_run_id),
            pe,
            now,
        )
        .await?;
    }
    for dc in &backup.document_categories {
        backup_repo::insert_document_category(&mut *tx, r(dc.id), new_company_id, dc, now).await?;
    }
    for d in &backup.documents {
        backup_repo::insert_document(
            &mut *tx,
            r(d.id),
            new_company_id,
            ro(d.employee_id),
            ro(d.category_id),
            d,
            now,
        )
        .await?;
    }
    for t in &backup.teams {
        backup_repo::insert_team(&mut *tx, r(t.id), new_company_id, t, now).await?;
    }
    for tm in &backup.team_members {
        backup_repo::insert_team_member(
            &mut *tx,
            r(tm.id),
            r(tm.team_id),
            r(tm.employee_id),
            tm,
            now,
        )
        .await?;
    }
    for h in &backup.holidays {
        backup_repo::insert_holiday(&mut *tx, r(h.id), new_company_id, h, now).await?;
    }
    for w in &backup.working_day_config {
        backup_repo::insert_working_day_config(&mut *tx, r(w.id), new_company_id, w, now).await?;
    }
    for et in &backup.email_templates {
        backup_repo::insert_email_template(&mut *tx, r(et.id), new_company_id, et, now).await?;
    }
    for cs in &backup.company_settings {
        backup_repo::insert_company_setting(&mut *tx, r(cs.id), new_company_id, cs, now).await?;
    }

    tx.commit().await?;

    if let Some(warning) = files::restore_backup_files(&backup.files).await {
        warnings.push(warning);
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
