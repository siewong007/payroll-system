use axum::{
    extract::State,
    Json,
};

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::company::{Company, UpdateCompanyRequest};
use crate::services::company_service;

pub async fn get(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Company>> {
    let company_id = auth.0.company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let company = company_service::get_company(&state.pool, company_id).await?;
    Ok(Json(company))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<UpdateCompanyRequest>,
) -> AppResult<Json<Company>> {
    let company_id = auth.0.company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let company = company_service::update_company(
        &state.pool,
        company_id,
        req,
        auth.0.sub,
    ).await?;

    Ok(Json(company))
}

pub async fn stats(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<company_service::CompanyStats>> {
    let company_id = auth.0.company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let stats = company_service::get_company_stats(&state.pool, company_id).await?;
    Ok(Json(stats))
}
