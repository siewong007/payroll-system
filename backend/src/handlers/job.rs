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

/// Whether the caller may poll a job of `kind`, keyed by what its `result`
/// carries. Unlisted job types fail closed.
fn can_read_job(auth: &AuthUser, kind: &str) -> Result<bool, AppError> {
    if kind == job_type::PAYROLL_RUN {
        // result is `{run_id}` — accessible to anyone who can view or prepare payroll drafts.
        Ok(auth.can(Permission::ViewPayroll) || auth.can(Permission::ManagePayrollDraft))
    } else if kind == job_type::EMPLOYEE_IMPORT {
        // result embeds per-row import detail — the importer's permission.
        Ok(auth.can(Permission::ImportEmployees))
    } else {
        Err(AppError::Forbidden(
            "This job type has no status endpoint".into(),
        ))
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

    if !auth.can(Permission::ViewPayroll)
        && !auth.can(Permission::ManagePayrollDraft)
        && !auth.can(Permission::ImportEmployees)
    {
        return Err(AppError::Forbidden(
            "Not authorized to view background jobs".into(),
        ));
    }

    let job = job_service::get(&state.pool, company_id, id).await?;
    if !can_read_job(&auth, &job.job_type)? {
        return Err(AppError::NotFound("Job not found".into()));
    }

    Ok(Json(job))
}
