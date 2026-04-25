use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::user_company::{
    CompanySummary, CreateUserRequest, UpdateUserRequest, UserWithCompanies,
};

const VALID_ROLES: &[&str] = &[
    "super_admin",
    "admin",
    "payroll_admin",
    "hr_manager",
    "finance",
    "exec",
    "employee",
];

const PRIMARY_ROLE_PRIORITY: &[&str] = &[
    "super_admin",
    "admin",
    "payroll_admin",
    "hr_manager",
    "finance",
    "exec",
    "employee",
];

pub async fn create_user(pool: &PgPool, req: CreateUserRequest) -> AppResult<UserWithCompanies> {
    let (primary_role, roles) = normalize_requested_roles(Some(&req.role), req.roles.as_ref())?;

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
    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&req.email)
        .fetch_optional(pool)
        .await?;
    if exists.is_some() {
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
    let user = sqlx::query_as::<_, UserWithCompanies>(
        r#"INSERT INTO users (email, password_hash, full_name, role, roles, company_id)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, email, full_name, role, roles, company_id, employee_id, is_active, created_at"#,
    )
    .bind(&req.email)
    .bind(&password_hash)
    .bind(&req.full_name)
    .bind(&primary_role)
    .bind(&roles)
    .bind(active_company_id)
    .fetch_one(pool)
    .await?;

    // Insert user_companies entries
    for company_id in &req.company_ids {
        sqlx::query(
            "INSERT INTO user_companies (user_id, company_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(user.id)
        .bind(company_id)
        .execute(pool)
        .await?;
    }

    // Fetch companies for response
    let companies = get_user_companies(pool, user.id).await?;
    Ok(UserWithCompanies { companies, ..user })
}

pub async fn list_users(
    pool: &PgPool,
    caller_role: &str,
    caller_id: Uuid,
) -> AppResult<Vec<UserWithCompanies>> {
    let mut users = if caller_role == "super_admin" {
        sqlx::query_as::<_, UserWithCompanies>(
            r#"SELECT id, email, full_name, role, roles, company_id, employee_id, is_active, created_at
            FROM users
            ORDER BY created_at DESC"#,
        )
        .fetch_all(pool)
        .await?
    } else {
        // Admin sees users who share at least one company
        sqlx::query_as::<_, UserWithCompanies>(
            r#"SELECT DISTINCT u.id, u.email, u.full_name, u.role, u.roles, u.company_id,
                u.employee_id, u.is_active, u.created_at
            FROM users u
            JOIN user_companies uc ON u.id = uc.user_id
            WHERE uc.company_id IN (
                SELECT company_id FROM user_companies WHERE user_id = $1
            )
            AND NOT (u.roles = ARRAY['employee']::VARCHAR(50)[])
            ORDER BY u.created_at DESC"#,
        )
        .bind(caller_id)
        .fetch_all(pool)
        .await?
    };

    // Populate companies for each user
    for user in &mut users {
        user.companies = get_user_companies(pool, user.id).await?;
    }

    Ok(users)
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
    let user = sqlx::query_as::<_, UserWithCompanies>(
        "SELECT id, email, full_name, role, roles, company_id, employee_id, is_active, created_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    // Exec can only have one company
    if user.roles.iter().any(|role| role == "exec") && company_ids.len() != 1 {
        return Err(AppError::BadRequest(
            "Exec role can only be assigned to one company".into(),
        ));
    }

    // Remove old assignments
    sqlx::query("DELETE FROM user_companies WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;

    // Insert new assignments
    for company_id in &company_ids {
        sqlx::query("INSERT INTO user_companies (user_id, company_id) VALUES ($1, $2)")
            .bind(user_id)
            .bind(company_id)
            .execute(pool)
            .await?;
    }

    // If current active company is no longer in the list, update it
    let needs_update = user
        .company_id
        .is_none_or(|cid| !company_ids.contains(&cid));
    if needs_update {
        sqlx::query("UPDATE users SET company_id = $2, updated_at = NOW() WHERE id = $1")
            .bind(user_id)
            .bind(company_ids[0])
            .execute(pool)
            .await?;
    }

    // Fetch updated user
    let mut updated = sqlx::query_as::<_, UserWithCompanies>(
        "SELECT id, email, full_name, role, roles, company_id, employee_id, is_active, created_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    updated.companies = get_user_companies(pool, user_id).await?;

    Ok(updated)
}

pub async fn get_user_companies(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<CompanySummary>> {
    let companies = sqlx::query_as::<_, CompanySummary>(
        r#"SELECT c.id, c.name
        FROM user_companies uc
        JOIN companies c ON uc.company_id = c.id
        WHERE uc.user_id = $1
        ORDER BY c.name ASC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(companies)
}

pub async fn switch_company(
    pool: &PgPool,
    user_id: Uuid,
    target_company_id: Uuid,
) -> AppResult<()> {
    // Verify user has access to this company
    let has_access: Option<(Uuid,)> =
        sqlx::query_as("SELECT user_id FROM user_companies WHERE user_id = $1 AND company_id = $2")
            .bind(user_id)
            .bind(target_company_id)
            .fetch_optional(pool)
            .await?;

    if has_access.is_none() {
        return Err(AppError::Forbidden(
            "You do not have access to this company".into(),
        ));
    }

    // Update active company
    sqlx::query("UPDATE users SET company_id = $2, updated_at = NOW() WHERE id = $1")
        .bind(user_id)
        .bind(target_company_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn update_user(
    pool: &PgPool,
    user_id: Uuid,
    req: UpdateUserRequest,
) -> AppResult<UserWithCompanies> {
    // Check user exists
    let existing = sqlx::query_as::<_, UserWithCompanies>(
        "SELECT id, email, full_name, role, roles, company_id, employee_id, is_active, created_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    let (primary_role, roles) = if req.role.is_some() || req.roles.is_some() {
        let fallback_role = if req.roles.is_some() {
            req.role.as_deref()
        } else {
            req.role.as_deref().or(Some(&existing.role))
        };
        normalize_requested_roles(fallback_role, req.roles.as_ref())?
    } else {
        (existing.role.clone(), existing.roles.clone())
    };

    // Check email uniqueness if changing
    if let Some(ref email) = req.email {
        let exists: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM users WHERE email = $1 AND id != $2")
                .bind(email)
                .bind(user_id)
                .fetch_optional(pool)
                .await?;
        if exists.is_some() {
            return Err(AppError::BadRequest(
                "A user with this email already exists".into(),
            ));
        }
    }

    // Update user fields
    sqlx::query(
        r#"UPDATE users SET
            full_name = COALESCE($2, full_name),
            email = COALESCE($3, email),
            role = $4,
            roles = $5,
            is_active = COALESCE($6, is_active),
            updated_at = NOW()
        WHERE id = $1"#,
    )
    .bind(user_id)
    .bind(&req.full_name)
    .bind(&req.email)
    .bind(&primary_role)
    .bind(&roles)
    .bind(req.is_active)
    .execute(pool)
    .await?;

    // Update companies if provided
    if let Some(company_ids) = req.company_ids {
        if roles
            .iter()
            .any(|role| role == "exec" || role == "employee")
            && company_ids.len() > 1
        {
            return Err(AppError::BadRequest(format!(
                "{} role can only be assigned to one company",
                primary_role
            )));
        }
        if !company_ids.is_empty() {
            sqlx::query("DELETE FROM user_companies WHERE user_id = $1")
                .bind(user_id)
                .execute(pool)
                .await?;
            for cid in &company_ids {
                sqlx::query("INSERT INTO user_companies (user_id, company_id) VALUES ($1, $2)")
                    .bind(user_id)
                    .bind(cid)
                    .execute(pool)
                    .await?;
            }
            // Update active company if needed
            let current_company = existing.company_id;
            if current_company.is_none_or(|c| !company_ids.contains(&c)) {
                sqlx::query("UPDATE users SET company_id = $2, updated_at = NOW() WHERE id = $1")
                    .bind(user_id)
                    .bind(company_ids[0])
                    .execute(pool)
                    .await?;
            }
        }
    }

    let mut updated = sqlx::query_as::<_, UserWithCompanies>(
        "SELECT id, email, full_name, role, roles, company_id, employee_id, is_active, created_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    updated.companies = get_user_companies(pool, user_id).await?;
    Ok(updated)
}

fn normalize_requested_roles(
    primary_role: Option<&str>,
    requested_roles: Option<&Vec<String>>,
) -> AppResult<(String, Vec<String>)> {
    let mut roles = requested_roles.cloned().unwrap_or_default();
    if roles.is_empty()
        && let Some(role) = primary_role
    {
        roles.push(role.to_string());
    }

    let mut normalized = Vec::new();
    for role in roles {
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

    if let Some(role) = primary_role
        && !role.trim().is_empty()
        && !normalized.iter().any(|candidate| candidate == role)
    {
        return Err(AppError::BadRequest(
            "Primary role must be included in roles".into(),
        ));
    }

    let primary_role = PRIMARY_ROLE_PRIORITY
        .iter()
        .find(|role| normalized.iter().any(|candidate| candidate == **role))
        .expect("validated role list must have a primary role")
        .to_string();

    Ok((primary_role, normalized))
}

pub async fn delete_user(pool: &PgPool, user_id: Uuid) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("User not found".into()));
    }

    Ok(())
}
