//! Administration of platform user accounts.
//!
//! Three invariants drive the shape of this module:
//!
//! 1. **Validate everything before writing anything.** Every rule (roles, company
//!    cardinality, self-lockout, email uniqueness) is checked up front, so a
//!    rejected request leaves no trace. The previous implementation committed the
//!    role change and *then* rejected the company set.
//! 2. **One transaction per mutation.** A user row, its company links, its active
//!    company, the session revocation and the audit row either all land or none do.
//! 3. **One rule, one place.** `validate_company_assignment` is the single
//!    company-assignment rule shared by create and update; previously the two
//!    paths disagreed about `employee`, about empty sets, and about duplicates.

use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::audit::AuditRequestMeta;
use crate::models::user_company::{
    CompanySummary, CreateUserRequest, UpdateUserRequest, UserRow, UserWithCompanies,
};
use crate::repositories::reads::user_management;
use crate::repositories::{audit_logs, refresh_tokens, user_companies, user_sessions, users};

const VALID_ROLES: &[&str] = &[
    "super_admin",
    "admin",
    "payroll_admin",
    "hr_manager",
    "finance",
    "exec",
    "employee",
];

/// Roles that may hold membership in exactly one company. `exec` sees
/// company-wide dashboards and `employee` is a self-service account bound to one
/// employee record; neither has a company switcher.
const SINGLE_COMPANY_ROLES: &[&str] = &["exec", "employee"];

// ─── Pure helpers (unit-tested below, no database) ───

/// Canonical storage form of an email: trimmed and lowercased, so the stored
/// value matches what every `lower(btrim(email))` lookup compares against and
/// what lands in JWT claims, API responses and outbound `To:` headers.
pub(crate) fn normalize_email(raw: &str) -> String {
    raw.trim().to_lowercase()
}

pub(crate) fn normalize_requested_roles(requested_roles: &[String]) -> AppResult<Vec<String>> {
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
            .any(|role| SINGLE_COMPANY_ROLES.contains(&role.as_str()))
    {
        return Err(AppError::BadRequest(
            "Employee and exec roles cannot be combined with other roles".into(),
        ));
    }

    Ok(normalized)
}

pub(crate) fn is_single_company_role_set(roles: &[String]) -> bool {
    roles
        .iter()
        .any(|role| SINGLE_COMPANY_ROLES.contains(&role.as_str()))
}

/// The single company-assignment rule, shared by create and update. Deduplicates
/// while preserving order, rejects an empty set, and requires exactly one company
/// for the `exec` and `employee` roles.
///
/// Deduplication happens *before* counting, so `exec` with `[A, A]` is one
/// company and succeeds rather than tripping the cardinality check — and the
/// caller can pass the result straight to a set-based insert without hitting the
/// primary key.
pub(crate) fn validate_company_assignment(
    roles: &[String],
    company_ids: &[Uuid],
) -> AppResult<Vec<Uuid>> {
    let mut deduped: Vec<Uuid> = Vec::with_capacity(company_ids.len());
    for company_id in company_ids {
        if !deduped.contains(company_id) {
            deduped.push(*company_id);
        }
    }

    if deduped.is_empty() {
        return Err(AppError::BadRequest(
            "At least one company must be assigned".into(),
        ));
    }

    if is_single_company_role_set(roles) && deduped.len() != 1 {
        return Err(AppError::BadRequest(
            "Employee and exec roles must be assigned to exactly one company".into(),
        ));
    }

    Ok(deduped)
}

/// Order-insensitive set comparison, so a re-submitted company selection that
/// merely reorders the ids is not treated as a change (and does not sign the
/// user out of every device).
pub(crate) fn same_set(a: &[Uuid], b: &[Uuid]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut a_sorted = a.to_vec();
    let mut b_sorted = b.to_vec();
    a_sorted.sort_unstable();
    b_sorted.sort_unstable();
    a_sorted == b_sorted
}

// ─── Helpers over the database ───

/// Ends every live session for a user after their roles, company access or
/// active status changed. JWT claims carry `roles` and `company_id` and
/// `AuthUser` never re-reads either, so without this a demoted, deactivated or
/// removed user keeps the old authority — including cross-tenant reads — until
/// their access token expires.
///
/// Takes a connection rather than the pool so it joins the caller's transaction:
/// revocation must commit with the change that made it necessary.
async fn revoke_access(conn: &mut sqlx::PgConnection, user_id: Uuid) -> AppResult<()> {
    user_sessions::revoke_all_for_user(&mut *conn, user_id).await?;
    refresh_tokens::revoke_all_for_user(&mut *conn, user_id).await?;
    Ok(())
}

