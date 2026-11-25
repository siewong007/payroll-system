//! User groups: company-scoped bundles of permissions granted on top of roles.
//!
//! Groups only ever *add*. There is no negative grant, so a group can never be
//! used to strip a capability a role confers, and "why can this person do X?"
//! stays answerable by union rather than by replaying an order-dependent set of
//! allows and denies.

use sqlx::PgPool;
use uuid::Uuid;

use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::core::permission::{Permission, role_permissions};
use crate::models::user_group::{UserGroup, UserGroupMember, UserGroupWithDetail};
use crate::repositories::{user_companies, user_groups, users};
use crate::services::audit_service::{self, AuditRequestMeta};

const GROUP_ENTITY: &str = "user_group";
const GROUP_MEMBER_ENTITY: &str = "user_group_member";

fn parse_permission(key: &str) -> Option<Permission> {
    Permission::ALL.iter().find(|p| p.as_str() == key).copied()
}

/// Rejects unknown keys, platform-only capabilities, and four-eyes payroll
/// self-approval combinations inside one group.
fn validate_permissions(keys: &[String]) -> AppResult<Vec<String>> {
    let mut validated: Vec<String> = Vec::with_capacity(keys.len());
    let mut has_submit = false;
    let mut has_approve = false;

    for key in keys {
        let perm = parse_permission(key)
            .ok_or_else(|| AppError::BadRequest(format!("Unknown permission: {key}")))?;
        if perm.is_platform_only() {
            return Err(AppError::BadRequest(format!(
                "Platform permission '{key}' cannot be granted via user groups"
            )));
        }
        if perm == Permission::SubmitPayroll {
            has_submit = true;
        }
        if perm == Permission::ApprovePayroll {
            has_approve = true;
        }
        if !validated.contains(key) {
            validated.push(key.clone());
        }
    }

    if has_submit && has_approve {
        return Err(AppError::BadRequest(
            "A single group cannot grant both submit_payroll and approve_payroll".into(),
        ));
    }

    Ok(validated)
}

fn ensure_monotonic_delegation(auth: &AuthUser, permission_keys: &[String]) -> AppResult<()> {
    if auth.is_super_admin() {
        return Ok(());
    }
    for key in permission_keys {
        if let Some(perm) = parse_permission(key) {
            auth.require_permission(perm)?;
        }
    }
    Ok(())
}

fn ensure_user_permission_invariants(
    user_roles: &[String],
    candidate_keys: &[String],
) -> AppResult<()> {
    let is_super_admin = user_roles.iter().any(|r| r == "super_admin");
    // Mirrors the request-time strip in `AuthUser::from_request_parts`: these
    // roles never receive payroll capabilities through a group, so refuse the
    // grant up front rather than store one that silently does nothing.
    let is_restricted = user_roles
        .iter()
        .any(|r| r == "exec" || r == "employee" || r == "manager");

    let mut has_submit = false;
    let mut has_approve = false;

    for role in user_roles {
        for perm in role_permissions(role) {
            if *perm == Permission::SubmitPayroll {
                has_submit = true;
            }
            if *perm == Permission::ApprovePayroll {
                has_approve = true;
            }
        }
    }

    for key in candidate_keys {
        if let Some(perm) = parse_permission(key) {
            if is_restricted && perm.is_payroll_capability() {
                return Err(AppError::BadRequest(
                    "Payroll capabilities cannot be granted to exec, employee or manager accounts"
                        .into(),
                ));
            }
            if perm == Permission::SubmitPayroll {
                has_submit = true;
            }
            if perm == Permission::ApprovePayroll {
                has_approve = true;
            }
        }
    }

    if !is_super_admin && has_submit && has_approve {
        return Err(AppError::BadRequest(
            "User cannot hold both submit_payroll and approve_payroll across roles and groups"
                .into(),
        ));
    }

    Ok(())
}

