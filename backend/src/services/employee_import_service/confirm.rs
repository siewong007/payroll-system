use chrono::Datelike;
use sqlx::{Executor, PgPool};

/// One committed import row awaiting post-commit provisioning.
struct ProvisionRow {
    row_number: usize,
    employee_id: Uuid,
    email: Option<String>,
    full_name: String,
    ic_number: Option<String>,
    date_joined: chrono::NaiveDate,
}
use uuid::Uuid;

// Per-row savepoint, issued as explicit SQL rather than via sqlx's nested
// `Transaction`. The nested-transaction API resolves through several traits
// depending on what is in scope, and getting it subtly wrong is silent: the
// savepoint is simply never created, the connection stays aborted after the
// first failing row, and the batch behaves exactly as it did before. Naming the
// statements makes that impossible to get wrong by accident.
//
// Issued through `Executor::execute` with a bare `&str`, which sqlx runs on the
// simple query protocol — the same call its own `PgTransactionManager` uses for
// SAVEPOINT/ROLLBACK. That protocol choice matters for the rollback: after a
// failed statement Postgres rejects extended-protocol Parse with 25P02, so an
// extended-protocol `ROLLBACK TO SAVEPOINT` could not clear the very state it
// exists to clear.
const SAVEPOINT_BEGIN: &str = "SAVEPOINT import_row";
const SAVEPOINT_RELEASE: &str = "RELEASE SAVEPOINT import_row";
const SAVEPOINT_ROLLBACK: &str = "ROLLBACK TO SAVEPOINT import_row";

use super::values::{parse_bool, parse_date, parse_money_to_sen};
use crate::core::error::{AppError, AppResult};
use crate::models::employee::CreateEmployeeRequest;
use crate::models::employee_import::*;
use crate::repositories::{bulk_import_sessions, employees as employee_repo, salary_history};
use crate::services::audit_service::{self, AuditRequestMeta};
use crate::services::{employee_service, portal_service};

fn row_to_create_request(row: &ImportRowRaw) -> CreateEmployeeRequest {
    CreateEmployeeRequest {
        employee_number: row.employee_number.clone().unwrap_or_default(),
        full_name: row.full_name.clone().unwrap_or_default(),
        ic_number: row.ic_number.clone(),
        passport_number: row.passport_number.clone(),
        date_of_birth: row.date_of_birth.as_ref().and_then(|s| parse_date(s).ok()),
        gender: row.gender.as_ref().map(|s| s.to_lowercase()),
        nationality: row.nationality.clone(),
        race: row.race.as_ref().map(|s| s.to_lowercase()),
        residency_status: row.residency_status.as_ref().map(|s| s.to_lowercase()),
        marital_status: row.marital_status.as_ref().map(|s| s.to_lowercase()),
        email: row.email.clone(),
        phone: row.phone.clone(),
        address_line1: row.address_line1.clone(),
        address_line2: row.address_line2.clone(),
        city: row.city.clone(),
        state: row.state.clone(),
        postcode: row.postcode.clone(),
        department: row.department.clone(),
        designation: row.designation.clone(),
        cost_centre: row.cost_centre.clone(),
        branch: row.branch.clone(),
        employment_type: row.employment_type.as_ref().map(|s| s.to_lowercase()),
        date_joined: row
            .date_joined
            .as_ref()
            .and_then(|s| parse_date(s).ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive()),
        probation_start: row
            .probation_start
            .as_ref()
            .and_then(|s| parse_date(s).ok()),
        probation_end: row.probation_end.as_ref().and_then(|s| parse_date(s).ok()),
        basic_salary: row
            .basic_salary
            .as_ref()
            .and_then(|s| parse_money_to_sen(s).ok())
            .unwrap_or(0),
        hourly_rate: row
            .hourly_rate
            .as_ref()
            .and_then(|s| parse_money_to_sen(s).ok()),
        daily_rate: row
            .daily_rate
            .as_ref()
            .and_then(|s| parse_money_to_sen(s).ok()),
        bank_name: row.bank_name.clone(),
        bank_account_number: row.bank_account_number.clone(),
        bank_account_type: row.bank_account_type.clone(),
        tax_identification_number: row.tax_identification_number.clone(),
        epf_number: row.epf_number.clone(),
        socso_number: row.socso_number.clone(),
        eis_number: row.eis_number.clone(),
        working_spouse: row.working_spouse.as_ref().and_then(|s| parse_bool(s).ok()),
        num_children: row.num_children.as_ref().and_then(|s| s.parse().ok()),
        epf_category: row.epf_category.clone(),
        is_muslim: row.is_muslim.as_ref().and_then(|s| parse_bool(s).ok()),
        zakat_eligible: row.zakat_eligible.as_ref().and_then(|s| parse_bool(s).ok()),
        zakat_monthly_amount: row
            .zakat_monthly_amount
            .as_ref()
            .and_then(|s| parse_money_to_sen(s).ok()),
        ptptn_monthly_amount: row
            .ptptn_monthly_amount
            .as_ref()
            .and_then(|s| parse_money_to_sen(s).ok()),
        tabung_haji_amount: row
            .tabung_haji_amount
            .as_ref()
            .and_then(|s| parse_money_to_sen(s).ok()),
        payroll_group_id: row
            .payroll_group_id
            .as_ref()
            .and_then(|s| Uuid::parse_str(s).ok()),
        salary_group: row.salary_group.clone(),
    }
}

