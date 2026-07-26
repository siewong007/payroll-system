use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, Permission};
use crate::core::error::AppResult;
use crate::core::extract::ValidatedJson;
use crate::models::user_group::{
    AddUserGroupMemberRequest, CreateUserGroupRequest, UpdateUserGroupRequest, UserGroup,
    UserGroupMember, UserGroupWithDetail,
};
use crate::services::{audit_service::AuditRequestMeta, user_group_service};

// Managing a group means handing out capabilities, so it is gated on the same
// permission as managing users rather than a weaker one — a group is a way to
// grant access, and anyone who can create one can grant themselves anything.

pub async fn list_groups(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<UserGroupWithDetail>>> {
    let company_id = auth.authorize(Permission::ViewUserDirectory)?;
    Ok(Json(
        user_group_service::list_groups(&state.pool, company_id).await?,
    ))
}

pub async fn get_group(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserGroup>> {
    let company_id = auth.authorize(Permission::ViewUserDirectory)?;
    Ok(Json(
        user_group_service::get_group(&state.pool, company_id, id).await?,
    ))
}

pub async fn create_group(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    ValidatedJson(req): ValidatedJson<CreateUserGroupRequest>,
) -> AppResult<Json<UserGroup>> {
    let (user_id, company_id) = auth.authorize_actor(Permission::ManageUsers)?;
    let group = user_group_service::create_group(
        &state.pool,
        company_id,
        &req.name,
        req.description.as_deref(),
        &req.permissions,
        user_id,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(group))
}

pub async fn update_group(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    audit_meta: AuditRequestMeta,
    ValidatedJson(req): ValidatedJson<UpdateUserGroupRequest>,
) -> AppResult<Json<UserGroup>> {
    let (user_id, company_id) = auth.authorize_actor(Permission::ManageUsers)?;
    let group = user_group_service::update_group(
        &state.pool,
        company_id,
        id,
        req.name.as_deref(),
        req.description.as_deref(),
        req.is_active,
        req.permissions.as_deref(),
        user_id,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(group))
}

pub async fn delete_group(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    audit_meta: AuditRequestMeta,
) -> AppResult<Json<serde_json::Value>> {
    let (user_id, company_id) = auth.authorize_actor(Permission::ManageUsers)?;
    user_group_service::delete_group(&state.pool, company_id, id, user_id, Some(&audit_meta))
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ─── Members ───

pub async fn list_members(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(group_id): Path<Uuid>,
) -> AppResult<Json<Vec<UserGroupMember>>> {
    let company_id = auth.authorize(Permission::ViewUserDirectory)?;
    Ok(Json(
        user_group_service::list_members(&state.pool, company_id, group_id).await?,
    ))
}

pub async fn add_member(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(group_id): Path<Uuid>,
    audit_meta: AuditRequestMeta,
    Json(req): Json<AddUserGroupMemberRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let (user_id, company_id) = auth.authorize_actor(Permission::ManageUsers)?;
    user_group_service::add_member(
        &state.pool,
        company_id,
        group_id,
        req.user_id,
        user_id,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn remove_member(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((group_id, member_id)): Path<(Uuid, Uuid)>,
    audit_meta: AuditRequestMeta,
) -> AppResult<Json<serde_json::Value>> {
    let (user_id, company_id) = auth.authorize_actor(Permission::ManageUsers)?;
    user_group_service::remove_member(
        &state.pool,
        company_id,
        group_id,
        member_id,
        user_id,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
