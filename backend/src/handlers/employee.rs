use axum::{
    Json,
    extract::{Path, Query, State},
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, Permission};
use crate::core::error::{AppError, AppResult};
use crate::models::employee::{
    CarryForwardRequest, CreateEmployeeRequest, CreateTp3Request, Employee, InitBalancesQuery,
    ListQuery, SalaryHistory, Tp3Record, UpdateEmployeeRequest,
};
use crate::models::pagination::PaginatedResponse;
use crate::services::audit_service::AuditRequestMeta;
use crate::services::{company_service, email_service, employee_service, portal_service};

fn redact_payroll_fields(employee: &mut Employee) {
    employee.basic_salary = 0;
    employee.hourly_rate = None;
    employee.daily_rate = None;
    employee.tax_identification_number = None;
    employee.epf_number = None;
    employee.socso_number = None;
    employee.eis_number = None;
    employee.working_spouse = None;
    employee.epf_category = None;
    employee.is_muslim = None;
    employee.zakat_eligible = None;
    employee.zakat_monthly_amount = None;
    employee.ptptn_monthly_amount = None;
    employee.tabung_haji_amount = None;
    employee.hrdf_contribution = None;
    employee.payroll_group_id = None;
    employee.salary_group = None;
}

/// Identity, contact and banking details. Separate from payroll figures because
/// HR roles legitimately need these while read-only roles (e.g. `exec`) do not,
/// and a leaked bank account is what lets salary payment be redirected.
fn redact_personal_fields(employee: &mut Employee) {
    employee.ic_number = None;
    employee.passport_number = None;
    employee.date_of_birth = None;
    employee.phone = None;
    employee.address_line1 = None;
    employee.address_line2 = None;
    employee.city = None;
    employee.state = None;
    employee.postcode = None;
    employee.bank_name = None;
    employee.bank_account_number = None;
    employee.bank_account_type = None;
}

fn create_request_touches_payroll_fields(req: &CreateEmployeeRequest) -> bool {
    req.basic_salary != 0
        || req.hourly_rate.is_some()
        || req.daily_rate.is_some()
        || req.tax_identification_number.is_some()
        || req.epf_number.is_some()
        || req.socso_number.is_some()
        || req.eis_number.is_some()
        || req.working_spouse.is_some()
        || req.epf_category.is_some()
        || req.is_muslim.is_some()
        || req.zakat_eligible.is_some()
        || req.zakat_monthly_amount.is_some()
        || req.ptptn_monthly_amount.is_some()
        || req.tabung_haji_amount.is_some()
        || req.payroll_group_id.is_some()
        || req.salary_group.is_some()
        // Banking decides where salary lands, so it is payroll-sensitive.
        || req.bank_name.is_some()
        || req.bank_account_number.is_some()
}

