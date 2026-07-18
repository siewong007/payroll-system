use std::collections::HashMap;

use chrono::Utc;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::files;
use crate::core::error::{AppError, AppResult};
use crate::models::backup::{CompanyBackup, ImportResult};
use crate::repositories::{backup as backup_repo, companies, user_companies, users};

const MAX_EMPLOYEE_NUMBER_CHARS: usize = 50;

#[derive(Debug, PartialEq, Eq)]
enum AccountImportOutcome {
    Created,
    LinkedExisting,
    SkippedDeleted,
    SkippedConflicting,
}

fn importable_employee_email(
    email: Option<&str>,
    is_active: Option<bool>,
    is_deleted: bool,
) -> Option<&str> {
    if is_deleted || is_active == Some(false) {
        return None;
    }

    email.map(str::trim).filter(|email| !email.is_empty())
}

async fn provision_imported_employee_account(
    conn: &mut PgConnection,
    employee_id: Uuid,
    company_id: Uuid,
    full_name: &str,
    email: &str,
) -> AppResult<AccountImportOutcome> {
    if let Some(existing) = users::find_by_email(&mut *conn, email).await? {
        // A super-admin deletion is a tombstone, not an invitation for a backup
        // restore to recreate access.
        if existing.is_deleted {
            return Ok(AccountImportOutcome::SkippedDeleted);
        }

        // Only reconnect the employee-only account already owned by this exact
        // company. Never repurpose another company's account (or a privileged
        // account) because a backup happens to contain the same email address.
        if existing.roles.as_slice() != ["employee"] || existing.company_id != Some(company_id) {
            return Ok(AccountImportOutcome::SkippedConflicting);
        }

        users::link_to_employee(&mut *conn, employee_id, company_id, existing.id).await?;
        user_companies::insert(&mut *conn, existing.id, company_id).await?;
        return Ok(AccountImportOutcome::LinkedExisting);
    }

    // Backups intentionally exclude credentials. Give the account an unknowable,
    // random temporary secret and require the employee to use Forgot Password.
    let temporary_secret = format!("{}{}", Uuid::new_v4(), Uuid::new_v4());
    let password_hash = bcrypt::hash(temporary_secret, 12)
        .map_err(|e| AppError::Internal(format!("Failed to hash password: {e}")))?;
    let user_id = Uuid::now_v7();

    users::insert_employee_user(
        &mut *conn,
        user_id,
        email,
        &password_hash,
        full_name,
        company_id,
        employee_id,
    )
    .await?;
    user_companies::insert(&mut *conn, user_id, company_id).await?;

    Ok(AccountImportOutcome::Created)
}

fn normalize_employee_number_for_import(
    employee_number: &str,
    is_deleted: bool,
    employee_index: usize,
) -> AppResult<String> {
    let normalized = if is_deleted {
        employee_number
            .rsplit_once("_DEL_")
            .filter(|(_, suffix)| Uuid::parse_str(suffix).is_ok())
            .map_or(employee_number, |(original, _)| original)
    } else {
        employee_number
    };

    if normalized.chars().count() > MAX_EMPLOYEE_NUMBER_CHARS {
        return Err(AppError::BadRequest(format!(
            "Invalid backup: employees[{employee_index}].employee_number exceeds the {MAX_EMPLOYEE_NUMBER_CHARS}-character limit"
        )));
    }

    Ok(normalized.to_owned())
}

