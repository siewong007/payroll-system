//! User groups: company-scoped bundles of permissions granted on top of roles.
//!
//! Groups only ever *add*. There is no negative grant, so a group can never be
//! used to strip a capability a role confers, and "why can this person do X?"
//! stays answerable by union rather than by replaying an order-dependent set of
//! allows and denies.

use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::core::permission::Permission;
use crate::models::user_group::{UserGroup, UserGroupMember, UserGroupWithDetail};
use crate::repositories::user_groups;
use crate::services::audit_service::{self, AuditRequestMeta};

const GROUP_ENTITY: &str = "user_group";
const GROUP_MEMBER_ENTITY: &str = "user_group_member";

/// Rejects any key that is not a current `Permission`.
///
/// The permission column is free text on purpose — a CHECK constraint would be
/// a second copy of the enum needing a migration per capability. That makes
/// this the only gate, so it runs before every write: a typo'd key would
/// otherwise be stored silently and grant nothing, which looks exactly like a
/// permission that does not work.
fn validate_permissions(keys: &[String]) -> AppResult<Vec<String>> {
    let mut validated: Vec<String> = Vec::with_capacity(keys.len());
    for key in keys {
        let known = Permission::ALL.iter().any(|p| p.as_str() == key);
        if !known {
            return Err(AppError::BadRequest(format!("Unknown permission: {key}")));
        }
        if !validated.contains(key) {
            validated.push(key.clone());
        }
    }
    Ok(validated)
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
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<UserGroup> {
    let permissions = validate_permissions(permissions)?;

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
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<UserGroup> {
    let existing = get_group(pool, company_id, group_id).await?;
    let existing_permissions = user_groups::permissions_for(pool, group_id).await?;

    let validated = match permissions {
        Some(keys) => Some(validate_permissions(keys)?),
        None => None,
    };

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
) -> AppResult<Vec<UserGroupMember>> {
    get_group(pool, company_id, group_id).await?;
    user_groups::list_members(pool, company_id, group_id).await
}

pub async fn add_member(
    pool: &PgPool,
    company_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
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
    fn every_permission_in_the_enum_is_grantable() {
        let keys: Vec<String> = Permission::ALL
            .iter()
            .map(|p| p.as_str().to_string())
            .collect();
        assert_eq!(validate_permissions(&keys).unwrap().len(), keys.len());
    }
}