/// Maps the constraint violations reachable from user writes onto client-visible
/// errors. Without this a duplicate email or a stale company id surfaces as a
/// bare 500.
fn map_user_write_error(err: AppError) -> AppError {
    match err {
        AppError::Database(sqlx::Error::Database(ref e))
            if e.code().as_deref() == Some("23505") =>
        {
            AppError::Conflict("A user with this email already exists".into())
        }
        AppError::Database(sqlx::Error::Database(ref e))
            if e.code().as_deref() == Some("23503") =>
        {
            AppError::BadRequest("One or more selected companies no longer exist".into())
        }
        other => other,
    }
}

fn audit_snapshot(row: &UserRow, company_ids: &[Uuid]) -> serde_json::Value {
    serde_json::json!({
        "email": row.email,
        "full_name": row.full_name,
        "roles": row.roles,
        "is_active": row.is_active,
        "company_ids": company_ids,
    })
}

pub async fn get_user_companies(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<CompanySummary>> {
    user_management::list_companies_for_user(pool, user_id).await
}

async fn company_ids_for(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<Uuid>> {
    Ok(get_user_companies(pool, user_id)
        .await?
        .into_iter()
        .map(|company| company.id)
        .collect())
}

// ─── Commands ───

pub async fn create_user(
    pool: &PgPool,
    req: CreateUserRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<UserWithCompanies> {
    let roles = normalize_requested_roles(&req.roles)?;

    // `employee` accounts are provisioned alongside an employee record, which
    // owns the tenancy and profile link. Creating one here would produce an
    // account with no `employee_id`, which fails every portal call.
    if roles.iter().any(|role| role == "employee") {
        return Err(AppError::BadRequest(
            "Employee accounts are provisioned automatically when an employee record is created"
                .into(),
        ));
    }

    let company_ids = validate_company_assignment(&roles, &req.company_ids)?;
    let email = normalize_email(&req.email);

    if users::find_id_by_email(pool, &email).await?.is_some() {
        return Err(AppError::Conflict(
            "A user with this email already exists".into(),
        ));
    }

    super::auth_service::validate_password_strength(&req.password)?;
    // Hash before opening the transaction: holding a pooled connection idle for
    // the ~300 ms bcrypt takes would starve the pool under concurrent creates.
    let password_hash = super::auth_service::hash_password(&req.password).await?;

    let active_company_id = company_ids[0];
    let mut tx = pool.begin().await?;

    let row = users::insert_admin(
        &mut *tx,
        &email,
        &password_hash,
        req.full_name.trim(),
        &roles,
        active_company_id,
    )
    .await
    .map_err(map_user_write_error)?;

    user_companies::insert_many(&mut *tx, row.id, &company_ids)
        .await
        .map_err(map_user_write_error)?;

    audit_logs::insert(
        &mut *tx,
        Some(active_company_id),
        Some(actor_id),
        "create",
        "user",
        Some(row.id),
        None,
        Some(audit_snapshot(&row, &company_ids)),
        Some(&format!("Created user {}", row.email)),
        audit_meta.and_then(|meta| meta.ip_address.as_deref()),
        audit_meta.and_then(|meta| meta.user_agent.as_deref()),
    )
    .await?;

    tx.commit().await?;

    let mut user = row.into_user();
    user.companies = get_user_companies(pool, user.id).await?;
    Ok(user)
}

pub async fn list_users(
    pool: &PgPool,
    caller_is_super_admin: bool,
    caller_id: Uuid,
    company_id: Option<Uuid>,
    search: Option<&str>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<UserWithCompanies>, i64)> {
    let search = search.map(str::trim).filter(|term| !term.is_empty());

    let (rows, total) = if caller_is_super_admin {
        (
            users::list_page(pool, company_id, search, limit, offset).await?,
            users::count_all(pool, company_id, search).await?,
        )
    } else {
        // A company admin sees only users who share a company with them.
        (
            user_management::list_page_for_admin(
                pool, caller_id, company_id, search, limit, offset,
            )
            .await?,
            user_management::count_for_admin(pool, caller_id, company_id, search).await?,
        )
    };

    let mut result: Vec<UserWithCompanies> = rows.into_iter().map(UserRow::into_user).collect();

    // One batched read for the whole page rather than a query per user. A
    // non-super-admin caller only learns about companies they belong to
    // themselves, so listing a user who also belongs to another tenant does not
    // disclose that tenant's name.
    let user_ids: Vec<Uuid> = result.iter().map(|user| user.id).collect();
    if !user_ids.is_empty() {
        let visible_to = (!caller_is_super_admin).then_some(caller_id);
        let mut by_user: HashMap<Uuid, Vec<CompanySummary>> = HashMap::new();
        for row in user_management::list_companies_for_users(pool, &user_ids, visible_to).await? {
            by_user
                .entry(row.user_id)
                .or_default()
                .push(CompanySummary {
                    id: row.company_id,
                    name: row.company_name,
                });
        }
        for user in &mut result {
            user.companies = by_user.remove(&user.id).unwrap_or_default();
        }
    }

    Ok((result, total))
}

pub async fn update_user(
    pool: &PgPool,
    user_id: Uuid,
    req: UpdateUserRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<UserWithCompanies> {
    let existing = users::get_projection_by_id(pool, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;
    let existing_company_ids = company_ids_for(pool, user_id).await?;

    let roles = match req.roles.as_ref() {
        Some(requested) => normalize_requested_roles(requested)?,
        None => existing.roles.clone(),
    };

    // Validate against the *effective* company set. When the request omits
    // `company_ids`, that is the user's current membership — which is what makes
    // `{"roles": ["exec"]}` on a four-company admin fail here rather than
    // silently producing a multi-company exec.
    let effective_company_ids = validate_company_assignment(
        &roles,
        req.company_ids.as_deref().unwrap_or(&existing_company_ids),
    )?;

    let roles_changed = roles != existing.roles;
    let deactivating = req.is_active == Some(false) && existing.is_active != Some(false);

    // Self-lockout guard. `delete_user` has always refused self-deletion; the
    // same account could nonetheless demote or deactivate itself here and lose
    // the console with no way back in.
    if user_id == actor_id {
        if deactivating {
            return Err(AppError::BadRequest(
                "You cannot deactivate your own account".into(),
            ));
        }
        if existing.roles.iter().any(|role| role == "super_admin")
            && !roles.iter().any(|role| role == "super_admin")
        {
            return Err(AppError::BadRequest(
                "You cannot remove your own super admin role".into(),
            ));
        }
    }

    let email = req.email.as_deref().map(normalize_email);
    if let Some(ref email) = email
        && users::find_id_by_email_excluding(pool, email, user_id)
            .await?
            .is_some()
    {
        return Err(AppError::Conflict(
            "A user with this email already exists".into(),
        ));
    }

    let full_name = req.full_name.as_deref().map(str::trim);

    let mut tx = pool.begin().await?;

    // Refuse a change that would leave the platform with no way to administer
    // it. Checked inside the transaction so two concurrent demotions cannot both
    // observe a count of 2.
    let losing_super_admin = existing.roles.iter().any(|role| role == "super_admin")
        && (!roles.iter().any(|role| role == "super_admin") || deactivating);
    if losing_super_admin && users::count_active_super_admins(&mut *tx).await? <= 1 {
        return Err(AppError::Conflict(
            "At least one active super admin must remain".into(),
        ));
    }

    let updated = users::update_profile_and_roles(
        &mut *tx,
        user_id,
        full_name,
        email.as_deref(),
        &roles,
        req.is_active,
    )
    .await
    .map_err(map_user_write_error)?
    .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    // Only rewrite links when the set actually differs. Comparing presence
    // rather than content is what made a name-only edit sign the user out of
    // every device.
    let companies_changed =
        req.company_ids.is_some() && !same_set(&effective_company_ids, &existing_company_ids);
    if companies_changed {
        user_companies::delete_by_user(&mut *tx, user_id).await?;
        user_companies::insert_many(&mut *tx, user_id, &effective_company_ids)
            .await
            .map_err(map_user_write_error)?;
        if existing
            .company_id
            .is_none_or(|current| !effective_company_ids.contains(&current))
        {
            users::update_active_company(&mut *tx, user_id, effective_company_ids[0]).await?;
        }
    }

    if roles_changed || companies_changed || deactivating {
        revoke_access(&mut tx, user_id).await?;
    }

    audit_logs::insert(
        &mut *tx,
        existing.company_id,
        Some(actor_id),
        "update",
        "user",
        Some(user_id),
        Some(audit_snapshot(&existing, &existing_company_ids)),
        Some(audit_snapshot(&updated, &effective_company_ids)),
        Some(&format!("Updated user {}", updated.email)),
        audit_meta.and_then(|meta| meta.ip_address.as_deref()),
        audit_meta.and_then(|meta| meta.user_agent.as_deref()),
    )
    .await?;

    tx.commit().await?;

    let mut user = updated.into_user();
    user.companies = get_user_companies(pool, user_id).await?;
    Ok(user)
}

pub async fn delete_user(
    pool: &PgPool,
    user_id: Uuid,
    deleted_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    if user_id == deleted_by {
        return Err(AppError::BadRequest(
            "You cannot delete your own account".into(),
        ));
    }

    let existing = users::get_projection_by_id(pool, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;
    let existing_company_ids = company_ids_for(pool, user_id).await?;

    let mut tx = pool.begin().await?;

    if existing.roles.iter().any(|role| role == "super_admin")
        && users::count_active_super_admins(&mut *tx).await? <= 1
    {
        return Err(AppError::Conflict(
            "At least one active super admin must remain".into(),
        ));
    }

    let rows = users::soft_delete(&mut *tx, user_id, deleted_by).await?;
    if rows == 0 {
        return Err(AppError::NotFound("User not found".into()));
    }

    // Both halves of the session state, not just refresh tokens: an access token
    // outlives its refresh token and `AuthUser` checks the session row.
    revoke_access(&mut tx, user_id).await?;
    user_companies::delete_by_user(&mut *tx, user_id).await?;

    audit_logs::insert(
        &mut *tx,
        existing.company_id,
        Some(deleted_by),
        "delete",
        "user",
        Some(user_id),
        Some(audit_snapshot(&existing, &existing_company_ids)),
        None,
        Some(&format!("Deleted user {}", existing.email)),
        audit_meta.and_then(|meta| meta.ip_address.as_deref()),
        audit_meta.and_then(|meta| meta.user_agent.as_deref()),
    )
    .await?;

    tx.commit().await?;

    Ok(())
}

pub async fn switch_company(
    pool: &PgPool,
    user_id: Uuid,
    target_company_id: Uuid,
) -> AppResult<()> {
    // Single statement: the membership check and the write cannot interleave
    // with a concurrent revocation of that membership.
    if !users::set_active_company_if_member(pool, user_id, target_company_id).await? {
        return Err(AppError::Forbidden(
            "You do not have access to this company".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_email, normalize_requested_roles, same_set, validate_company_assignment,
    };
    use crate::core::error::AppError;
    use uuid::Uuid;

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

    #[test]
    fn normalize_email_lowercases_and_trims() {
        assert_eq!(normalize_email("  Admin@Example.COM "), "admin@example.com");
        assert_eq!(normalize_email("already@lower.com"), "already@lower.com");
    }

    #[test]
    fn validate_company_assignment_rejects_an_empty_set_for_any_role() {
        for role_set in [roles(&["admin"]), roles(&["exec"]), roles(&["employee"])] {
            assert!(
                matches!(
                    validate_company_assignment(&role_set, &[]),
                    Err(AppError::BadRequest(_))
                ),
                "empty company set must be rejected for {:?}",
                role_set
            );
        }
    }

    #[test]
    fn validate_company_assignment_rejects_single_company_roles_with_two_companies() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        // `employee` is the rule the old `update_user_companies` omitted entirely.
        for exclusive_role in ["exec", "employee"] {
            assert!(matches!(
                validate_company_assignment(&roles(&[exclusive_role]), &[a, b]),
                Err(AppError::BadRequest(_))
            ));
        }
    }

    #[test]
    fn validate_company_assignment_dedupes_preserving_order() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        let assigned = validate_company_assignment(&roles(&["admin"]), &[a, b, a])
            .expect("duplicates are deduplicated, not rejected");
        assert_eq!(assigned, [a, b]);
    }

    #[test]
    fn validate_company_assignment_dedupes_before_counting_for_exec() {
        let a = Uuid::now_v7();
        let assigned = validate_company_assignment(&roles(&["exec"]), &[a, a])
            .expect("one distinct company satisfies the exec rule");
        assert_eq!(assigned, [a]);
    }

    #[test]
    fn validate_company_assignment_allows_multiple_companies_for_admin() {
        let ids = [Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7()];
        let assigned = validate_company_assignment(&roles(&["admin"]), &ids)
            .expect("multi-company admins are the normal case");
        assert_eq!(assigned, ids);
    }

    #[test]
    fn same_set_is_order_insensitive_and_length_sensitive() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        assert!(same_set(&[a, b], &[b, a]));
        assert!(same_set(&[], &[]));
        assert!(!same_set(&[a], &[a, b]));
        assert!(!same_set(&[a], &[b]));
    }
}
