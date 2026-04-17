use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::work_schedule::{
    CreateWorkScheduleRequest, UpdateWorkScheduleRequest, WorkSchedule,
};
use crate::services::work_schedule_service;

/// GET /work-schedules — list all schedules for the caller's company
pub async fn list_schedules(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<WorkSchedule>>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let schedules = work_schedule_service::list_schedules(&state.pool, company_id).await?;
    Ok(Json(schedules))
}

/// GET /work-schedules/default — get the default schedule
pub async fn get_default_schedule(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let schedule = work_schedule_service::get_default_schedule(&state.pool, company_id).await?;
    Ok(Json(serde_json::json!({ "schedule": schedule })))
}

/// PUT /work-schedules/default — create or update the default schedule
pub async fn upsert_default_schedule(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateWorkScheduleRequest>,
) -> AppResult<Json<WorkSchedule>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    if !matches!(auth.0.role.as_str(), "admin" | "super_admin" | "hr_manager") {
        return Err(AppError::Forbidden("Admin role required".into()));
    }

    let schedule =
        work_schedule_service::upsert_default_schedule(&state.pool, company_id, &req).await?;
    Ok(Json(schedule))
}

/// PUT /work-schedules/:id — update a specific schedule
pub async fn update_schedule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateWorkScheduleRequest>,
) -> AppResult<Json<WorkSchedule>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    if !matches!(auth.0.role.as_str(), "admin" | "super_admin" | "hr_manager") {
        return Err(AppError::Forbidden("Admin role required".into()));
    }

    let schedule =
        work_schedule_service::update_schedule(&state.pool, company_id, id, &req).await?;
    Ok(Json(schedule))
}
