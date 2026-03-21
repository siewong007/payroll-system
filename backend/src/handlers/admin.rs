use axum::{
    extract::{Path, State},
    Json,
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::company::{Company, CreateCompanyRequest, UpdateCompanyRequest};
use crate::models::session::PasswordResetWithUser;
use crate::models::user_company::{CreateUserRequest, UpdateUserCompaniesRequest, UpdateUserRequest, UserWithCompanies};
use crate::services::{company_service, password_reset_service, user_service};

fn require_super_admin(auth: &AuthUser) -> AppResult<()> {
    if auth.0.role != "super_admin" {
        return Err(AppError::Forbidden("Super admin access required".into()));
    }
    Ok(())
}

// ─── Companies ───

pub async fn list_companies(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<Company>>> {
    require_super_admin(&auth)?;
    let companies = company_service::list_companies(&state.pool).await?;
    Ok(Json(companies))
}

pub async fn create_company(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateCompanyRequest>,
) -> AppResult<Json<Company>> {
    require_super_admin(&auth)?;
    let company = company_service::create_company(&state.pool, req, auth.0.sub).await?;
    Ok(Json(company))
}

pub async fn update_company(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(company_id): Path<Uuid>,
    Json(req): Json<UpdateCompanyRequest>,
) -> AppResult<Json<Company>> {
    require_super_admin(&auth)?;
    let company = company_service::update_company(&state.pool, company_id, req, auth.0.sub).await?;
    Ok(Json(company))
}

// ─── Users ───

pub async fn list_users(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<UserWithCompanies>>> {
    let users = user_service::list_users(&state.pool, &auth.0.role, auth.0.sub).await?;
    Ok(Json(users))
}

pub async fn create_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateUserRequest>,
) -> AppResult<Json<UserWithCompanies>> {
    require_super_admin(&auth)?;
    let user = user_service::create_user(&state.pool, req).await?;
    Ok(Json(user))
}

pub async fn update_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(user_id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> AppResult<Json<UserWithCompanies>> {
    require_super_admin(&auth)?;
    let user = user_service::update_user(&state.pool, user_id, req).await?;
    Ok(Json(user))
}

pub async fn delete_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(user_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_super_admin(&auth)?;
    user_service::delete_user(&state.pool, user_id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn update_user_companies(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(user_id): Path<Uuid>,
    Json(req): Json<UpdateUserCompaniesRequest>,
) -> AppResult<Json<UserWithCompanies>> {
    require_super_admin(&auth)?;
    let user = user_service::update_user_companies(&state.pool, user_id, req.company_ids).await?;
    Ok(Json(user))
}

// ─── Password Reset Admin ───

pub async fn list_password_resets(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<PasswordResetWithUser>>> {
    require_super_admin(&auth)?;
    let requests = password_reset_service::list_requests(&state.pool).await?;
    Ok(Json(requests))
}

pub async fn approve_password_reset(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(request_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_super_admin(&auth)?;
    let (request, raw_token) =
        password_reset_service::approve_request(&state.pool, request_id, auth.0.sub).await?;
    Ok(Json(serde_json::json!({
        "request": request,
        "reset_token": raw_token,
        "message": "Reset approved. Share the reset token with the user securely."
    })))
}

pub async fn reject_password_reset(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(request_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_super_admin(&auth)?;
    password_reset_service::reject_request(&state.pool, request_id, auth.0.sub).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn count_pending_resets(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    require_super_admin(&auth)?;
    let count = password_reset_service::count_pending(&state.pool).await?;
    Ok(Json(serde_json::json!({ "count": count })))
}
