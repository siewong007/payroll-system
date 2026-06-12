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
    let current_year = chrono::Utc::now().year();
    let _ = crate::services::portal_service::initialize_leave_balances(
        pool,
        emp.id,
        company_id,
        emp.date_joined,
        current_year,
    )
    .await;

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
        if existing.roles.as_slice() == ["employee"] {
            // Stale employee account — clean up and recreate below
            user_companies::delete_by_user(pool, existing_id).await?;
            refresh_tokens::delete_by_user(pool, existing_id).await?;
            users::delete(pool, existing_id).await?;
        } else {
            // Non-employee user (admin, etc.) — link to this employee silently
            users::link_to_employee(pool, emp.id, emp.company_id, existing_id).await?;
            user_companies::insert(pool, existing_id, emp.company_id).await?;
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

    // Track salary change
    if let Some(new_salary) = req.basic_salary
        && new_salary != existing.basic_salary
    {
        let history_id = Uuid::now_v7();
        salary_history::insert(
            pool,
            history_id,
            id,
            existing.basic_salary,
            new_salary,
            updated_by,
        )
        .await?;
    }

    let emp = employees::update(pool, id, company_id, &req, updated_by).await?;

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

pub async fn soft_delete_employee(pool: &PgPool, id: Uuid, company_id: Uuid) -> AppResult<()> {
    let rows = employees::soft_delete(pool, id, company_id).await?;

    if rows == 0 {
        return Err(AppError::NotFound("Employee not found".into()));
    }

    // Delete the user account linked to this employee
    user_companies::delete_by_employee(pool, id).await?;
    refresh_tokens::delete_by_employee(pool, id).await?;
    users::delete_by_employee(pool, id).await?;

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