pub async fn import_company(
    pool: &PgPool,
    mut backup: CompanyBackup,
    target_company_id: Option<Uuid>,
    importing_user_id: Uuid,
) -> AppResult<ImportResult> {
    if backup.metadata.format_version != "1.0" {
        return Err(AppError::BadRequest(format!(
            "Unsupported backup format version: {}. Expected 1.0",
            backup.metadata.format_version
        )));
    }

    for (index, employee) in backup.employees.iter_mut().enumerate() {
        employee.employee_number = normalize_employee_number_for_import(
            &employee.employee_number,
            employee.deleted_at.is_some(),
            index,
        )?;
    }

    let (new_company_id, new_company_name, is_overwrite) = match target_company_id {
        Some(target_company_id) => {
            let target_company_name = backup_repo::company_name(pool, target_company_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Import target company not found".into()))?;
            (target_company_id, target_company_name, true)
        }
        None => {
            if backup_repo::company_name_exists(pool, &backup.company.name).await? {
                return Err(AppError::Conflict(format!(
                    "A company named \"{}\" already exists. Select it as the restore target to overwrite it.",
                    backup.company.name
                )));
            }
            (Uuid::now_v7(), backup.company.name.clone(), false)
        }
    };

    let mut remap = HashMap::new();
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
    let mut accounts_created = 0usize;
    let mut accounts_linked = 0usize;
    let mut deleted_accounts_skipped = 0usize;
    let mut conflicting_accounts_skipped = 0usize;
    let now = Utc::now();

    if is_overwrite {
        warnings.push(format!(
            "Target company \"{new_company_name}\" data was overwritten from backup \"{}\".",
            backup.company.name,
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
        let employee_id = r(e.id);
        backup_repo::insert_employee(
            &mut *tx,
            employee_id,
            new_company_id,
            ro(e.payroll_group_id),
            e,
            now,
        )
        .await?;

        if let Some(email) =
            importable_employee_email(e.email.as_deref(), e.is_active, e.deleted_at.is_some())
        {
            match provision_imported_employee_account(
                &mut tx,
                employee_id,
                new_company_id,
                &e.full_name,
                email,
            )
            .await?
            {
                AccountImportOutcome::Created => accounts_created += 1,
                AccountImportOutcome::LinkedExisting => accounts_linked += 1,
                AccountImportOutcome::SkippedDeleted => deleted_accounts_skipped += 1,
                AccountImportOutcome::SkippedConflicting => conflicting_accounts_skipped += 1,
            }
        }
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

    // Older backup formats may omit one or more setup domains. Fill only
    // missing defaults before exposing the restored company.
    companies::provision_defaults(&mut *tx, new_company_id, Some(importing_user_id)).await?;

    tx.commit().await?;

    if let Some(warning) = files::restore_backup_files(&backup.files).await {
        warnings.push(warning);
    }
    if accounts_created > 0 {
        warnings.push(format!(
            "Created {accounts_created} employee login account(s). Employees must use Forgot Password before signing in."
        ));
    }
    if deleted_accounts_skipped > 0 {
        warnings.push(format!(
            "Skipped {deleted_accounts_skipped} employee account(s) that were deleted by a super admin."
        ));
    }
    if conflicting_accounts_skipped > 0 {
        warnings.push(format!(
            "Skipped {conflicting_accounts_skipped} employee account(s) because their email belongs to an account outside this company or with a privileged role."
        ));
    }

    let mut records_imported = HashMap::new();
    records_imported.insert("company".into(), 1usize);
    records_imported.insert("payroll_groups".into(), backup.payroll_groups.len());
    records_imported.insert("employees".into(), backup.employees.len());
    records_imported.insert("employee_accounts_created".into(), accounts_created);
    records_imported.insert("employee_accounts_linked".into(), accounts_linked);
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
        new_company_name,
        is_overwrite,
        records_imported,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_UUID: &str = "6c2e7095-7995-44c1-9580-6a751e18ebef";

    #[test]
    fn legacy_deleted_suffix_is_removed_before_length_validation() {
        let legacy_number = format!("EMPTEST001_DEL_{TEST_UUID}");
        assert!(legacy_number.chars().count() > MAX_EMPLOYEE_NUMBER_CHARS);

        let normalized = normalize_employee_number_for_import(&legacy_number, true, 0).unwrap();

        assert_eq!(normalized, "EMPTEST001");
    }

    #[test]
    fn active_employee_number_is_not_rewritten() {
        let number = format!("EMPTEST001_DEL_{TEST_UUID}");
        let error = normalize_employee_number_for_import(&number, false, 3).unwrap_err();

        assert!(matches!(
            error,
            AppError::BadRequest(message)
                if message.contains("employees[3].employee_number")
                    && message.contains("50-character limit")
        ));
    }

    #[test]
    fn invalid_legacy_suffix_is_rejected_with_field_context() {
        let number = format!("{}_DEL_not-a-uuid", "E".repeat(MAX_EMPLOYEE_NUMBER_CHARS));
        let error = normalize_employee_number_for_import(&number, true, 7).unwrap_err();

        assert!(matches!(
            error,
            AppError::BadRequest(message)
                if message.contains("employees[7].employee_number")
                    && message.contains("50-character limit")
        ));
    }

    #[test]
    fn employee_number_limit_counts_characters_not_bytes() {
        let number = "员".repeat(MAX_EMPLOYEE_NUMBER_CHARS);

        assert_eq!(
            normalize_employee_number_for_import(&number, false, 0).unwrap(),
            number
        );
    }

    #[test]
    fn only_active_non_deleted_employees_with_email_get_accounts() {
        assert_eq!(
            importable_employee_email(Some(" employee@example.com "), Some(true), false),
            Some("employee@example.com")
        );
        assert_eq!(
            importable_employee_email(Some("employee@example.com"), Some(false), false),
            None
        );
        assert_eq!(
            importable_employee_email(Some("employee@example.com"), Some(true), true),
            None
        );
        assert_eq!(importable_employee_email(Some("  "), None, false), None);
        assert_eq!(importable_employee_email(None, None, false), None);
    }

    #[tokio::test]
    async fn provisioning_creates_and_then_relinks_an_employee_account() {
        let Some(pool) = crate::tests::support::skip_if_no_db().await else {
            return;
        };
        let company_id = crate::tests::support::seed_company(&pool).await;
        let employee_id =
            crate::tests::support::seed_employee(&pool, company_id, None, 500_000).await;
        let email = format!("import-{}@example.invalid", Uuid::new_v4());
        let mut conn = pool.acquire().await.expect("acquire connection");

        let created = provision_imported_employee_account(
            &mut conn,
            employee_id,
            company_id,
            "Imported Employee",
            &email,
        )
        .await
        .expect("create imported account");
        assert_eq!(created, AccountImportOutcome::Created);

        let account_id = users::find_by_email(&mut *conn, &email)
            .await
            .expect("find account")
            .expect("account exists")
            .id;
        let account = users::get_by_id(&mut *conn, account_id)
            .await
            .expect("load account")
            .expect("account exists");
        assert_eq!(account.employee_id, Some(employee_id));
        assert!(account.must_change_password);
        assert!(
            user_companies::user_has_company(&mut *conn, account_id, company_id)
                .await
                .expect("company membership")
        );

        let replacement_employee_id =
            crate::tests::support::seed_employee(&pool, company_id, None, 500_000).await;
        let linked = provision_imported_employee_account(
            &mut conn,
            replacement_employee_id,
            company_id,
            "Imported Employee",
            &email,
        )
        .await
        .expect("relink imported account");
        assert_eq!(linked, AccountImportOutcome::LinkedExisting);

        let account = users::get_by_id(&mut *conn, account_id)
            .await
            .expect("reload account")
            .expect("account still exists");
        assert_eq!(account.employee_id, Some(replacement_employee_id));
    }
}
