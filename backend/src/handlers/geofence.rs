use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::company_location::{
    CompanyLocation, CreateLocationRequest, SetGeofenceModeRequest, UpdateLocationRequest,
};
use crate::services::geofence_service;

/// GET /geofence/locations
pub async fn list_locations(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<CompanyLocation>>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let locs = geofence_service::list_locations(&state.pool, company_id).await?;
    Ok(Json(locs))
}

/// POST /geofence/locations
pub async fn create_location(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateLocationRequest>,
) -> AppResult<Json<CompanyLocation>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    if !matches!(auth.0.role.as_str(), "admin" | "super_admin" | "hr_manager") {
        return Err(AppError::Forbidden("Admin role required".into()));
    }

    let loc = geofence_service::create_location(&state.pool, company_id, &req).await?;
    Ok(Json(loc))
}

/// PUT /geofence/locations/:id
pub async fn update_location(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateLocationRequest>,
) -> AppResult<Json<CompanyLocation>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    if !matches!(auth.0.role.as_str(), "admin" | "super_admin" | "hr_manager") {
        return Err(AppError::Forbidden("Admin role required".into()));
    }

    let loc = geofence_service::update_location(&state.pool, company_id, id, &req).await?;
    Ok(Json(loc))
}

/// DELETE /geofence/locations/:id
pub async fn delete_location(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    if !matches!(auth.0.role.as_str(), "admin" | "super_admin" | "hr_manager") {
        return Err(AppError::Forbidden("Admin role required".into()));
    }

    geofence_service::delete_location(&state.pool, company_id, id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /geofence/mode
pub async fn get_mode(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let mode = geofence_service::get_geofence_mode(&state.pool, company_id).await?;
    Ok(Json(serde_json::json!({ "mode": mode })))
}

/// PUT /geofence/mode
pub async fn set_mode(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<SetGeofenceModeRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    if !matches!(auth.0.role.as_str(), "admin" | "super_admin" | "hr_manager") {
        return Err(AppError::Forbidden("Admin role required".into()));
    }

    geofence_service::set_geofence_mode(&state.pool, company_id, &req.mode).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