pub async fn list_groups(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<UserGroupWithDetail>> {
    user_groups::list_for_company(pool, company_id).await
}

pub async fn get_group(pool: &PgPool, company_id: Uuid, group_id: Uuid) -> AppResult<UserGroup> {
    user_groups::get(pool, company_id, group_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Group not found".into()))
}

pub async fn create_group(
    pool: &PgPool,
    company_id: Uuid,
    name: &str,
    description: Option<&str>,
    permissions: &[String],
    auth: &AuthUser,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<UserGroup> {
    let permissions = validate_permissions(permissions)?;
    ensure_monotonic_delegation(auth, &permissions)?;
    let actor_id = auth.0.sub;

    // The group and its grants land together: a group that exists briefly with
    // no permissions is a group that briefly means nothing, and a failure
    // partway would leave one behind.
    let mut tx = pool.begin().await?;
    let group =
        user_groups::insert(&mut *tx, company_id, name.trim(), description, actor_id).await?;
    user_groups::insert_permissions(&mut *tx, group.id, &permissions, actor_id).await?;
    tx.commit().await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "create",
        GROUP_ENTITY,
        Some(group.id),
        None,
        Some(serde_json::json!({
            "name": group.name,
            "description": group.description,
            "permissions": permissions,
        })),
        Some("User group created"),
        audit_meta,
    )
    .await;

    Ok(group)
}

#[allow(clippy::too_many_arguments)]
pub async fn update_group(
    pool: &PgPool,
    company_id: Uuid,
    group_id: Uuid,
    name: Option<&str>,
    description: Option<&str>,
    is_active: Option<bool>,
    permissions: Option<&[String]>,
    auth: &AuthUser,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<UserGroup> {
    let existing = get_group(pool, company_id, group_id).await?;
    let existing_permissions = user_groups::permissions_for(pool, group_id).await?;
    ensure_monotonic_delegation(auth, &existing_permissions)?;

    let validated = match permissions {
        Some(keys) => {
            let v = validate_permissions(keys)?;
            ensure_monotonic_delegation(auth, &v)?;
            Some(v)
        }
        None => None,
    };

    let candidate_keys = validated.as_deref().unwrap_or(&existing_permissions);
    if validated.is_some() || is_active == Some(true) {
        let members = user_groups::member_user_ids(pool, group_id).await?;
        for member_id in members {
            if let Some(user) = users::get_active_by_id(pool, member_id).await? {
                let mut combined = user_groups::user_permissions_in_other_groups(
                    pool, member_id, company_id, group_id,
                )
                .await?;
                combined.extend(candidate_keys.iter().cloned());
                ensure_user_permission_invariants(&user.roles, &combined)?;
            }
        }
    }

    let actor_id = auth.0.sub;
    let mut tx = pool.begin().await?;
    let group = user_groups::update(
        &mut *tx,
        company_id,
        group_id,
        name.map(str::trim),
        description,
        is_active,
        actor_id,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Group not found".into()))?;

    if let Some(keys) = validated.as_deref() {
        // Replaced wholesale inside the transaction, so the group is never
        // observably empty mid-edit — this is a live authorization input.
        user_groups::clear_permissions(&mut *tx, group_id).await?;
        user_groups::insert_permissions(&mut *tx, group_id, keys, actor_id).await?;
    }
    tx.commit().await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "update",
        GROUP_ENTITY,
        Some(group_id),
        Some(serde_json::json!({
            "name": existing.name,
            "description": existing.description,
            "is_active": existing.is_active,
            "permissions": existing_permissions,
        })),
        Some(serde_json::json!({
            "name": group.name,
            "description": group.description,
            "is_active": group.is_active,
            "permissions": validated.unwrap_or(existing_permissions),
        })),
        Some("User group updated"),
        audit_meta,
    )
    .await;

    Ok(group)
}

pub async fn delete_group(
    pool: &PgPool,
    company_id: Uuid,
    group_id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let existing = get_group(pool, company_id, group_id).await?;
    let permissions = user_groups::permissions_for(pool, group_id).await?;

    let rows = user_groups::delete(pool, company_id, group_id).await?;
    if rows == 0 {
        return Err(AppError::NotFound("Group not found".into()));
    }

    // Deleting cascades to memberships and grants, revoking access from
    // everyone who held it. The audit row is the only surviving record of what
    // the group conferred.
    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "delete",
        GROUP_ENTITY,
        Some(group_id),
        Some(serde_json::json!({
            "name": existing.name,
            "permissions": permissions,
        })),
        None,
        Some("User group deleted"),
        audit_meta,
    )
    .await;

    Ok(())
}

// ─── Members ───

pub async fn list_members(
    pool: &PgPool,
    company_id: Uuid,
    group_id: Uuid,
    hide_privileged: bool,
) -> AppResult<Vec<UserGroupMember>> {
    get_group(pool, company_id, group_id).await?;
    user_groups::list_members(pool, company_id, group_id, hide_privileged).await
}

pub async fn add_member(
    pool: &PgPool,
    company_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
    auth: &AuthUser,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    get_group(pool, company_id, group_id).await?;
    let group_perms = user_groups::permissions_for(pool, group_id).await?;
    ensure_monotonic_delegation(auth, &group_perms)?;

    let user = users::get_active_by_id(pool, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;
    if !user_companies::user_has_company(pool, user_id, company_id).await? {
        return Err(AppError::BadRequest(
            "User does not belong to this company".into(),
        ));
    }

    let mut combined =
        user_groups::user_permissions_in_other_groups(pool, user_id, company_id, group_id).await?;
    combined.extend(group_perms);
    ensure_user_permission_invariants(&user.roles, &combined)?;

    let actor_id = auth.0.sub;
    let inserted = user_groups::add_member(pool, company_id, group_id, user_id, actor_id).await?;
    if !inserted {
        // Either the group is not this company's, or the user already holds it.
        // Distinguish the two so a repeated click is not reported as a missing
        // group.
        get_group(pool, company_id, group_id).await?;
        return Err(AppError::Conflict(
            "User is already a member of this group".into(),
        ));
    }

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "create",
        GROUP_MEMBER_ENTITY,
        Some(group_id),
        None,
        Some(serde_json::json!({ "group_id": group_id, "user_id": user_id })),
        Some("User added to group"),
        audit_meta,
    )
    .await;

    Ok(())
}

pub async fn remove_member(
    pool: &PgPool,
    company_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let rows = user_groups::remove_member(pool, company_id, group_id, user_id).await?;
    if rows == 0 {
        return Err(AppError::NotFound("Member not found in this group".into()));
    }

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "delete",
        GROUP_MEMBER_ENTITY,
        Some(group_id),
        Some(serde_json::json!({ "group_id": group_id, "user_id": user_id })),
        None,
        Some("User removed from group"),
        audit_meta,
    )
    .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_permission_is_accepted() {
        let keys = vec!["view_teams".to_string()];
        assert_eq!(validate_permissions(&keys).unwrap(), keys);
    }

    #[test]
    fn an_unknown_permission_is_rejected() {
        // A typo'd key would be stored happily by the free-text column and
        // grant nothing, which is indistinguishable from a broken permission.
        let keys = vec!["view_teamz".to_string()];
        assert!(matches!(
            validate_permissions(&keys),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn duplicates_are_collapsed() {
        let keys = vec!["view_teams".to_string(), "view_teams".to_string()];
        assert_eq!(validate_permissions(&keys).unwrap().len(), 1);
    }

    #[test]
    fn every_non_platform_permission_except_conflicting_pair_is_grantable() {
        let keys: Vec<String> = Permission::ALL
            .iter()
            .filter(|p| !p.is_platform_only() && **p != Permission::ApprovePayroll)
            .map(|p| p.as_str().to_string())
            .collect();
        assert_eq!(validate_permissions(&keys).unwrap().len(), keys.len());
    }

    #[test]
    fn platform_only_and_maker_checker_pairs_are_rejected() {
        for platform_perm in [
            "manage_users",
            "manage_companies",
            "manage_backups",
            "manage_platform_settings",
        ] {
            let keys = vec![platform_perm.to_string()];
            assert!(matches!(
                validate_permissions(&keys),
                Err(AppError::BadRequest(_))
            ));
        }

        let sod_pair = vec!["submit_payroll".to_string(), "approve_payroll".to_string()];
        assert!(matches!(
            validate_permissions(&sod_pair),
            Err(AppError::BadRequest(_))
        ));
    }
}
