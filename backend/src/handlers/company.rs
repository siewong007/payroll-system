use axum::{Json, extract::State};

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::company::{Company, UpdateCompanyRequest};
use crate::services::company_service;

fn request_touches_payroll_fields(req: &UpdateCompanyRequest) -> bool {
    req.epf_number.is_some()
        || req.socso_code.is_some()
        || req.eis_code.is_some()
        || req.hrdf_number.is_some()
        || req.hrdf_enabled.is_some()
        || req.unpaid_leave_divisor.is_some()
}

fn redact_payroll_fields(company: &mut Company) {
    company.epf_number = None;
    company.socso_code = None;
    company.eis_code = None;
    company.hrdf_number = None;
    company.hrdf_enabled = None;
    company.unpaid_leave_divisor = None;
}

pub async fn get(State(state): State<AppState>, auth: AuthUser) -> AppResult<Json<Company>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let mut company = company_service::get_company(&state.pool, company_id).await?;
    if !auth.is_payroll_privileged() {
        redact_payroll_fields(&mut company);
    }
    Ok(Json(company))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<UpdateCompanyRequest>,
) -> AppResult<Json<Company>> {
    if request_touches_payroll_fields(&req) && !auth.is_payroll_privileged() {
        return Err(AppError::Forbidden(
            "Payroll settings not available for this role".into(),
        ));
    }
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let mut company =
        company_service::update_company(&state.pool, company_id, req, auth.0.sub).await?;
    if !auth.is_payroll_privileged() {
        redact_payroll_fields(&mut company);
    }

    Ok(Json(company))
}

pub async fn stats(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<company_service::CompanyStats>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let stats = company_service::get_company_stats(&state.pool, company_id).await?;
    Ok(Json(stats))
}
