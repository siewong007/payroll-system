use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::employee::{
    CreateEmployeeRequest, CreateTp3Request, Employee, SalaryHistory, Tp3Record,
    UpdateEmployeeRequest,
};
use crate::services::{company_service, email_service, employee_service};

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub search: Option<String>,
    pub department: Option<String>,
    pub is_active: Option<bool>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct PaginatedResponse<T: serde::Serialize> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<PaginatedResponse<Employee>>> {
    let company_id = auth.0.company_id
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

    if auth.is_exec() {
        for emp in &mut employees {
            emp.basic_salary = 0;
            emp.hourly_rate = None;
            emp.daily_rate = None;
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
    let company_id = auth.0.company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let mut emp = employee_service::get_employee(&state.pool, id, company_id).await?;
    if auth.is_exec() {
        emp.basic_salary = 0;
        emp.hourly_rate = None;
        emp.daily_rate = None;
    }
    Ok(Json(emp))
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateEmployeeRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.0.company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let (emp, account_info) = employee_service::create_employee(&state.pool, company_id, req, auth.0.sub).await?;

    // Auto-send welcome email if a new user account was created
    if let Some(ref info) = account_info {
        if info.created {
            if let Some(ref email_addr) = emp.email {
                let company = company_service::get_company(&state.pool, company_id).await?;
                let default_pw = info.default_password.as_deref().unwrap_or("(your IC number)");
                let body_html = email_service::default_welcome_html(
                    &emp.full_name,
                    &company.name,
                    &state.config.frontend_url,
                    email_addr,
                    default_pw,
                );
                let subject = format!("Welcome to {} - PayrollMY", company.name);

                let config = state.config.clone();
                let pool = state.pool.clone();
                let emp_id = emp.id;
                let emp_name = emp.full_name.clone();
                let email = email_addr.clone();
                let user_id = auth.0.sub;
                tokio::spawn(async move {
                    if let Err(e) = email_service::send_email(
                        &config,
                        &pool,
                        company_id,
                        Some(emp_id),
                        None,
                        "welcome",
                        &email,
                        &emp_name,
                        &subject,
                        &body_html,
                        user_id,
                    )
                    .await
                    {
                        tracing::error!("Failed to send welcome email for employee {}: {}", emp_id, e);
                    }
                });
            }
        }
    }

    Ok(Json(serde_json::json!({
        "employee": emp,
        "account": account_info,
    })))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateEmployeeRequest>,
) -> AppResult<Json<Employee>> {
    let company_id = auth.0.company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let emp = employee_service::update_employee(&state.pool, id, company_id, req, auth.0.sub).await?;
    Ok(Json(emp))
}

pub async fn delete(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.0.company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    employee_service::soft_delete_employee(&state.pool, id, company_id).await?;
    Ok(Json(serde_json::json!({"message": "Employee deleted"})))
}

pub async fn salary_history(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<SalaryHistory>>> {
    auth.deny_exec()?;
    let history = employee_service::get_salary_history(&state.pool, id).await?;
    Ok(Json(history))
}

pub async fn create_tp3(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateTp3Request>,
) -> AppResult<Json<Tp3Record>> {
    auth.deny_exec()?;
    let record = employee_service::create_tp3(&state.pool, id, req, auth.0.sub).await?;
    Ok(Json(record))
}
