use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, Permission};
use crate::core::error::{AppError, AppResult};
use crate::models::background_job::{BackgroundJob, job_type};
use crate::services::job_service;

/// The read gate for a job's status, keyed by what its `result` carries —
/// the same permission the synchronous endpoint required, not a weaker one.
/// Unlisted job types get no read path at all (fail closed).
fn read_permission(kind: &str) -> Option<Permission> {
    if kind == job_type::PAYROLL_RUN {
        // result is just `{run_id}` — the run's own read gate.
        Some(Permission::ViewPayroll)
    } else if kind == job_type::EMPLOYEE_IMPORT {
        // result embeds per-row import detail — the importer's permission.
        Some(Permission::ImportEmployees)
    } else {
        None
    }
}

/// `GET /api/jobs/{id}` — poll a submitted background job.
pub async fn status(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<BackgroundJob>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let job = job_service::get(&state.pool, company_id, id).await?;
    let permission = read_permission(&job.job_type)
        .ok_or_else(|| AppError::Forbidden("This job type has no status endpoint".into()))?;
    auth.require_permission(permission)?;

    Ok(Json(job))
}
