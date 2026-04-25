use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::portal::{
    Claim, CreateClaimRequest, CreateLeaveRequest, CreateOvertimeRequest, LeaveRequest,
    OvertimeApplication, UpdateClaimRequest, UpdateLeaveRequest, UpdateOvertimeRequest,
};
use crate::services::approval_service::{
    self, ClaimWithEmployee, LeaveRequestWithEmployee, OvertimeWithEmployee,
};
use crate::services::audit_service::AuditRequestMeta;

fn require_admin(auth: &AuthUser) -> AppResult<Uuid> {
    match auth.0.role.as_str() {
        "super_admin" | "admin" | "payroll_admin" | "hr_manager" | "exec" => Ok(auth
            .0
            .company_id
            .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?),
        _ => Err(AppError::Forbidden("Admin access required".into())),
    }
}

// ─── Leave ───

#[derive(Debug, Deserialize)]
pub struct StatusQuery {
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AdminLeaveRequest {
    pub employee_id: Uuid,
    pub leave_type_id: Uuid,
    pub start_date: chrono::NaiveDate,
    pub end_date: chrono::NaiveDate,
    pub days: rust_decimal::Decimal,
    pub reason: Option<String>,
    pub attachment_url: Option<String>,
    pub attachment_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AdminClaimRequest {
    pub employee_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub amount: i64,
    pub category: Option<String>,
    pub receipt_url: Option<String>,
    pub receipt_file_name: Option<String>,
    pub expense_date: chrono::NaiveDate,
}

#[derive(Debug, Deserialize)]
pub struct AdminOvertimeRequest {
    pub employee_id: Uuid,
    pub ot_date: chrono::NaiveDate,
    pub start_time: String,
    pub end_time: String,
    pub hours: rust_decimal::Decimal,
    pub ot_type: Option<String>,
    pub reason: Option<String>,
}

pub async fn list_leave_requests(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<StatusQuery>,
) -> AppResult<Json<Vec<LeaveRequestWithEmployee>>> {
    let company_id = require_admin(&auth)?;
    let requests =
        approval_service::get_pending_leave_requests(&state.pool, company_id, q.status.as_deref())
            .await?;
    Ok(Json(requests))
}

pub async fn create_leave_request(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(req): Json<AdminLeaveRequest>,
) -> AppResult<Json<LeaveRequest>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let leave = approval_service::create_leave_request_admin(
        &state.pool,
        company_id,
        req.employee_id,
        CreateLeaveRequest {
            leave_type_id: req.leave_type_id,
            start_date: req.start_date,
            end_date: req.end_date,
            days: req.days,
            reason: req.reason,
            attachment_url: req.attachment_url,
            attachment_name: req.attachment_name,
        },
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(leave))
}

pub async fn update_leave_request(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateLeaveRequest>,
) -> AppResult<Json<LeaveRequest>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let leave = approval_service::update_leave_request_admin(
        &state.pool,
        company_id,
        id,
        req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(leave))
}

pub async fn delete_leave_request(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    approval_service::delete_leave_request_admin(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn cancel_leave_request(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> AppResult<Json<LeaveRequest>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let leave = approval_service::cancel_leave_request_admin(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(leave))
}

#[derive(Debug, Deserialize)]
pub struct ReviewRequest {
    pub notes: Option<String>,
}

pub async fn approve_leave(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ReviewRequest>,
) -> AppResult<Json<LeaveRequest>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let lr = approval_service::approve_leave(
        &state.pool,
        &state.config,
        company_id,
        id,
        auth.0.sub,
        req.notes.as_deref(),
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(lr))
}

pub async fn reject_leave(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ReviewRequest>,
) -> AppResult<Json<LeaveRequest>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let lr = approval_service::reject_leave(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        req.notes.as_deref(),
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(lr))
}

// ─── Claims ───

pub async fn list_claims(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<StatusQuery>,
) -> AppResult<Json<Vec<ClaimWithEmployee>>> {
    let company_id = require_admin(&auth)?;
    let claims =
        approval_service::get_pending_claims(&state.pool, company_id, q.status.as_deref()).await?;
    Ok(Json(claims))
}

pub async fn create_claim(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(req): Json<AdminClaimRequest>,
) -> AppResult<Json<Claim>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let claim = approval_service::create_claim_admin(
        &state.pool,
        company_id,
        req.employee_id,
        CreateClaimRequest {
            title: req.title,
            description: req.description,
            amount: req.amount,
            category: req.category,
            receipt_url: req.receipt_url,
            receipt_file_name: req.receipt_file_name,
            expense_date: req.expense_date,
        },
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(claim))
}

pub async fn update_claim(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateClaimRequest>,
) -> AppResult<Json<ClaimWithEmployee>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let claim = approval_service::update_claim_admin(
        &state.pool,
        company_id,
        id,
        req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    let enriched =
        approval_service::get_claim_with_employee_by_id(&state.pool, company_id, claim.id).await?;
    Ok(Json(enriched))
}

pub async fn delete_claim(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    approval_service::delete_claim_admin(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn cancel_claim(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Claim>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let claim = approval_service::cancel_claim_admin(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(claim))
}

pub async fn approve_claim(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ReviewRequest>,
) -> AppResult<Json<Claim>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let claim = approval_service::approve_claim(
        &state.pool,
        &state.config,
        company_id,
        id,
        auth.0.sub,
        req.notes.as_deref(),
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(claim))
}

pub async fn reject_claim(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ReviewRequest>,
) -> AppResult<Json<Claim>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let claim = approval_service::reject_claim(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        req.notes.as_deref(),
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(claim))
}

// ─── Overtime ───

pub async fn list_overtime(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<StatusQuery>,
) -> AppResult<Json<Vec<OvertimeWithEmployee>>> {
    let company_id = require_admin(&auth)?;
    let apps = approval_service::get_pending_overtime(&state.pool, company_id, q.status.as_deref())
        .await?;
    Ok(Json(apps))
}

pub async fn create_overtime(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(req): Json<AdminOvertimeRequest>,
) -> AppResult<Json<OvertimeApplication>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let overtime = approval_service::create_overtime_admin(
        &state.pool,
        company_id,
        req.employee_id,
        CreateOvertimeRequest {
            ot_date: req.ot_date,
            start_time: req.start_time,
            end_time: req.end_time,
            hours: req.hours,
            ot_type: req.ot_type,
            reason: req.reason,
        },
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(overtime))
}

pub async fn update_overtime(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateOvertimeRequest>,
) -> AppResult<Json<OvertimeWithEmployee>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let overtime = approval_service::update_overtime_admin(
        &state.pool,
        company_id,
        id,
        req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    let enriched =
        approval_service::get_overtime_with_employee_by_id(&state.pool, company_id, overtime.id)
            .await?;
    Ok(Json(enriched))
}

pub async fn delete_overtime(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    approval_service::delete_overtime_admin(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn cancel_overtime(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> AppResult<Json<OvertimeApplication>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let ot = approval_service::cancel_overtime_admin(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(ot))
}

pub async fn approve_overtime(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ReviewRequest>,
) -> AppResult<Json<OvertimeApplication>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let ot = approval_service::approve_overtime(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        req.notes.as_deref(),
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(ot))
}

pub async fn reject_overtime(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ReviewRequest>,
) -> AppResult<Json<OvertimeApplication>> {
    let company_id = require_admin(&auth)?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let ot = approval_service::reject_overtime(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        req.notes.as_deref(),
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(ot))
}
