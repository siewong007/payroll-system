use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::user_company::{
    CompanySummary, CreateUserRequest, UpdateUserRequest, UserWithCompanies,
};
use crate::repositories::reads::user_management;
use crate::repositories::{refresh_tokens, user_companies, users};

const VALID_ROLES: &[&str] = &[
    "super_admin",
    "admin",
    "payroll_admin",
    "hr_manager",
    "finance",
    "exec",
    "employee",
];

pub async fn create_user(pool: &PgPool, req: CreateUserRequest) -> AppResult<UserWithCompanies> {
    let roles = normalize_requested_roles(&req.roles)?;

    if roles
        .iter()
        .any(|role| role == "exec" || role == "employee")
        && req.company_ids.len() != 1
    {
        return Err(AppError::BadRequest(
            "Employee and exec roles must be assigned to exactly one company".into(),
        ));
    }

    if req.company_ids.is_empty() {
        return Err(AppError::BadRequest(
            "At least one company must be assigned".into(),
        ));
    }

    // Check email uniqueness
    if users::find_id_by_email(pool, &req.email).await?.is_some() {
        return Err(AppError::BadRequest(
            "A user with this email already exists".into(),
        ));
    }

    // Validate and hash password
    super::auth_service::validate_password_strength(&req.password)?;
    let password_hash = bcrypt::hash(&req.password, 12)
        .map_err(|e| AppError::Internal(format!("Failed to hash password: {}", e)))?;

    // Insert user with first company as active
    let active_company_id = req.company_ids[0];
    let mut user = users::insert_admin(
        pool,
        &req.email,
        &password_hash,
        &req.full_name,
        &roles,
        active_company_id,
    )
    .await?
    .into_user();

    // Insert user_companies entries (idempotent)
    for company_id in &req.company_ids {
        user_companies::insert(pool, user.id, *company_id).await?;
    }

    // Fetch companies for response
    user.companies = get_user_companies(pool, user.id).await?;
    Ok(user)
}

pub async fn list_users(
    pool: &PgPool,
    caller_is_super_admin: bool,
    caller_id: Uuid,
    company_id: Option<Uuid>,
) -> AppResult<Vec<UserWithCompanies>> {
    let mut result: Vec<UserWithCompanies> = if caller_is_super_admin {
        users::list_all(pool, company_id)
            .await?
            .into_iter()
            .map(|row| row.into_user())
            .collect()
    } else {
        // Admin sees users who share at least one company
        user_management::list_for_admin(pool, caller_id)
            .await?
            .into_iter()
            .map(|row| row.into_user())
            .collect()
    };

    // Populate companies for each user
    for user in &mut result {
        user.companies = get_user_companies(pool, user.id).await?;
    }

    Ok(result)
}

