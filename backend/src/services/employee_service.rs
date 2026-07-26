use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::employee::{
    CreateEmployeeRequest, CreateTp3Request, Employee, EmployeeAccountInfo, SalaryHistory,
    Tp3Record, UpdateEmployeeRequest,
};
use crate::repositories::{
    employees, refresh_tokens, salary_history, tp3_records, user_companies, users,
};
use crate::services::audit_service::AuditRequestMeta;

pub async fn list_employees(
    pool: &PgPool,
    company_id: Uuid,
    search: Option<&str>,
    department: Option<&str>,
    is_active: Option<bool>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<Employee>, i64)> {
    let total = employees::count(pool, company_id, search, department, is_active).await?;
    let items = employees::list(
        pool, company_id, search, department, is_active, limit, offset,
    )
    .await?;
    Ok((items, total))
}

pub async fn get_employee(pool: &PgPool, id: Uuid, company_id: Uuid) -> AppResult<Employee> {
    employees::get(pool, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Employee not found".into()))
}

pub async fn create_employee(
    pool: &PgPool,
    company_id: Uuid,
    req: CreateEmployeeRequest,
    created_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<(Employee, Option<EmployeeAccountInfo>)> {
    // Check for duplicate employee number within the same company
    if employees::exists_by_number(pool, company_id, &req.employee_number).await? {
        return Err(AppError::Conflict(format!(
            "Employee number '{}' already exists in this company",
            req.employee_number
        )));
    }

    let id = Uuid::now_v7();
    let emp = employees::insert(pool, id, company_id, &req, created_by).await?;

    // Auto-create a user account for the employee if they have an email
    let account_info = create_user_for_employee(pool, &emp).await?;

    // Initialize leave balances for the current year (prorated for mid-year joiners)
    //
    // Logged rather than discarded: `initialize_leave_balances` does not log
    // internally either, so a half-provisioned employee — created, with an
    // account, and no leave entitlement — used to produce no output at any level.
    // Still non-fatal: the employee and their account exist and are usable, and
    // balances can be re-initialised from the UI.
    let current_year = chrono::Utc::now().year();
    if let Err(e) = crate::services::portal_service::initialize_leave_balances(
        pool,
        emp.id,
        company_id,
        emp.date_joined,
        current_year,
    )
    .await
    {
        tracing::error!(
            employee_id = %emp.id,
            company_id = %company_id,
            year = current_year,
            error = %e,
            "Failed to initialise leave balances for a newly created employee"
        );
    }

    // Audit Log
    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(created_by),
        "create_employee",
        "employee",
        Some(emp.id),
        None,
        Some(serde_json::to_value(&emp).unwrap_or_default()),
        Some(&format!(
            "Created employee {} ({})",
            emp.full_name, emp.employee_number
        )),
        audit_meta,
    )
    .await;

    Ok((emp, account_info))
}

pub async fn create_user_for_employee(
    pool: &PgPool,
    emp: &Employee,
) -> AppResult<Option<EmployeeAccountInfo>> {
    let Some(ref email) = emp.email else {
        return Ok(None);
    };

    // Check if email already exists
    if let Some(existing) = users::find_by_email(pool, email).await? {
        let existing_id = existing.id;

        // Never touch a soft-deleted account: hard-deleting and recreating it
        // would resurrect a tombstone under a caller-chosen password, and
        // linking it would revive an account that must not authenticate.
        if existing.is_deleted {
            tracing::warn!(
                employee_id = %emp.id,
                "employee email matches a deleted user account; skipping portal account creation"
            );
            return Ok(None);
        }

        // Never touch an account belonging to another company. Both branches
        // below rewrite the account's company/employee binding, so without this
        // any caller could pull a foreign tenant's user into their own company.
        if existing.company_id != Some(emp.company_id) {
            tracing::warn!(
                employee_id = %emp.id,
                "employee email belongs to a user in another company; skipping portal account creation"
            );
            return Ok(None);
        }

        if existing.roles.as_slice() == ["employee"] {
            // Stale employee account in this same company — clean up and recreate below
            user_companies::delete_by_user(pool, existing_id).await?;
            refresh_tokens::delete_by_user(pool, existing_id).await?;
            users::delete(pool, existing_id).await?;
        } else {
            // Never adopt a privileged account into an employee record. Linking
            // it would hand this employee's lifecycle control over that account:
            // `soft_delete_employee` hard-deletes whatever `employee_id` points
            // at, so an employee-manager could capture and permanently destroy a
            // super_admin whose active company happens to be this one —
            // bypassing `require_super_admin`, the self-delete guard and the
            // tombstone that stops the account being recreated.
            tracing::warn!(
                employee_id = %emp.id,
                "employee email belongs to a privileged account; skipping portal account creation"
            );
            return Ok(None);
        }
    }

    // Default password: IC number or "Welcome@123" if no IC
    let default_password = emp.ic_number.as_deref().unwrap_or("Welcome@123");
    let password_hash = bcrypt::hash(default_password, 12)
        .map_err(|e| AppError::Internal(format!("Failed to hash password: {}", e)))?;

    let user_id = Uuid::now_v7();
    users::insert_employee_user(
        pool,
        user_id,
        email,
        &password_hash,
        &emp.full_name,
        emp.company_id,
        emp.id,
    )
    .await?;

    // Link user to company
    user_companies::insert(pool, user_id, emp.company_id).await?;

    Ok(Some(EmployeeAccountInfo {
        created: true,
        email: email.clone(),
        role: "employee".into(),
        default_password: Some(default_password.to_string()),
        message: format!(
            "User account created for {}. Default password is their IC number.",
            emp.full_name
        ),
    }))
}

pub async fn update_employee(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
    req: UpdateEmployeeRequest,
    updated_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Employee> {
    let existing = get_employee(pool, id, company_id).await?;

    // The employee row is written first, and both writes share one transaction.
    //
    // This is not a race — it was deterministic. `employees::update` casts
    // free-text into enum columns (`gender_type`, `race_type`,
    // `employment_type`), so one PUT carrying a new salary *and* an invalid
    // gender committed a salary-history row asserting a raise and then failed on
    // the employee update. The salary history is what the audit trail and the EA
    // form read, so it recorded a raise that never happened.
    let mut tx = pool.begin().await?;
    let emp = employees::update(&mut *tx, id, company_id, &req, updated_by).await?;

    // Track salary change
    if let Some(new_salary) = req.basic_salary
        && new_salary != existing.basic_salary
    {
        let history_id = Uuid::now_v7();
        salary_history::insert(
            &mut *tx,
            history_id,
            id,
            existing.basic_salary,
            new_salary,
            updated_by,
        )
        .await?;
    }
    tx.commit().await?;

    // Audit Log
    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(updated_by),
        "update_employee",
        "employee",
        Some(emp.id),
        Some(serde_json::to_value(&existing).unwrap_or_default()),
        Some(serde_json::to_value(&emp).unwrap_or_default()),
        Some(&format!(
            "Updated employee {} ({})",
            emp.full_name, emp.employee_number
        )),
        audit_meta,
    )
    .await;

    Ok(emp)
}

/// Soft-delete an employee and tear down their portal login.
///
/// All four writes share one transaction. Split across connections, a failure
/// part-way left the employee soft-deleted with a live account and valid refresh
/// tokens — or, in the other order, an orphaned login for a record that still
/// existed. Nothing retried it, because the first write had already succeeded.
///
/// It is also audited now. This is the most destructive HR mutation in the
/// system and it recorded nothing at all, while `create_employee` and
/// `update_employee` either side of it both capture actor, before/after and
/// request metadata. The prior state goes into `old_values` so the row itself is
/// the record of what was removed.
pub async fn soft_delete_employee(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
    deleted_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    // Read before deleting: afterwards there is nothing left to describe.
    let existing = get_employee(pool, id, company_id).await?;

    let mut tx = pool.begin().await?;
    let rows = employees::soft_delete(&mut *tx, id, company_id).await?;

    if rows == 0 {
        return Err(AppError::NotFound("Employee not found".into()));
    }

    // Retire the portal login linked to this employee. The company links and
    // refresh tokens really are deleted — they are access, not history — but the
    // user row is soft-deleted so the audit trail, payroll approvals and
    // attendance records that reference it stay intact.
    user_companies::delete_by_employee(&mut *tx, id).await?;
    refresh_tokens::delete_by_employee(&mut *tx, id).await?;
    users::soft_delete_by_employee(&mut *tx, id, deleted_by).await?;

    crate::services::audit_service::log_action_with_metadata(
        &mut *tx,
        Some(company_id),
        Some(deleted_by),
        "delete_employee",
        "employee",
        Some(id),
        Some(serde_json::to_value(&existing).unwrap_or_default()),
        None,
        Some(&format!(
            "Deleted employee {} ({})",
            existing.full_name, existing.employee_number
        )),
        audit_meta,
    )
    .await?;

    tx.commit().await?;

    Ok(())
}

pub async fn get_salary_history(pool: &PgPool, employee_id: Uuid) -> AppResult<Vec<SalaryHistory>> {
    salary_history::list_by_employee(pool, employee_id).await
}

pub async fn create_tp3(
    pool: &PgPool,
    employee_id: Uuid,
    req: CreateTp3Request,
    created_by: Uuid,
) -> AppResult<Tp3Record> {
    let id = Uuid::now_v7();
    tp3_records::upsert(pool, id, employee_id, &req, created_by).await
}
