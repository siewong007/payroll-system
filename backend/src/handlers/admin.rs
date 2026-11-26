use axum::{
    Json,
    extract::{Path, Query, State},
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, Permission};
use crate::core::config::{BUILD_REVISION, BUILD_VERSION};
use crate::core::error::{AppError, AppResult};
use crate::core::extract::ValidatedJson;
use crate::models::audit::AuditRequestMeta;
use crate::models::company::{Company, CreateCompanyRequest, UpdateCompanyRequest};
use crate::models::pagination::PaginatedResponse;
use crate::models::totp::{AdminTotpResetRequest, TwoFactorPolicyRequest, TwoFactorPolicyResponse};
use crate::models::user_company::{CreateUserRequest, UpdateUserRequest, UserWithCompanies};
use crate::repositories::reads::system as system_reads;
use crate::services::{
    auth_service, company_service, job_health_service, totp_service, user_service,
};

#[derive(Debug, serde::Deserialize)]
pub struct UserListQuery {
    pub company_id: Option<Uuid>,
    pub search: Option<String>,
    pub role: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

fn require_super_admin(auth: &AuthUser) -> AppResult<()> {
    if !auth.is_super_admin() {
        return Err(AppError::Forbidden("Super admin access required".into()));
    }
    Ok(())
}

// ─── Companies ───

pub async fn list_companies(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<Company>>> {
    auth.require_permission(Permission::ManageCompanies)?;
    require_super_admin(&auth)?;
    let companies = company_service::list_companies(&state.pool).await?;
    Ok(Json(companies))
}

pub async fn create_company(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateCompanyRequest>,
) -> AppResult<Json<Company>> {
    auth.require_permission(Permission::ManageCompanies)?;
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
    auth.require_permission(Permission::ManageCompanies)?;
    require_super_admin(&auth)?;
    let access = crate::services::upload_service::access_from_auth(&auth, company_id);
    let company =
        company_service::update_company(&state.pool, company_id, req, auth.0.sub, &access, None)
            .await?;
    Ok(Json(company))
}

pub async fn delete_company(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(company_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_permission(Permission::ManageCompanies)?;
    require_super_admin(&auth)?;
    company_service::delete_company(&state.pool, company_id, auth.0.sub).await?;

    if auth.0.company_id == Some(company_id) {
        let user = auth_service::get_user_by_id(&state.pool, auth.0.sub).await?;
        if let Some(next_company_id) = user.company_id {
            let _ = crate::repositories::push_subscriptions::update_company_for_session(
                &state.pool,
                auth.0.sub,
                auth.0.sid,
                next_company_id,
            )
            .await;
        }
        let token = crate::core::auth::create_token_with_exp(
            user.id,
            &user.email,
            &user.roles,
            user.company_id,
            user.employee_id,
            auth.0.sid,
            &state.config.jwt_secret,
            auth.0.exp,
        )?;
        let user_response = auth_service::build_user_response(&state.pool, user).await?;
        return Ok(Json(serde_json::json!({
            "ok": true,
            "token": token,
            "user": user_response,
        })));
    }

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
    // When `ViewUserDirectory` comes only from an active-company user group (not
    // from the user's global role), scope the directory strictly to that active
    // company so group grants in Company A cannot inspect Company B's users.
    let company_filter = if !is_super_admin
        && !crate::core::permission::roles_grant(&auth.0.roles, Permission::ViewUserDirectory)
    {
        let active_company_id = auth.company_id()?;
        if let Some(requested) = query.company_id
            && requested != active_company_id
        {
            return Err(AppError::Forbidden(
                "Not authorized: directory access is scoped to your active company".into(),
            ));
        }
        Some(active_company_id)
    } else {
        query.company_id
    };

    let page = query.page.unwrap_or(1).clamp(1, 100_000);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1).saturating_mul(per_page);

    let (data, total) = user_service::list_users_with_role(
        &state.pool,
        is_super_admin,
        auth.0.sub,
        company_filter,
        query.search.as_deref(),
        query.role.as_deref(),
        user_service::hides_privileged_users(&auth),
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
    require_super_admin(&auth)?;
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
    require_super_admin(&auth)?;
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
    require_super_admin(&auth)?;
    user_service::delete_user(&state.pool, user_id, auth.0.sub, Some(&meta)).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ─── 2FA break-glass reset ───

/// Super_admin-only: strip a user's TOTP enrolment and backup codes and
/// revoke their sessions, so someone locked out of a lost authenticator (or
/// locked out by a pre-v2 key-management bug) can sign in again without
/// direct database access. The caller re-presents their own password.
pub async fn reset_user_2fa(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(user_id): Path<Uuid>,
    meta: AuditRequestMeta,
    ValidatedJson(req): ValidatedJson<AdminTotpResetRequest>,
) -> AppResult<Json<serde_json::Value>> {
    // Deliberately not `require_permission`: no permission expresses
    // "platform super_admin" here, and every role that holds ManageUsers
    // (company admins) must not be able to switch off their own users'
    // second factor — that would make phishing one company admin worth a
    // company-wide 2FA bypass.
    if !auth.has_any_role(&["super_admin"]) {
        return Err(AppError::Forbidden("super_admin only".into()));
    }

    totp_service::admin_reset(&state.pool, user_id, auth.0.sub, &req.password, Some(&meta)).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ─── Platform 2FA enforcement policy (plan item 36) ───

/// The policy plus the privileged accounts it would refuse, so an operator
/// can see exactly who a cutover locks out before flipping it on.
pub async fn get_2fa_policy(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<TwoFactorPolicyResponse>> {
    auth.require_permission(Permission::ManagePlatformSettings)?;
    require_super_admin(&auth)?;
    Ok(Json(
        auth_service::get_two_factor_policy(&state.pool).await?,
    ))
}

pub async fn set_2fa_policy(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    ValidatedJson(req): ValidatedJson<TwoFactorPolicyRequest>,
) -> AppResult<Json<TwoFactorPolicyResponse>> {
    auth.require_permission(Permission::ManagePlatformSettings)?;
    require_super_admin(&auth)?;
    let status =
        auth_service::set_two_factor_policy(&state.pool, req, auth.0.sub, Some(&audit_meta))
            .await?;
    Ok(Json(status))
}

// ─── Operational platform health (items 25 & 70) ───

pub async fn get_platform_health(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_permission(Permission::ManagePlatformSettings)?;
    require_super_admin(&auth)?;

    let (latest_migration_version, migration_count) =
        system_reads::get_migration_status(&state.pool).await?;
    let jobs = job_health_service::read_admin_job_health(&state.pool).await?;
    let queues = system_reads::operational_queue_counts(&state.pool).await?;

    Ok(Json(serde_json::json!({
        "version": BUILD_VERSION,
        "revision": BUILD_REVISION,
        "latest_migration_version": latest_migration_version,
        "migration_count": migration_count,
        "jobs": jobs,
        "queues": queues,
    })))
}