pub async fn update_user_companies(
    pool: &PgPool,
    user_id: Uuid,
    company_ids: Vec<Uuid>,
) -> AppResult<UserWithCompanies> {
    if company_ids.is_empty() {
        return Err(AppError::BadRequest(
            "At least one company must be assigned".into(),
        ));
    }

    // Get user to check role
    let user = users::get_projection_by_id(pool, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    // Exec can only have one company
    if user.roles.iter().any(|role| role == "exec") && company_ids.len() != 1 {
        return Err(AppError::BadRequest(
            "Exec role can only be assigned to one company".into(),
        ));
    }

    // Replace assignments
    user_companies::delete_by_user(pool, user_id).await?;
    for company_id in &company_ids {
        user_companies::add(pool, user_id, *company_id).await?;
    }

    // If current active company is no longer in the list, update it
    let needs_update = user
        .company_id
        .is_none_or(|cid| !company_ids.contains(&cid));
    if needs_update {
        users::update_active_company(pool, user_id, company_ids[0]).await?;
    }

    // Fetch updated user
    let mut updated = users::get_projection_by_id(pool, user_id)
        .await?
        .ok_or_else(|| AppError::Internal("User not found".into()))?
        .into_user();
    updated.companies = get_user_companies(pool, user_id).await?;

    Ok(updated)
}

pub async fn get_user_companies(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<CompanySummary>> {
    user_management::list_companies_for_user(pool, user_id).await
}

pub async fn switch_company(
    pool: &PgPool,
    user_id: Uuid,
    target_company_id: Uuid,
) -> AppResult<()> {
    // Verify user has access to this company
    if !user_companies::user_has_company(pool, user_id, target_company_id).await? {
        return Err(AppError::Forbidden(
            "You do not have access to this company".into(),
        ));
    }

    // Update active company
    users::update_active_company(pool, user_id, target_company_id).await?;

    Ok(())
}

pub async fn update_user(
    pool: &PgPool,
    user_id: Uuid,
    req: UpdateUserRequest,
) -> AppResult<UserWithCompanies> {
    // Check user exists
    let existing = users::get_projection_by_id(pool, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    let roles = match req.roles.as_ref() {
        Some(requested) => normalize_requested_roles(requested)?,
        None => existing.roles.clone(),
    };

    // Check email uniqueness if changing
    if let Some(ref email) = req.email
        && users::find_id_by_email_excluding(pool, email, user_id)
            .await?
            .is_some()
    {
        return Err(AppError::BadRequest(
            "A user with this email already exists".into(),
        ));
    }

    // Update user fields
    users::update_profile_and_roles(
        pool,
        user_id,
        req.full_name.as_deref(),
        req.email.as_deref(),
        &roles,
        req.is_active,
    )
    .await?;

    // Update companies if provided
    if let Some(company_ids) = req.company_ids {
        if roles
            .iter()
            .any(|role| role == "exec" || role == "employee")
            && company_ids.len() > 1
        {
            return Err(AppError::BadRequest(
                "Employee and exec roles can only be assigned to one company".into(),
            ));
        }
        if !company_ids.is_empty() {
            user_companies::delete_by_user(pool, user_id).await?;
            for cid in &company_ids {
                user_companies::add(pool, user_id, *cid).await?;
            }
            // Update active company if needed
            let current_company = existing.company_id;
            if current_company.is_none_or(|c| !company_ids.contains(&c)) {
                users::update_active_company(pool, user_id, company_ids[0]).await?;
            }
        }
    }

    let mut updated = users::get_projection_by_id(pool, user_id)
        .await?
        .ok_or_else(|| AppError::Internal("User not found".into()))?
        .into_user();
    updated.companies = get_user_companies(pool, user_id).await?;
    Ok(updated)
}

fn normalize_requested_roles(requested_roles: &[String]) -> AppResult<Vec<String>> {
    let mut normalized = Vec::new();
    for role in requested_roles {
        let role = role.trim().to_string();
        if role.is_empty() {
            continue;
        }
        if !VALID_ROLES.contains(&role.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Invalid role '{}'. Valid roles: {}",
                role,
                VALID_ROLES.join(", ")
            )));
        }
        if !normalized.iter().any(|existing| existing == &role) {
            normalized.push(role);
        }
    }

    if normalized.is_empty() {
        return Err(AppError::BadRequest("At least one role is required".into()));
    }

    if normalized.len() > 1
        && normalized
            .iter()
            .any(|role| role == "employee" || role == "exec")
    {
        return Err(AppError::BadRequest(
            "Employee and exec roles cannot be combined with other roles".into(),
        ));
    }

    Ok(normalized)
}

pub async fn delete_user(pool: &PgPool, user_id: Uuid, deleted_by: Uuid) -> AppResult<()> {
    if user_id == deleted_by {
        return Err(AppError::BadRequest(
            "You cannot delete your own account".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    let rows = users::soft_delete(&mut *tx, user_id, deleted_by).await?;

    if rows == 0 {
        return Err(AppError::NotFound("User not found".into()));
    }

    refresh_tokens::revoke_all_for_user(&mut *tx, user_id).await?;
    user_companies::delete_by_user(&mut *tx, user_id).await?;
    tx.commit().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::normalize_requested_roles;
    use crate::core::error::AppError;

    fn roles(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn role_normalization_trims_deduplicates_and_preserves_order() {
        let normalized =
            normalize_requested_roles(&roles(&[" finance ", "payroll_admin", "finance", " "]))
                .expect("valid roles should normalize");

        assert_eq!(normalized, ["finance", "payroll_admin"]);
    }

    #[test]
    fn role_normalization_rejects_unknown_and_empty_role_sets() {
        assert!(matches!(
            normalize_requested_roles(&roles(&["admin", "root"])),
            Err(AppError::BadRequest(_))
        ));
        assert!(matches!(
            normalize_requested_roles(&roles(&["", "  "])),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn employee_and_exec_roles_cannot_be_combined() {
        for exclusive_role in ["employee", "exec"] {
            assert!(matches!(
                normalize_requested_roles(&roles(&[exclusive_role, "admin"])),
                Err(AppError::BadRequest(_))
            ));
            assert!(normalize_requested_roles(&roles(&[exclusive_role])).is_ok());
        }
    }
}
