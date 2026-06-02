use axum::{Json, extract::State};

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::services::dashboard_service::{self, DashboardSummary};

pub async fn summary(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<DashboardSummary>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let can_access_payroll = auth.is_payroll_privileged();

    Ok(Json(
        dashboard_service::summary(&state.pool, company_id, can_access_payroll).await?,
    ))
}
