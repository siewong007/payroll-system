use axum::{
    Json,
    extract::{Path, Query, State},
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, Permission};
use crate::core::error::AppResult;
use crate::core::extract::ValidatedJson;
use crate::models::audit::AuditRequestMeta;
use crate::models::company::{Company, CreateCompanyRequest, UpdateCompanyRequest};
use crate::models::pagination::PaginatedResponse;
use crate::models::user_company::{CreateUserRequest, UpdateUserRequest, UserWithCompanies};
use crate::services::{company_service, user_service};

#[derive(Debug, serde::Deserialize)]
pub struct UserListQuery {
    pub company_id: Option<Uuid>,
    pub search: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

// ─── Companies ───

pub async fn list_companies(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<Company>>> {
    auth.require_permission(Permission::ManageCompanies)?;
    let companies = company_service::list_companies(&state.pool).await?;
    Ok(Json(companies))
}

pub async fn create_company(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateCompanyRequest>,
) -> AppResult<Json<Company>> {
    auth.require_permission(Permission::ManageCompanies)?;
    let company = company_service::create_company(&state.pool, req, auth.0.sub).await?;
    Ok(Json(company))
}

pub async fn update_company(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(company_id): Path<Uuid>,
    Json(req): Json<UpdateCompanyRequest>,
) -> AppResult<Json<Company>> {
    auth.require_permission(Permission::ManageCompanies)?;
    let company =
        company_service::update_company(&state.pool, company_id, req, auth.0.sub, None).await?;
    Ok(Json(company))
}

pub async fn delete_company(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(company_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_permission(Permission::ManageCompanies)?;
    company_service::delete_company(&state.pool, company_id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ─── Users ───

pub async fn list_users(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<UserListQuery>,
) -> AppResult<Json<PaginatedResponse<UserWithCompanies>>> {
    auth.require_permission(Permission::ViewUserDirectory)?;

    let is_super_admin = auth.has_any_role(&["super_admin"]);
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * per_page;

    // The company filter is honoured for both roles: a company admin's own
    // visibility is already restricted to companies they belong to, so the
    // filter narrows within that set rather than widening it.
    let (data, total) = user_service::list_users(
        &state.pool,
        is_super_admin,
        auth.0.sub,
        query.company_id,
        query.search.as_deref(),
        per_page,
        offset,
    )
    .await?;

    Ok(Json(PaginatedResponse {
        data,
        total,
        page,
        per_page,
    }))
}

pub async fn create_user(
    State(state): State<AppState>,
    auth: AuthUser,
    meta: AuditRequestMeta,
    ValidatedJson(req): ValidatedJson<CreateUserRequest>,
) -> AppResult<Json<UserWithCompanies>> {
    auth.require_permission(Permission::ManageUsers)?;
    let user = user_service::create_user(&state.pool, req, auth.0.sub, Some(&meta)).await?;
    Ok(Json(user))
}

pub async fn update_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(user_id): Path<Uuid>,
    meta: AuditRequestMeta,
    ValidatedJson(req): ValidatedJson<UpdateUserRequest>,
) -> AppResult<Json<UserWithCompanies>> {
    auth.require_permission(Permission::ManageUsers)?;
    let user =
        user_service::update_user(&state.pool, user_id, req, auth.0.sub, Some(&meta)).await?;
    Ok(Json(user))
}

pub async fn delete_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(user_id): Path<Uuid>,
    meta: AuditRequestMeta,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_permission(Permission::ManageUsers)?;
    user_service::delete_user(&state.pool, user_id, auth.0.sub, Some(&meta)).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
