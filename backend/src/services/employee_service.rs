use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::employee::{
    CreateEmployeeRequest, CreateTp3Request, Employee, EmployeeAccountInfo, SalaryHistory,
    Tp3Record, UpdateEmployeeRequest,
};
use crate::repositories::{
    employees, refresh_tokens, salary_history, tp3_records, user_companies, user_sessions, users,
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

#[allow(clippy::too_many_arguments)]
fn validate_employee_fields(
    basic_salary: i64,
    hourly_rate: Option<i64>,
    daily_rate: Option<i64>,
    num_children: Option<i32>,
    zakat_monthly_amount: Option<i64>,
    ptptn_monthly_amount: Option<i64>,
    tabung_haji_amount: Option<i64>,
    date_joined: chrono::NaiveDate,
    probation_start: Option<chrono::NaiveDate>,
    probation_end: Option<chrono::NaiveDate>,
    confirmation_date: Option<chrono::NaiveDate>,
) -> AppResult<()> {
    if basic_salary < 0 {
        return Err(AppError::Validation(
            "Basic salary cannot be negative".into(),
        ));
    }
    if basic_salary > crate::models::payroll::MAX_MONEY_SEN {
        return Err(AppError::Validation(
            "Basic salary exceeds supported maximum".into(),
        ));
    }
    if hourly_rate.is_some_and(|v| v < 0) {
        return Err(AppError::Validation(
            "Hourly rate cannot be negative".into(),
        ));
    }
    if hourly_rate.is_some_and(|v| v > crate::models::payroll::MAX_MONEY_SEN) {
        return Err(AppError::Validation(
            "Hourly rate exceeds supported maximum".into(),
        ));
    }
    if daily_rate.is_some_and(|v| v < 0) {
        return Err(AppError::Validation("Daily rate cannot be negative".into()));
    }
    if daily_rate.is_some_and(|v| v > crate::models::payroll::MAX_MONEY_SEN) {
        return Err(AppError::Validation(
            "Daily rate exceeds supported maximum".into(),
        ));
    }
    if num_children.is_some_and(|v| !(0..=30).contains(&v)) {
        return Err(AppError::Validation(
            "Number of children must be between 0 and 30".into(),
        ));
    }
    if zakat_monthly_amount.is_some_and(|v| v < 0) {
        return Err(AppError::Validation(
            "Monthly Zakat amount cannot be negative".into(),
        ));
    }
    if zakat_monthly_amount.is_some_and(|v| v > crate::models::payroll::MAX_MONEY_SEN) {
        return Err(AppError::Validation(
            "Monthly Zakat amount exceeds supported maximum".into(),
        ));
    }
    if ptptn_monthly_amount.is_some_and(|v| v < 0) {
        return Err(AppError::Validation(
            "Monthly PTPTN amount cannot be negative".into(),
        ));
    }
    if ptptn_monthly_amount.is_some_and(|v| v > crate::models::payroll::MAX_MONEY_SEN) {
        return Err(AppError::Validation(
            "Monthly PTPTN amount exceeds supported maximum".into(),
        ));
    }
    if tabung_haji_amount.is_some_and(|v| v < 0) {
        return Err(AppError::Validation(
            "Monthly Tabung Haji amount cannot be negative".into(),
        ));
    }
    if tabung_haji_amount.is_some_and(|v| v > crate::models::payroll::MAX_MONEY_SEN) {
        return Err(AppError::Validation(
            "Monthly Tabung Haji amount exceeds supported maximum".into(),
        ));
    }
    if let (Some(start), Some(end)) = (probation_start, probation_end)
        && end < start
    {
        return Err(AppError::Validation(
            "Probation end date cannot be earlier than probation start date".into(),
        ));
    }
    if let Some(conf) = confirmation_date
        && conf < date_joined
    {
        return Err(AppError::Validation(
            "Confirmation date cannot be earlier than join date".into(),
        ));
    }
    Ok(())
}

pub async fn create_employee(
    pool: &PgPool,
    company_id: Uuid,
    mut req: CreateEmployeeRequest,
    created_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<(Employee, Option<EmployeeAccountInfo>)> {
    req.email = req
        .email
        .take()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    validate_employee_fields(
        req.basic_salary,
        req.hourly_rate,
        req.daily_rate,
        req.num_children,
        req.zakat_monthly_amount,
        req.ptptn_monthly_amount,
        req.tabung_haji_amount,
        req.date_joined,
        req.probation_start,
        req.probation_end,
        None,
    )?;

    // Check for duplicate employee number or normalized email within the same company
    if employees::exists_by_number(pool, company_id, &req.employee_number).await? {
        return Err(AppError::Conflict(format!(
            "Employee number '{}' already exists in this company",
            req.employee_number
        )));
    }
    if let Some(ref email) = req.email
        && employees::exists_by_normalized_email(pool, company_id, email, None).await?
    {
        return Err(AppError::Conflict(
            "An active employee already uses that email address.".into(),
        ));
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
    let Some(email) = emp
        .email
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty())
    else {
        return Ok(None);
    };
    create_user_for_employee_fields(
        pool,
        emp.id,
        emp.company_id,
        email,
        &emp.full_name,
        emp.ic_number.as_deref(),
    )
    .await
}

fn random_index_unbiased(len: usize) -> usize {
    debug_assert!(len > 0 && len <= 256);
    let limit = 256 - (256 % len);
    loop {
        let bytes = *Uuid::new_v4().as_bytes();
        // Exclude UUIDv4 version (byte 6) and variant (byte 8) bit-masked bytes.
        for &idx in &[0usize, 1, 2, 3, 4, 5, 7, 9, 10, 11, 12, 13, 14, 15] {
            let v = bytes[idx] as usize;
            if v < limit {
                return v % len;
            }
        }
    }
}

fn generate_random_password() -> String {
    const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    const DIGIT: &[u8] = b"0123456789";
    const SYMBOL: &[u8] = b"!@#$%^&*";
    const ALL: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*";

    let mut chars = Vec::with_capacity(16);
    chars.push(UPPER[random_index_unbiased(UPPER.len())]);
    chars.push(LOWER[random_index_unbiased(LOWER.len())]);
    chars.push(DIGIT[random_index_unbiased(DIGIT.len())]);
    chars.push(SYMBOL[random_index_unbiased(SYMBOL.len())]);
    for _ in 0..12 {
        chars.push(ALL[random_index_unbiased(ALL.len())]);
    }
    for i in (1..chars.len()).rev() {
        let j = random_index_unbiased(i + 1);
        chars.swap(i, j);
    }
    String::from_utf8(chars).expect("ASCII password bytes")
}

/// Primitive-based form so callers holding row data — the bulk importer —
/// can provision portal accounts without materialising a full `Employee`
/// (plan item 14). Same semantics as [`create_user_for_employee`].
pub async fn create_user_for_employee_fields(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    email: &str,
    full_name: &str,
    _ic_number: Option<&str>,
) -> AppResult<Option<EmployeeAccountInfo>> {
    // Check if email already exists
    if let Some(existing) = users::find_by_email(pool, email).await? {
        // Never touch a soft-deleted account: hard-deleting and recreating it
        // would resurrect a tombstone under a caller-chosen password, and
        // linking it would revive an account that must not authenticate.
        if existing.is_deleted {
            tracing::warn!(
                employee_id = %employee_id,
                "employee email matches a deleted user account; skipping portal account creation"
            );
            return Ok(None);
        }

        // Never touch an account belonging to another company.
        if existing.company_id != Some(company_id) {
            tracing::warn!(
                employee_id = %employee_id,
                "employee email belongs to a user in another company; skipping portal account creation"
            );
            return Ok(None);
        }

        if existing.roles.as_slice() == ["employee"] {
            // Never delete or commandeer an existing employee account on duplicate email.
            tracing::warn!(
                employee_id = %employee_id,
                existing_user_id = %existing.id,
                "employee email already belongs to an existing employee account; skipping portal account creation"
            );
            return Ok(None);
        } else {
            // Never adopt a privileged account into an employee record.
            tracing::warn!(
                employee_id = %employee_id,
                "employee email belongs to a privileged account; skipping portal account creation"
            );
            return Ok(None);
        }
    }

    let default_password = generate_random_password();
    // bcrypt at cost 12 is ~250ms of blocking CPU. In the bulk-import path this
    // runs once per imported employee, so off the async worker it must go —
    // the same spawn_blocking auth_service already uses for login hashing.
    let hash_input = default_password.clone();
    let password_hash = tokio::task::spawn_blocking(move || bcrypt::hash(&hash_input, 12))
        .await
        .map_err(|e| AppError::Internal(format!("Password hashing task failed: {}", e)))?
        .map_err(|e| AppError::Internal(format!("Failed to hash password: {}", e)))?;

    let user_id = Uuid::now_v7();
    let mut tx = pool.begin().await?;
    users::insert_employee_user(
        &mut *tx,
        user_id,
        email,
        &password_hash,
        full_name,
        company_id,
        employee_id,
    )
    .await?;

    // Link user to company
    user_companies::insert(&mut *tx, user_id, company_id).await?;
    tx.commit().await?;

    Ok(Some(EmployeeAccountInfo {
        created: true,
        email: email.to_string(),
        role: "employee".into(),
        default_password: Some(default_password),
        message: format!(
            "Temporary portal password generated for {full_name}; password change is required on first sign-in."
        ),
    }))
}

fn update_touches_self_restricted_fields(req: &UpdateEmployeeRequest, existing: &Employee) -> bool {
    let changing_email = (req.clear_email == Some(true) && existing.email.is_some())
        || req.email.as_deref().is_some_and(|e| {
            existing
                .email
                .as_deref()
                .is_none_or(|old| !old.trim().eq_ignore_ascii_case(e.trim()))
        });
    let changing_active = req.is_active.is_some_and(|a| Some(a) != existing.is_active);
    changing_email
        || changing_active
        || req.basic_salary.is_some()
        || req.hourly_rate.is_some()
        || req.daily_rate.is_some()
        || req.bank_name.is_some()
        || req.bank_account_number.is_some()
        || req.bank_account_type.is_some()
        || req.clear_bank_name == Some(true)
        || req.clear_bank_account_number == Some(true)
        || req.tax_identification_number.is_some()
        || req.epf_number.is_some()
        || req.socso_number.is_some()
        || req.eis_number.is_some()
        || req.working_spouse.is_some()
        || req.num_children.is_some()
        || req.epf_category.is_some()
        || req.clear_epf_category == Some(true)
        || req.is_muslim.is_some()
        || req.zakat_eligible.is_some()
        || req.zakat_monthly_amount.is_some()
        || req.ptptn_monthly_amount.is_some()
        || req.tabung_haji_amount.is_some()
        || req.hrdf_contribution.is_some()
        || req.payroll_group_id.is_some()
        || req.clear_payroll_group_id == Some(true)
        || req.salary_group.is_some()
        || req.clear_salary_group == Some(true)
}

pub async fn update_employee(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
    mut req: UpdateEmployeeRequest,
    updated_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Employee> {
    if let Some(raw_email) = req.email.take() {
        let trimmed = raw_email.trim().to_string();
        if trimmed.is_empty() {
            req.clear_email = Some(true);
        } else {
            req.email = Some(trimmed);
        }
    }
    let clearing = req.clear_date_resigned.unwrap_or(false);
    if clearing && req.date_resigned.is_some() {
        return Err(AppError::BadRequest(
            "Cannot set and clear the resignation date in the same request".into(),
        ));
    }
    let clearing_epf = req.clear_epf_category.unwrap_or(false);
    if clearing_epf && req.epf_category.is_some() {
        return Err(AppError::BadRequest(
            "Cannot set and clear epf_category in the same request".into(),
        ));
    }
    if req.clear_payroll_group_id == Some(true) && req.payroll_group_id.is_some() {
        return Err(AppError::BadRequest(
            "Cannot set and clear payroll_group_id in the same request".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    let existing = employees::get_locked(&mut *tx, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Employee not found".into()))?;

    if let Some(actor) = crate::repositories::users::get_by_id(&mut *tx, updated_by).await?
        && !actor.roles.iter().any(|r| r == "super_admin")
        && crate::repositories::users::is_employee_owned_by_user(
            &mut *tx, updated_by, id, company_id,
        )
        .await?
        && update_touches_self_restricted_fields(&req, &existing)
    {
        return Err(AppError::Forbidden(
            "You cannot modify your own compensation, banking, statutory, email, or active status fields"
                .into(),
        ));
    }

    if let Some(ref new_email) = req.email
        && employees::exists_by_normalized_email(&mut *tx, company_id, new_email, Some(id)).await?
    {
        return Err(AppError::Conflict(
            "An active employee already uses that email address.".into(),
        ));
    }

    let effective_resigned = if clearing {
        None
    } else {
        req.date_resigned.or(existing.date_resigned)
    };
    if let Some(resigned) = effective_resigned
        && resigned < existing.date_joined
    {
        return Err(AppError::BadRequest(format!(
            "Resignation date {resigned} is earlier than the date joined {}. Correct the resignation date or clear it.",
            existing.date_joined
        )));
    }

    validate_employee_fields(
        req.basic_salary.unwrap_or(existing.basic_salary),
        req.hourly_rate.or(existing.hourly_rate),
        req.daily_rate.or(existing.daily_rate),
        req.num_children.or(existing.num_children),
        req.zakat_monthly_amount.or(existing.zakat_monthly_amount),
        req.ptptn_monthly_amount.or(existing.ptptn_monthly_amount),
        req.tabung_haji_amount.or(existing.tabung_haji_amount),
        existing.date_joined,
        req.probation_start.or(existing.probation_start),
        req.probation_end.or(existing.probation_end),
        req.confirmation_date.or(existing.confirmation_date),
    )?;

    let emp = employees::update(&mut *tx, id, company_id, &req, updated_by).await?;

    // Track salary change
    if let Some(new_salary) = req.basic_salary
        && new_salary != existing.basic_salary
    {
        // Mirrors `salary_history_change_type_check` — validated here so a bad
        // value fails as 400 naming the field, not as a constraint violation.
        const CHANGE_TYPES: &[&str] = &[
            "new_hire",
            "increment",
            "promotion",
            "demotion",
            "contract_change",
            "correction",
            "backdated_adjustment",
            "termination",
            "adjustment",
        ];
        let change_type = req.salary_change_type.as_deref().unwrap_or("adjustment");
        if !CHANGE_TYPES.contains(&change_type) {
            return Err(AppError::BadRequest(format!(
                "Invalid salary_change_type '{change_type}' — expected one of: {}",
                CHANGE_TYPES.join(", ")
            )));
        }
        let notes = req
            .salary_change_notes
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.chars().take(500).collect::<String>());
        let history_id = Uuid::now_v7();
        salary_history::insert(
            &mut *tx,
            history_id,
            id,
            company_id,
            existing.basic_salary,
            new_salary,
            change_type,
            notes.as_deref(),
            updated_by,
        )
        .await?;
    }

    if req.is_active == Some(false) {
        user_sessions::revoke_by_employee(&mut *tx, id).await?;
        refresh_tokens::delete_by_employee(&mut *tx, id).await?;
        crate::repositories::mfa_challenges::revoke_for_employee(&mut *tx, id).await?;
        crate::repositories::push_subscriptions::revoke_all_for_employee(&mut *tx, id).await?;
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

    if let Some(actor) = users::get_by_id(pool, deleted_by).await?
        && !actor.roles.iter().any(|r| r == "super_admin")
        && users::is_employee_owned_by_user(pool, deleted_by, id, company_id).await?
    {
        return Err(AppError::Forbidden(
            "You cannot delete your own employee record".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    let rows = employees::soft_delete(&mut *tx, id, company_id).await?;

    if rows == 0 {
        return Err(AppError::NotFound("Employee not found".into()));
    }

    // Retire the portal login linked to this employee. The company links and
    // refresh tokens really are deleted — they are access, not history — but the
    // user row is soft-deleted so the audit trail, payroll approvals and
    // attendance records that reference it stay intact.
    //
    // A staff login linked to the record (an employee promoted to a staff role)
    // only loses this company: its membership here goes, its sessions are
    // revoked so it re-authenticates without the stale employee link, and the
    // link itself is cleared last (the revocations select by `employee_id`).
    user_companies::delete_by_employee(&mut *tx, id, company_id).await?;
    user_sessions::revoke_by_employee(&mut *tx, id).await?;
    refresh_tokens::delete_by_employee(&mut *tx, id).await?;
    crate::repositories::mfa_challenges::revoke_for_employee(&mut *tx, id).await?;
    crate::repositories::push_subscriptions::revoke_all_for_employee(&mut *tx, id).await?;
    crate::repositories::password_reset_requests::expire_pending_for_employee(&mut *tx, id).await?;
    users::soft_delete_by_employee(&mut *tx, id, deleted_by).await?;
    users::unlink_staff_from_employee(&mut *tx, id, company_id).await?;

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

/// Whether a caller holding `caller_roles` may cut off a login holding
/// `target_roles` — by deactivating or deleting the employee record it is
/// linked to. Only `super_admin` may do that to a `super_admin`, and only an
/// `admin` or `super_admin` to an `admin`. Everyone else is fair game for any
/// holder of `ManageEmployees`, matching who may manage employees at all.
pub fn may_revoke_login(caller_roles: &[String], target_roles: &[String]) -> bool {
    let holds = |roles: &[String], role: &str| roles.iter().any(|r| r == role);
    if holds(target_roles, "super_admin") && !holds(caller_roles, "super_admin") {
        return false;
    }
    if holds(target_roles, "admin")
        && !holds(caller_roles, "admin")
        && !holds(caller_roles, "super_admin")
    {
        return false;
    }
    true
}

/// Refuses to deactivate or delete an employee whose linked login outranks the
/// caller. Deactivation fails every request that login makes (the auth
/// extractor rejects an inactive employee), so without this a manager or HR
/// user could lock an administrator out of the platform.
pub async fn ensure_may_revoke_linked_logins(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    caller_roles: &[String],
) -> AppResult<()> {
    // Company-scoped read first, so a foreign id is NotFound, not Forbidden.
    get_employee(pool, employee_id, company_id).await?;
    for target_roles in users::roles_linked_to_employee(pool, employee_id, company_id).await? {
        if !may_revoke_login(caller_roles, &target_roles) {
            return Err(AppError::Forbidden(
                "This employee is linked to a login with a higher role than yours; ask a super admin".into(),
            ));
        }
    }
    Ok(())
}

pub async fn get_salary_history(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
) -> AppResult<Vec<SalaryHistory>> {
    salary_history::list_by_employee(pool, employee_id, company_id).await
}

pub async fn create_tp3(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    req: CreateTp3Request,
    created_by: Uuid,
) -> AppResult<Tp3Record> {
    if let Some(actor) = users::get_by_id(pool, created_by).await?
        && !actor.roles.iter().any(|r| r == "super_admin")
        && users::is_employee_owned_by_user(pool, created_by, employee_id, company_id).await?
    {
        return Err(AppError::Forbidden(
            "You cannot record TP3 prior-employer tax figures for yourself".into(),
        ));
    }
    if !(2000..=2200).contains(&req.tax_year) {
        return Err(AppError::BadRequest(
            "TP3 tax_year must be between 2000 and 2200".into(),
        ));
    }
    let zakat_ytd = req.previous_zakat_ytd.unwrap_or(0);
    if req.previous_income_ytd < 0
        || req.previous_epf_ytd < 0
        || req.previous_pcb_ytd < 0
        || req.previous_socso_ytd < 0
        || zakat_ytd < 0
    {
        return Err(AppError::BadRequest(
            "TP3 prior-employer YTD amounts cannot be negative".into(),
        ));
    }
    let max_sen = crate::models::payroll::MAX_MONEY_SEN;
    if req.previous_income_ytd > max_sen
        || req.previous_epf_ytd > max_sen
        || req.previous_pcb_ytd > max_sen
        || req.previous_socso_ytd > max_sen
        || zakat_ytd > max_sen
    {
        return Err(AppError::BadRequest(
            "TP3 prior-employer YTD amount exceeds supported maximum".into(),
        ));
    }
    let id = Uuid::now_v7();
    let record = tp3_records::upsert(pool, id, employee_id, company_id, &req, created_by).await?;
    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(created_by),
        "upsert_tp3",
        "tp3_record",
        Some(record.id),
        None,
        Some(serde_json::json!({
            "employee_id": employee_id,
            "tax_year": record.tax_year,
            "previous_income_ytd": record.previous_income_ytd,
            "previous_epf_ytd": record.previous_epf_ytd,
            "previous_pcb_ytd": record.previous_pcb_ytd,
            "previous_socso_ytd": record.previous_socso_ytd,
            "previous_zakat_ytd": record.previous_zakat_ytd,
        })),
        Some(&format!(
            "Saved TP3 record for tax year {}",
            record.tax_year
        )),
        None,
    )
    .await;
    Ok(record)
}

/// Spawn asynchronous welcome email delivery when employee creation also
/// provisioned a brand-new user account with an email address.
pub async fn dispatch_welcome_email_if_created(
    mailer: &crate::services::email_service::SharedMailer,
    config: &crate::core::config::AppConfig,
    pool: &PgPool,
    company_id: Uuid,
    emp: &Employee,
    account_info: Option<&EmployeeAccountInfo>,
    actor_id: Uuid,
) -> AppResult<()> {
    let Some(info) = account_info else {
        return Ok(());
    };
    if !info.created {
        return Ok(());
    }
    let Some(ref email_addr) = emp.email else {
        return Ok(());
    };

    let company = crate::services::company_service::get_company(pool, company_id).await?;
    let mailer = mailer.clone();
    let config = config.clone();
    let pool = pool.clone();
    let company_name = company.name;
    let emp_id = emp.id;
    let emp_name = emp.full_name.clone();
    let email = email_addr.clone();
    let default_pw = info
        .default_password
        .clone()
        .unwrap_or_else(|| "(your IC number)".to_string());

    if let Err(e) = crate::services::email_service::send_welcome_email(
        &mailer,
        &config,
        &pool,
        company_id,
        &company_name,
        emp_id,
        &emp_name,
        &email,
        &default_pw,
        actor_id,
    )
    .await
    {
        tracing::error!(
            "Failed to dispatch welcome email for employee {}: {}",
            emp_id,
            e
        );
    }

    Ok(())
}

#[cfg(test)]
mod revoke_rank_tests {
    use super::may_revoke_login;

    fn r(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_string()).collect()
    }

    #[test]
    fn only_super_admin_may_revoke_a_super_admin() {
        assert!(may_revoke_login(&r(&["super_admin"]), &r(&["super_admin"])));
        for caller in ["admin", "manager", "hr_manager", "payroll_admin"] {
            assert!(!may_revoke_login(&r(&[caller]), &r(&["super_admin"])));
        }
    }

    #[test]
    fn only_admin_or_super_admin_may_revoke_an_admin() {
        assert!(may_revoke_login(&r(&["admin"]), &r(&["admin", "finance"])));
        assert!(may_revoke_login(&r(&["super_admin"]), &r(&["admin"])));
        for caller in ["manager", "hr_manager", "payroll_admin"] {
            assert!(!may_revoke_login(&r(&[caller]), &r(&["admin"])));
        }
    }

    #[test]
    fn other_roles_are_revocable_by_any_employee_manager() {
        assert!(may_revoke_login(&r(&["hr_manager"]), &r(&["employee"])));
        assert!(may_revoke_login(&r(&["manager"]), &r(&["finance"])));
    }
}
