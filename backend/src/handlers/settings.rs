use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::setting::{BulkUpdateSettingsRequest, CompanySetting, UpdateSettingRequest};
use crate::services::settings_service;

#[derive(Debug, Deserialize)]
pub struct SettingsQuery {
    pub category: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<SettingsQuery>,
) -> AppResult<Json<Vec<CompanySetting>>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let mut settings =
        settings_service::get_all_settings(&state.pool, company_id, query.category.as_deref())
            .await?;

    if auth.is_exec() {
        settings.retain(|s| s.category != "payroll" && s.category != "statutory");
    }

    Ok(Json(settings))
}

pub async fn get(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((category, key)): Path<(String, String)>,
) -> AppResult<Json<CompanySetting>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let setting = settings_service::get_setting(&state.pool, company_id, &category, &key).await?;
    Ok(Json(setting))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((category, key)): Path<(String, String)>,
    Json(req): Json<UpdateSettingRequest>,
) -> AppResult<Json<CompanySetting>> {
    if auth.is_exec() && (category == "payroll" || category == "statutory") {
        return Err(AppError::Forbidden(
            "Payroll settings not available for this role".into(),
        ));
    }
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let setting = settings_service::update_setting(
        &state.pool,
        company_id,
        &category,
        &key,
        req.value,
        auth.0.sub,
    )
    .await?;
    Ok(Json(setting))
}

pub async fn bulk_update(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<BulkUpdateSettingsRequest>,
) -> AppResult<Json<Vec<CompanySetting>>> {
    if auth.is_exec()
        && req
            .settings
            .iter()
            .any(|s| s.category == "payroll" || s.category == "statutory")
    {
        return Err(AppError::Forbidden(
            "Payroll settings not available for this role".into(),
        ));
    }
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let settings =
        settings_service::bulk_update_settings(&state.pool, company_id, req.settings, auth.0.sub)
            .await?;
    Ok(Json(settings))
}
