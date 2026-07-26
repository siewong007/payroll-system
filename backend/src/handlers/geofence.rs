use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, Permission};
use crate::core::error::AppResult;
use crate::models::company_location::{
    CompanyLocation, CreateLocationRequest, SetGeofenceModeRequest, UpdateLocationRequest,
};
use crate::services::{audit_service::AuditRequestMeta, geofence_service};

/// GET /geofence/locations
pub async fn list_locations(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<CompanyLocation>>> {
    let company_id = auth.company_id()?;

    let locs = geofence_service::list_locations(&state.pool, company_id).await?;
    Ok(Json(locs))
}

/// POST /geofence/locations
pub async fn create_location(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Json(req): Json<CreateLocationRequest>,
) -> AppResult<Json<CompanyLocation>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageGeofence)?;

    let loc = geofence_service::create_location(
        &state.pool,
        company_id,
        &req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(loc))
}

/// PUT /geofence/locations/:id
pub async fn update_location(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateLocationRequest>,
) -> AppResult<Json<CompanyLocation>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageGeofence)?;

    let loc = geofence_service::update_location(
        &state.pool,
        company_id,
        id,
        &req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(loc))
}

/// DELETE /geofence/locations/:id
pub async fn delete_location(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageGeofence)?;

    geofence_service::delete_location(&state.pool, company_id, id, auth.0.sub, Some(&audit_meta))
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /geofence/mode
pub async fn get_mode(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;

    let mode = geofence_service::get_geofence_mode(&state.pool, company_id).await?;
    Ok(Json(serde_json::json!({ "mode": mode })))
}

/// PUT /geofence/mode
pub async fn set_mode(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Json(req): Json<SetGeofenceModeRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageGeofence)?;

    geofence_service::set_geofence_mode(
        &state.pool,
        company_id,
        &req.mode,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
