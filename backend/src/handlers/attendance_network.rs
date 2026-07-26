//! Administration of the attendance network allow-list.
//!
//! Every route here is gated on `ManageAttendanceNetworks`. That is an
//! allow-list gate in the sense CLAUDE.md asks for: a role added later gains
//! nothing until it is explicitly granted the permission.

use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, Permission};
use crate::core::error::AppResult;
use crate::core::extract::ClientIp;
use crate::models::company_network::{
    ApproveCandidateRequest, CompanyNetwork, CreateNetworkRequest, DismissCandidateRequest,
    NetworkCheckResult, ScoredCandidate, SetNetworkModeRequest, UpdateNetworkRequest,
};
use crate::services::{attendance_network_service, audit_service::AuditRequestMeta};

// ─── Mode ───

pub async fn get_mode(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendanceNetworks)?;

    let mode = attendance_network_service::get_mode(&state.pool, company_id).await?;
    Ok(Json(serde_json::json!({ "mode": mode })))
}

pub async fn set_mode(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Json(req): Json<SetNetworkModeRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendanceNetworks)?;

    attendance_network_service::set_mode(
        &state.pool,
        company_id,
        &req.mode,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

// ─── Allow-list ───

pub async fn list_networks(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<CompanyNetwork>>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendanceNetworks)?;

    let networks = attendance_network_service::list_networks(&state.pool, company_id).await?;
    Ok(Json(networks))
}

pub async fn create_network(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Json(req): Json<CreateNetworkRequest>,
) -> AppResult<Json<CompanyNetwork>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendanceNetworks)?;

    let network = attendance_network_service::create_network(
        &state.pool,
        company_id,
        &req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(network))
}

pub async fn update_network(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateNetworkRequest>,
) -> AppResult<Json<CompanyNetwork>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendanceNetworks)?;

    let network = attendance_network_service::update_network(
        &state.pool,
        company_id,
        id,
        &req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(network))
}

pub async fn delete_network(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendanceNetworks)?;

    attendance_network_service::delete_network(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ─── Learned candidates ───

pub async fn list_candidates(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<ScoredCandidate>>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendanceNetworks)?;

    let candidates = attendance_network_service::list_candidates(&state.pool, company_id).await?;
    Ok(Json(candidates))
}

pub async fn approve_candidate(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Json(req): Json<ApproveCandidateRequest>,
) -> AppResult<Json<CompanyNetwork>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendanceNetworks)?;

    let network = attendance_network_service::approve_candidate(
        &state.pool,
        company_id,
        &req.cidr,
        &req.label,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(network))
}

pub async fn dismiss_candidate(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Json(req): Json<DismissCandidateRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendanceNetworks)?;

    attendance_network_service::dismiss_candidate(
        &state.pool,
        company_id,
        &req.cidr,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ─── Diagnostics ───

/// What the server sees *this* request coming from, and whether it matches.
///
/// The answer to "I'm sitting in the office, why is it saying I'm not on the
/// network?" — an administrator on the office WiFi opens this and reads the
/// address to approve. Deliberately returns the caller's own resolved address
/// and nothing about anyone else, so it discloses no employee's location.
pub async fn whoami(
    State(state): State<AppState>,
    auth: AuthUser,
    ClientIp(client_ip): ClientIp,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendanceNetworks)?;

    let result: NetworkCheckResult =
        attendance_network_service::check_network(&state.pool, company_id, client_ip).await?;

    // The suggested block is what the learner would record, so an admin
    // approving by hand gets the same shape the candidate list would offer.
    let suggested = client_ip
        .and_then(|addr| attendance_network_service::candidate_prefix(addr).ok())
        .map(|prefix| prefix.to_string());

    Ok(Json(serde_json::json!({
        "client_ip": client_ip.map(|ip| ip.to_string()),
        "suggested_cidr": suggested,
        "is_approved": result.is_approved,
        "matched_label": result.matched_label,
        "has_approved_networks": result.has_approved_networks,
        // Surfacing this makes a misconfigured deployment self-diagnosing: if
        // the address above is a private one, the API is being reached without
        // the proxy that is supposed to be in front of it.
        "trust_proxy_headers": state.config.trust_proxy_headers,
    })))
}