fn update_request_touches_payroll_fields(req: &UpdateEmployeeRequest) -> bool {
    req.basic_salary.is_some()
        || req.hourly_rate.is_some()
        || req.daily_rate.is_some()
        || req.tax_identification_number.is_some()
        || req.epf_number.is_some()
        || req.socso_number.is_some()
        || req.eis_number.is_some()
        || req.working_spouse.is_some()
        || req.epf_category.is_some()
        || req.is_muslim.is_some()
        || req.zakat_eligible.is_some()
        || req.zakat_monthly_amount.is_some()
        || req.ptptn_monthly_amount.is_some()
        || req.tabung_haji_amount.is_some()
        || req.hrdf_contribution.is_some()
        || req.payroll_group_id.is_some()
        || req.salary_group.is_some()
        // Banking decides where salary lands, so it is payroll-sensitive.
        || req.bank_name.is_some()
        || req.bank_account_number.is_some()
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<PaginatedResponse<Employee>>> {
    auth.require_permission(Permission::ViewEmployees)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let (mut employees, total) = employee_service::list_employees(
        &state.pool,
        company_id,
        query.search.as_deref(),
        query.department.as_deref(),
        query.is_active,
        per_page,
        offset,
    )
    .await?;

    let hide_payroll = !auth.is_payroll_privileged();
    let hide_personal = hide_payroll && !auth.can(Permission::ManageEmployees);
    for emp in &mut employees {
        if hide_payroll {
            redact_payroll_fields(emp);
        }
        if hide_personal {
            redact_personal_fields(emp);
        }
    }

    Ok(Json(PaginatedResponse {
        data: employees,
        total,
        page,
        per_page,
    }))
}

pub async fn get(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Employee>> {
    auth.require_permission(Permission::ViewEmployees)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let mut emp = employee_service::get_employee(&state.pool, id, company_id).await?;
    if !auth.is_payroll_privileged() {
        redact_payroll_fields(&mut emp);
        if !auth.can(Permission::ManageEmployees) {
            redact_personal_fields(&mut emp);
        }
    }
    Ok(Json(emp))
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Json(req): Json<CreateEmployeeRequest>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_permission(Permission::ManageEmployees)?;
    if !auth.is_payroll_privileged() && create_request_touches_payroll_fields(&req) {
        return Err(AppError::Forbidden(
            "Payroll fields are not available for this role".into(),
        ));
    }
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let (emp, account_info) = employee_service::create_employee(
        &state.pool,
        company_id,
        req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;

    // Auto-send welcome email if a new user account was created. Composing it
    // is `email_service`'s job: the letter carries the account's initial
    // password and the service is what decides that the stored copy must not.
    if let Some(ref info) = account_info
        && info.created
        && let Some(ref email_addr) = emp.email
    {
        let company = company_service::get_company(&state.pool, company_id).await?;

        let config = state.config.clone();
        let pool = state.pool.clone();
        let company_name = company.name.clone();
        let emp_id = emp.id;
        let emp_name = emp.full_name.clone();
        let email = email_addr.clone();
        let default_pw = info
            .default_password
            .clone()
            .unwrap_or_else(|| "(your IC number)".to_string());
        let user_id = auth.0.sub;
        tokio::spawn(async move {
            if let Err(e) = email_service::send_welcome_email(
                &config,
                &pool,
                company_id,
                &company_name,
                emp_id,
                &emp_name,
                &email,
                &default_pw,
                user_id,
            )
            .await
            {
                tracing::error!(
                    "Failed to send welcome email for employee {}: {}",
                    emp_id,
                    e
                );
            }
        });
    }

    Ok(Json(serde_json::json!({
        "employee": emp,
        "account": account_info,
    })))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateEmployeeRequest>,
) -> AppResult<Json<Employee>> {
    auth.require_permission(Permission::ManageEmployees)?;
    if !auth.is_payroll_privileged() && update_request_touches_payroll_fields(&req) {
        return Err(AppError::Forbidden(
            "Payroll fields are not available for this role".into(),
        ));
    }
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let emp = employee_service::update_employee(
        &state.pool,
        id,
        company_id,
        req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(emp))
}

pub async fn delete(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_permission(Permission::ManageEmployees)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    employee_service::soft_delete_employee(
        &state.pool,
        id,
        company_id,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(serde_json::json!({"message": "Employee deleted"})))
}

pub async fn salary_history(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<SalaryHistory>>> {
    auth.require_permission(Permission::ViewPayroll)?;
    let company_id = auth.company_id()?;
    // Tenant isolation: confirm the employee belongs to the caller's company
    // before exposing salary history. `get_employee` is company-scoped and
    // returns NotFound for cross-tenant ids.
    employee_service::get_employee(&state.pool, id, company_id).await?;
    let history = employee_service::get_salary_history(&state.pool, id).await?;
    Ok(Json(history))
}

pub async fn create_tp3(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateTp3Request>,
) -> AppResult<Json<Tp3Record>> {
    auth.require_permission(Permission::ManagePayrollDraft)?;
    let company_id = auth.company_id()?;
    // Tenant isolation: confirm the employee belongs to the caller's company
    // before writing a TP3 record. `get_employee` is company-scoped.
    employee_service::get_employee(&state.pool, id, company_id).await?;
    let record = employee_service::create_tp3(&state.pool, id, company_id, req, auth.0.sub).await?;
    Ok(Json(record))
}

pub async fn initialize_balances(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(q): Query<InitBalancesQuery>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_permission(Permission::ManageEmployees)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let emp = employee_service::get_employee(&state.pool, id, company_id).await?;
    let year = q.year.unwrap_or_else(|| chrono::Utc::now().year());
    let balances = portal_service::initialize_leave_balances(
        &state.pool,
        id,
        company_id,
        emp.date_joined,
        year,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "message": format!("Initialized {} leave balances", balances.len()),
        "count": balances.len(),
    })))
}

pub async fn process_carry_forward(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CarryForwardRequest>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_permission(Permission::ManageEmployees)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let count = portal_service::process_year_end_carry_forward(
        &state.pool,
        company_id,
        req.from_year,
        req.to_year,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "message": format!("Processed {} leave balance entries", count),
        "count": count,
    })))
}

use chrono::Datelike;
