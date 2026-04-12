use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::portal::{Claim, LeaveRequest, OvertimeApplication};
use crate::services::approval_service::{self, ClaimWithEmployee, LeaveRequestWithEmployee, OvertimeWithEmployee};

fn require_admin(auth: &AuthUser) -> AppResult<Uuid> {
    match auth.0.role.as_str() {
        "super_admin" | "admin" | "payroll_admin" | "hr_manager" => Ok(auth
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

#[derive(Debug, Deserialize)]
pub struct ReviewRequest {
    pub notes: Option<String>,
}

pub async fn approve_leave(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<ReviewRequest>,
) -> AppResult<Json<LeaveRequest>> {
    let company_id = require_admin(&auth)?;
    let lr = approval_service::approve_leave(
        &state.pool,
        &state.config,
        company_id,
        id,
        auth.0.sub,
        req.notes.as_deref(),
    )
    .await?;
    Ok(Json(lr))
}

pub async fn reject_leave(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<ReviewRequest>,
) -> AppResult<Json<LeaveRequest>> {
    let company_id = require_admin(&auth)?;
    let lr = approval_service::reject_leave(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        req.notes.as_deref(),
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

pub async fn approve_claim(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<ReviewRequest>,
) -> AppResult<Json<Claim>> {
    let company_id = require_admin(&auth)?;
    let claim = approval_service::approve_claim(
        &state.pool,
        &state.config,
        company_id,
        id,
        auth.0.sub,
        req.notes.as_deref(),
    )
    .await?;
    Ok(Json(claim))
}

pub async fn reject_claim(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<ReviewRequest>,
) -> AppResult<Json<Claim>> {
    let company_id = require_admin(&auth)?;
    let claim = approval_service::reject_claim(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        req.notes.as_deref(),
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
    let apps =
        approval_service::get_pending_overtime(&state.pool, company_id, q.status.as_deref())
            .await?;
    Ok(Json(apps))
}

pub async fn approve_overtime(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<ReviewRequest>,
) -> AppResult<Json<OvertimeApplication>> {
    let company_id = require_admin(&auth)?;
    let ot = approval_service::approve_overtime(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        req.notes.as_deref(),
    )
    .await?;
    Ok(Json(ot))
}

pub async fn reject_overtime(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<ReviewRequest>,
) -> AppResult<Json<OvertimeApplication>> {
    let company_id = require_admin(&auth)?;
    let ot = approval_service::reject_overtime(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        req.notes.as_deref(),
    )
    .await?;
    Ok(Json(ot))
}
