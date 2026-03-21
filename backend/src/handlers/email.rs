use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::email::{
    is_valid_letter_type, CreateEmailTemplateRequest, EmailLog, EmailTemplate,
    PreviewLetterRequest, PreviewLetterResponse, SendLetterRequest, UpdateEmailTemplateRequest,
};
use crate::services::{company_service, email_service, employee_service};

use super::employee::PaginatedResponse;

// ── Templates ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TemplateQuery {
    pub letter_type: Option<String>,
}

pub async fn list_templates(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<TemplateQuery>,
) -> AppResult<Json<Vec<EmailTemplate>>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let templates =
        email_service::list_templates(&state.pool, company_id, query.letter_type.as_deref())
            .await?;
    Ok(Json(templates))
}

pub async fn get_template(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<EmailTemplate>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let template = email_service::get_template(&state.pool, id, company_id).await?;
    Ok(Json(template))
}

pub async fn create_template(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateEmailTemplateRequest>,
) -> AppResult<Json<EmailTemplate>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    if !is_valid_letter_type(&req.letter_type) {
        return Err(AppError::Validation(format!(
            "Invalid letter type: {}",
            req.letter_type
        )));
    }

    let template =
        email_service::create_template(&state.pool, company_id, req, auth.0.sub).await?;
    Ok(Json(template))
}

pub async fn update_template(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateEmailTemplateRequest>,
) -> AppResult<Json<EmailTemplate>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let template =
        email_service::update_template(&state.pool, id, company_id, req, auth.0.sub).await?;
    Ok(Json(template))
}

pub async fn delete_template(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    email_service::delete_template(&state.pool, id, company_id).await?;
    Ok(Json(serde_json::json!({"message": "Template deleted"})))
}

// ── Preview & Send ─────────────────────────────────────────────────────

pub async fn preview_letter(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<PreviewLetterRequest>,
) -> AppResult<Json<PreviewLetterResponse>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let employee = employee_service::get_employee(&state.pool, req.employee_id, company_id).await?;
    let company = company_service::get_company(&state.pool, company_id).await?;

    let subject = email_service::substitute_variables(
        &req.subject,
        &employee.full_name,
        &employee.employee_number,
        employee.email.as_deref().unwrap_or(""),
        employee.designation.as_deref().unwrap_or(""),
        employee.department.as_deref().unwrap_or(""),
        &employee.date_joined.to_string(),
        &company.name,
    );

    let body_html = email_service::substitute_variables(
        &req.body_html,
        &employee.full_name,
        &employee.employee_number,
        employee.email.as_deref().unwrap_or(""),
        employee.designation.as_deref().unwrap_or(""),
        employee.department.as_deref().unwrap_or(""),
        &employee.date_joined.to_string(),
        &company.name,
    );

    Ok(Json(PreviewLetterResponse {
        subject,
        body_html,
        recipient_email: employee.email.unwrap_or_default(),
        recipient_name: employee.full_name,
    }))
}

pub async fn send_letter(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<SendLetterRequest>,
) -> AppResult<Json<EmailLog>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    if !is_valid_letter_type(&req.letter_type) {
        return Err(AppError::Validation(format!(
            "Invalid letter type: {}",
            req.letter_type
        )));
    }

    let employee = employee_service::get_employee(&state.pool, req.employee_id, company_id).await?;
    let company = company_service::get_company(&state.pool, company_id).await?;

    let recipient_email = employee
        .email
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Employee has no email address".into()))?;

    // Substitute variables in subject and body
    let subject = email_service::substitute_variables(
        &req.subject,
        &employee.full_name,
        &employee.employee_number,
        recipient_email,
        employee.designation.as_deref().unwrap_or(""),
        employee.department.as_deref().unwrap_or(""),
        &employee.date_joined.to_string(),
        &company.name,
    );

    let body_html = email_service::substitute_variables(
        &req.body_html,
        &employee.full_name,
        &employee.employee_number,
        recipient_email,
        employee.designation.as_deref().unwrap_or(""),
        employee.department.as_deref().unwrap_or(""),
        &employee.date_joined.to_string(),
        &company.name,
    );

    let log = email_service::send_email(
        &state.config,
        &state.pool,
        company_id,
        Some(req.employee_id),
        req.template_id,
        &req.letter_type,
        recipient_email,
        &employee.full_name,
        &subject,
        &body_html,
        auth.0.sub,
    )
    .await?;

    Ok(Json(log))
}

// ── Email Logs ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct EmailLogQuery {
    pub employee_id: Option<Uuid>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub async fn list_email_logs(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<EmailLogQuery>,
) -> AppResult<Json<PaginatedResponse<EmailLog>>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let (logs, total) =
        email_service::list_email_logs(&state.pool, company_id, query.employee_id, per_page, offset)
            .await?;

    Ok(Json(PaginatedResponse {
        data: logs,
        total,
        page,
        per_page,
    }))
}