pub async fn confirm_import(
    pool: &PgPool,
    company_id: Uuid,
    user_id: Uuid,
    req: ImportConfirmRequest,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<ImportConfirmResponse> {
    confirm_import_inner(pool, company_id, user_id, req, audit_meta, None).await
}

/// `confirm_import` with an optional progress reporter — the job executor
/// ticks once per attempted row and once per provisioning step, so a polled
/// status shows a large import moving instead of sitting at 0 until done.
pub(crate) async fn confirm_import_inner(
    pool: &PgPool,
    company_id: Uuid,
    user_id: Uuid,
    req: ImportConfirmRequest,
    audit_meta: Option<&AuditRequestMeta>,
    progress: Option<&crate::services::job_service::JobProgress>,
) -> AppResult<ImportConfirmResponse> {
    let session = bulk_import_sessions::get(pool, req.session_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Import session not found".into()))?;

    let sess_company_id = session.company_id;
    let sess_user_id = session.user_id;
    let validated_data = session.validated_data;
    let status = session.status;
    let expires_at = session.expires_at;

    if sess_company_id != company_id || sess_user_id != user_id {
        return Err(AppError::Forbidden(
            "This import session belongs to another user".into(),
        ));
    }

    if status != "pending" {
        return Err(AppError::BadRequest(format!(
            "Import session is already {}",
            status
        )));
    }

    if expires_at < chrono::Utc::now() {
        return Err(AppError::BadRequest(
            "Import session has expired. Please upload the file again.".into(),
        ));
    }

    let rows: Vec<ImportRowValidation> = serde_json::from_value(validated_data)
        .map_err(|e| AppError::Internal(format!("Failed to deserialize session data: {}", e)))?;

    let (valid_rows, invalid_rows): (Vec<ImportRowValidation>, Vec<ImportRowValidation>) =
        rows.into_iter().partition(|r| r.status == RowStatus::Valid);

    if !req.skip_invalid && !invalid_rows.is_empty() {
        return Err(AppError::BadRequest(format!(
            "Cannot import: {} rows have errors. Set skip_invalid to true to import only valid rows.",
            invalid_rows.len()
        )));
    }

    if let Some(p) = progress {
        p.set_total(valid_rows.len() as i32).await;
    }

    let mut imported_count = 0;
    let mut processed_count = 0i32;
    let mut failed_rows = Vec::new();
    // Employees this import committed, held for post-commit provisioning:
    // portal account + first-year leave balances. The realistic first act of a
    // new tenant is importing the whole headcount — without this, nobody can
    // log in and nobody has an entitlement (plan item 14).
    let mut to_provision: Vec<ProvisionRow> = Vec::new();
    let mut tx = pool.begin().await?;

    for row_validation in &valid_rows {
        let create_req = row_to_create_request(&row_validation.data);
        let id = Uuid::now_v7();

        // Each row gets its own SAVEPOINT. Without one, a single failing INSERT
        // put the connection into Postgres' aborted-transaction state (25P02):
        // every later row failed too, and `COMMIT` on an aborted block is
        // silently executed as ROLLBACK *and reported as success*. The import
        // then answered `{imported_count: N}` having written nothing at all.
        // Rolling back to the savepoint leaves the outer transaction usable, so
        // `skip_invalid` finally means what it says.
        (&mut *tx).execute(SAVEPOINT_BEGIN).await?;

        let mut result =
            employee_repo::insert_bulk_import(&mut *tx, id, company_id, &create_req, user_id).await;

        if result.is_ok() {
            // The initial salary-history row is part of the employee, not an
            // optional extra. This was `let _ =`, so an employee could be
            // created with no salary record and nothing logged anywhere — and
            // on an already-poisoned connection it was guaranteed to fail.
            result = salary_history::insert_bulk_import_initial(
                &mut *tx,
                Uuid::now_v7(),
                id,
                company_id,
                create_req.basic_salary,
                create_req.date_joined,
                user_id,
            )
            .await;
        }

        match result {
            Ok(_) => {
                (&mut *tx).execute(SAVEPOINT_RELEASE).await?;
                imported_count += 1;
                to_provision.push(ProvisionRow {
                    row_number: row_validation.row_number,
                    employee_id: id,
                    email: create_req.email.clone(),
                    full_name: create_req.full_name.clone(),
                    ic_number: create_req.ic_number.clone(),
                    date_joined: create_req.date_joined,
                });
            }
            Err(e) => {
                (&mut *tx).execute(SAVEPOINT_ROLLBACK).await?;
                if req.skip_invalid {
                    failed_rows.push(ImportRowValidation {
                        row_number: row_validation.row_number,
                        status: RowStatus::Error,
                        errors: vec![FieldError {
                            field: "database".into(),
                            // Classified, never the raw Postgres text: that
                            // carries table, column and index names straight to
                            // the import client.
                            message: e.client_response().1,
                        }],
                        data: row_validation.data.clone(),
                    });
                } else {
                    return Err(e);
                }
            }
        }

        processed_count += 1;
        if let Some(p) = progress {
            p.tick(processed_count).await;
        }
    }

    // Claim the session inside the same transaction as the employees. Doing this
    // afterwards on the pool meant a failed import still burned the session, and
    // the caller's `status == "pending"` check above could be passed twice
    // concurrently.
    if !bulk_import_sessions::claim_for_confirmation(&mut *tx, req.session_id).await? {
        return Err(AppError::Conflict(
            "This import session was already confirmed by another request.".into(),
        ));
    }

    let skipped_count = failed_rows.len() + invalid_rows.len();

    // Audited inside the transaction so the trail cannot claim an import that
    // rolled back, nor miss one that committed.
    audit_service::log_action_with_metadata(
        &mut *tx,
        Some(company_id),
        Some(user_id),
        "bulk_import",
        "employee",
        Some(req.session_id),
        None,
        Some(serde_json::json!({
            "session_id": req.session_id,
            "total_imported": imported_count,
            "skipped": skipped_count,
        })),
        Some(&format!("Bulk imported {} employees", imported_count)),
        audit_meta,
    )
    .await?;

    tx.commit().await?;

    // Post-commit, best-effort, exactly like single-employee creation treats
    // these steps: the employees exist either way, and each failure is
    // reported per row instead of failing an import that already committed.
    // `create_user_for_employee_fields` deliberately refuses to adopt foreign
    // or privileged accounts, so re-importing over existing staff is inert.
    // Phase two covers the provisioning loop: the denominator grows by the
    // committed row count so progress stays monotonic across both phases.
    if let Some(p) = progress {
        p.set_total(processed_count + to_provision.len() as i32)
            .await;
    }

    let current_year = chrono::Utc::now().year();
    let mut portal_accounts_created = 0usize;
    let mut leave_balances_created = 0usize;
    let mut provisioning_warnings = Vec::new();
    for ProvisionRow {
        row_number,
        employee_id: id,
        email,
        full_name,
        ic_number,
        date_joined,
    } in &to_provision
    {
        // No address on the row means nothing to log in with: skip silently,
        // matching single-employee creation.
        if let Some(email) = email.as_deref().filter(|e| !e.trim().is_empty()) {
            match employee_service::create_user_for_employee_fields(
                pool,
                *id,
                company_id,
                email,
                full_name,
                ic_number.as_deref(),
            )
            .await
            {
                Ok(Some(_)) => portal_accounts_created += 1,
                Ok(None) => {}
                Err(e) => provisioning_warnings.push(format!(
                    "Row {row_number}: portal account not created: {}",
                    e.client_response().1
                )),
            }
        }

        match portal_service::initialize_leave_balances(
            pool,
            *id,
            company_id,
            *date_joined,
            current_year,
        )
        .await
        {
            Ok(balances) => leave_balances_created += balances.len(),
            Err(e) => provisioning_warnings.push(format!(
                "Row {row_number}: leave balances not initialised: {}",
                e.client_response().1
            )),
        }

        processed_count += 1;
        if let Some(p) = progress {
            p.tick(processed_count).await;
        }
    }

    Ok(ImportConfirmResponse {
        imported_count,
        skipped_count,
        errors: failed_rows,
        portal_accounts_created,
        leave_balances_created,
        provisioning_warnings,
    })
}
