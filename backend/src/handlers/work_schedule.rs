use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::AppResult;
use crate::models::work_schedule::{
    CreateWorkScheduleRequest, UpdateWorkScheduleRequest, WorkSchedule,
};
use crate::services::{audit_service::AuditRequestMeta, work_schedule_service};

/// GET /work-schedules — list all schedules for the caller's company
pub async fn list_schedules(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<WorkSchedule>>> {
    let company_id = auth.company_id()?;

    let schedules = work_schedule_service::list_schedules(&state.pool, company_id).await?;
    Ok(Json(schedules))
}

/// GET /work-schedules/default — get the default schedule
pub async fn get_default_schedule(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;

    let schedule = work_schedule_service::get_default_schedule(&state.pool, company_id).await?;
    Ok(Json(serde_json::json!({ "schedule": schedule })))
}

/// PUT /work-schedules/default — create or update the default schedule
pub async fn upsert_default_schedule(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(req): Json<CreateWorkScheduleRequest>,
) -> AppResult<Json<WorkSchedule>> {
    let company_id = auth.company_id()?;
    auth.require_hr_admin()?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);

    let schedule = work_schedule_service::upsert_default_schedule(
        &state.pool,
        company_id,
        &req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(schedule))
}

/// PUT /work-schedules/:id — update a specific schedule
pub async fn update_schedule(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateWorkScheduleRequest>,
) -> AppResult<Json<WorkSchedule>> {
    let company_id = auth.company_id()?;
    auth.require_hr_admin()?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);

    let schedule = work_schedule_service::update_schedule(
        &state.pool,
        company_id,
        id,
        &req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(schedule))
}
