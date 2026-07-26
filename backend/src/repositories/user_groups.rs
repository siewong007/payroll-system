//! Data access for `user_groups`, `user_group_permissions` and
//! `user_group_members`.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::user_group::{UserGroup, UserGroupMember, UserGroupWithDetail};

/// The permission keys a user holds through group membership in one company.
///
/// Runs on every authenticated request, so it is deliberately one indexed
/// query returning a flat `Vec<String>` rather than a join the caller has to
/// reshape. Inactive groups grant nothing — deactivating a group is the
/// intended way to suspend its access without losing its membership list.
pub async fn effective_permissions(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    company_id: Uuid,
) -> AppResult<Vec<String>> {
    let rows = sqlx::query_scalar!(
        r#"SELECT DISTINCT p.permission
        FROM user_group_members m
        JOIN user_groups g ON g.id = m.group_id
        JOIN user_group_permissions p ON p.group_id = g.id
        WHERE m.user_id = $1 AND g.company_id = $2 AND g.is_active = TRUE"#,
        user_id,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn list_for_company(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<UserGroupWithDetail>> {
    let groups = sqlx::query_as!(
        UserGroupWithDetail,
        r#"SELECT g.id, g.company_id, g.name, g.description, g.is_active,
            g.created_at, g.updated_at,
            COALESCE(
                ARRAY(SELECT p.permission FROM user_group_permissions p
                      WHERE p.group_id = g.id ORDER BY p.permission),
                ARRAY[]::VARCHAR[]
            ) AS "permissions!: Vec<String>",
            (SELECT COUNT(*) FROM user_group_members m WHERE m.group_id = g.id)
                AS "member_count!: i64"
        FROM user_groups g
        WHERE g.company_id = $1
        ORDER BY g.name"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(groups)
}

pub async fn get(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    group_id: Uuid,
) -> AppResult<Option<UserGroup>> {
    let group = sqlx::query_as!(
        UserGroup,
        "SELECT * FROM user_groups WHERE id = $1 AND company_id = $2",
        group_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(group)
}

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    name: &str,
    description: Option<&str>,
    created_by: Uuid,
) -> AppResult<UserGroup> {
    sqlx::query_as!(
        UserGroup,
        r#"INSERT INTO user_groups (company_id, name, description, created_by, updated_by)
        VALUES ($1, $2, $3, $4, $4)
        RETURNING *"#,
        company_id,
        name,
        description,
        created_by,
    )
    .fetch_one(executor)
    .await
    .map_err(|e| group_constraint_error(e, name))
}

pub async fn update(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    group_id: Uuid,
    name: Option<&str>,
    description: Option<&str>,
    is_active: Option<bool>,
    updated_by: Uuid,
) -> AppResult<Option<UserGroup>> {
    let group = sqlx::query_as!(
        UserGroup,
        r#"UPDATE user_groups SET
            name = COALESCE($3, name),
            description = COALESCE($4, description),
            is_active = COALESCE($5, is_active),
            updated_by = $6,
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *"#,
        group_id,
        company_id,
        name,
        description,
        is_active,
        updated_by,
    )
    .fetch_optional(executor)
    .await
    .map_err(|e| group_constraint_error(e, name.unwrap_or("")))?;
    Ok(group)
}

pub async fn delete(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    group_id: Uuid,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        "DELETE FROM user_groups WHERE id = $1 AND company_id = $2",
        group_id,
        company_id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}

fn group_constraint_error(e: sqlx::Error, name: &str) -> AppError {
    let constraint = match &e {
        sqlx::Error::Database(db_err) => db_err.constraint(),
        _ => None,
    };
    match constraint {
        Some("user_groups_company_name_key") => {
            AppError::Conflict(format!("A group named '{name}' already exists"))
        }
        Some("user_groups_name_not_blank") => {
            AppError::BadRequest("Group name cannot be blank".into())
        }
        _ => AppError::Database(e),
    }
}

// ─── Permissions ───

/// Replace a group's permission set wholesale.
///
/// The UI edits the set as a unit, so a delete-then-insert inside the caller's
/// transaction is both simpler and less prone to drift than diffing. Callers
/// pass `&mut tx` so the group is never briefly left with no permissions.
pub async fn clear_permissions(
    executor: impl Executor<'_, Database = Postgres>,
    group_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM user_group_permissions WHERE group_id = $1",
        group_id
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_permissions(
    executor: impl Executor<'_, Database = Postgres>,
    group_id: Uuid,
    permissions: &[String],
    granted_by: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO user_group_permissions (group_id, permission, granted_by)
        SELECT $1, permission, $3
        FROM UNNEST($2::VARCHAR[]) AS permission
        ON CONFLICT (group_id, permission) DO NOTHING"#,
        group_id,
        permissions,
        granted_by,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn permissions_for(
    executor: impl Executor<'_, Database = Postgres>,
    group_id: Uuid,
) -> AppResult<Vec<String>> {
    let rows = sqlx::query_scalar!(
        "SELECT permission FROM user_group_permissions WHERE group_id = $1 ORDER BY permission",
        group_id
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

// ─── Members ───

pub async fn list_members(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    group_id: Uuid,
) -> AppResult<Vec<UserGroupMember>> {
    let members = sqlx::query_as!(
        UserGroupMember,
        r#"SELECT m.group_id, m.user_id, m.added_at,
            u.full_name AS "full_name!", u.email AS "email!"
        FROM user_group_members m
        JOIN user_groups g ON g.id = m.group_id
        JOIN users u ON u.id = m.user_id
        WHERE m.group_id = $1 AND g.company_id = $2 AND u.deleted_at IS NULL
        ORDER BY u.full_name"#,
        group_id,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(members)
}

/// Add a member, scoped in SQL to a group the company owns.
///
/// Returns `false` when the group does not resolve inside `company_id`, which
/// the caller maps to `NotFound` — deliberately indistinguishable from a
/// missing group, so a caller in another company learns nothing.
pub async fn add_member(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
    added_by: Uuid,
) -> AppResult<bool> {
    let rows = sqlx::query!(
        r#"INSERT INTO user_group_members (group_id, user_id, added_by)
        SELECT g.id, $3, $4
        FROM user_groups g
        WHERE g.id = $1 AND g.company_id = $2
        ON CONFLICT (group_id, user_id) DO NOTHING"#,
        group_id,
        company_id,
        user_id,
        added_by,
    )
    .execute(executor)
    .await
    .map_err(member_constraint_error)?
    .rows_affected();
    Ok(rows > 0)
}

pub async fn remove_member(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        r#"DELETE FROM user_group_members m
        USING user_groups g
        WHERE m.group_id = g.id
            AND m.group_id = $1
            AND m.user_id = $2
            AND g.company_id = $3"#,
        group_id,
        user_id,
        company_id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}

fn member_constraint_error(e: sqlx::Error) -> AppError {
    let constraint = match &e {
        sqlx::Error::Database(db_err) => db_err.constraint(),
        _ => None,
    };
    match constraint {
        Some("user_group_members_same_company_check") => {
            AppError::BadRequest("User does not belong to this company".into())
        }
        _ => AppError::Database(e),
    }
}
