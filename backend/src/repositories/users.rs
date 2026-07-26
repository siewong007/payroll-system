//! Data access for the `users` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::user::{ExistingUser, User, UserContact};
use crate::models::user_company::UserRow;

pub async fn find_by_email(
    executor: impl Executor<'_, Database = Postgres>,
    email: &str,
) -> AppResult<Option<ExistingUser>> {
    let row = sqlx::query_as!(
        ExistingUser,
        r#"SELECT id, roles, company_id, deleted_at IS NOT NULL AS "is_deleted!"
        FROM users WHERE lower(btrim(email)) = lower(btrim($1))"#,
        email,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

pub async fn get_by_id(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<Option<User>> {
    let user = sqlx::query_as!(
        User,
        r#"SELECT id, email, password_hash, full_name, roles, company_id,
            employee_id, is_active, must_change_password, last_login, created_at, updated_at
        FROM users WHERE id = $1"#,
        id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(user)
}

pub async fn get_active_by_id(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<Option<User>> {
    let user = sqlx::query_as!(
        User,
        r#"SELECT id, email, password_hash, full_name, roles, company_id,
            employee_id, is_active, must_change_password, last_login, created_at, updated_at
        FROM users WHERE id = $1 AND is_active = TRUE AND deleted_at IS NULL"#,
        id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(user)
}

pub async fn find_active_by_email(
    executor: impl Executor<'_, Database = Postgres>,
    email: &str,
) -> AppResult<Option<User>> {
    let user = sqlx::query_as!(
        User,
        r#"SELECT id, email, password_hash, full_name, roles, company_id,
            employee_id, is_active, must_change_password, last_login, created_at, updated_at
        FROM users WHERE lower(btrim(email)) = lower(btrim($1)) AND is_active = TRUE AND deleted_at IS NULL"#,
        email,
    )
    .fetch_optional(executor)
    .await?;
    Ok(user)
}

pub async fn find_active_contact_by_email(
    executor: impl Executor<'_, Database = Postgres>,
    email: &str,
) -> AppResult<Option<UserContact>> {
    let user = sqlx::query_as!(
        UserContact,
        "SELECT id, email, full_name FROM users WHERE lower(btrim(email)) = lower(btrim($1)) AND is_active = TRUE AND deleted_at IS NULL",
        email,
    )
    .fetch_optional(executor)
    .await?;
    Ok(user)
}

/// `(email, full_name)` for a user by id; errors if the user does not exist.
pub async fn name_and_email(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<(String, String)> {
    let row = sqlx::query!("SELECT email, full_name FROM users WHERE id = $1", id)
        .fetch_one(executor)
        .await?;
    Ok((row.email, row.full_name))
}

pub async fn find_id_by_email(
    executor: impl Executor<'_, Database = Postgres>,
    email: &str,
) -> AppResult<Option<Uuid>> {
    let id = sqlx::query_scalar!(
        "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1))",
        email,
    )
    .fetch_optional(executor)
    .await?;
    Ok(id)
}

pub async fn find_id_by_email_excluding(
    executor: impl Executor<'_, Database = Postgres>,
    email: &str,
    exclude_id: Uuid,
) -> AppResult<Option<Uuid>> {
    let id = sqlx::query_scalar!(
        "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) AND id != $2",
        email,
        exclude_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(id)
}

/// Insert an admin-created user (first company as active), returning the projection.
///
/// `must_change_password` is set: an administrator chose and typed this password,
/// so the account holder must replace it with one only they know. Mirrors the
/// auto-provisioned employee accounts in `insert_employee_user`.
pub async fn insert_admin(
    executor: impl Executor<'_, Database = Postgres>,
    email: &str,
    password_hash: &str,
    full_name: &str,
    roles: &[String],
    company_id: Uuid,
) -> AppResult<UserRow> {
    let user = sqlx::query_as!(
        UserRow,
        r#"INSERT INTO users (email, password_hash, full_name, roles, company_id, must_change_password)
        VALUES ($1, $2, $3, $4, $5, TRUE)
        RETURNING id, email, full_name, roles, company_id, employee_id, is_active, created_at"#,
        email,
        password_hash,
        full_name,
        roles,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(user)
}

/// One page of users, newest first (super-admin view), optionally restricted to a
/// company and to a name/email search term.
///
/// Uses an `EXISTS` semi-join rather than `LEFT JOIN` + `DISTINCT`: the join form
/// multiplied each user by their company-link count and then paid a sort to
/// collapse the duplicates back, which also made `LIMIT` meaningless.
pub async fn list_page(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Option<Uuid>,
    search: Option<&str>,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<UserRow>> {
    let users = sqlx::query_as!(
        UserRow,
        r#"SELECT u.id, u.email, u.full_name, u.roles, u.company_id,
                u.employee_id, u.is_active, u.created_at
            FROM users u
            WHERE u.deleted_at IS NULL
              AND ($1::uuid IS NULL OR EXISTS (
                    SELECT 1 FROM user_companies uc
                    WHERE uc.user_id = u.id AND uc.company_id = $1))
              AND ($2::text IS NULL
                    OR u.full_name ILIKE '%' || $2 || '%'
                    OR u.email ILIKE '%' || $2 || '%')
            ORDER BY u.created_at DESC
            LIMIT $3 OFFSET $4"#,
        company_id,
        search,
        limit,
        offset,
    )
    .fetch_all(executor)
    .await?;
    Ok(users)
}

/// Total matching `list_page`'s filters, for the paginated response envelope.
pub async fn count_all(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Option<Uuid>,
    search: Option<&str>,
) -> AppResult<i64> {
    let total = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!"
            FROM users u
            WHERE u.deleted_at IS NULL
              AND ($1::uuid IS NULL OR EXISTS (
                    SELECT 1 FROM user_companies uc
                    WHERE uc.user_id = u.id AND uc.company_id = $1))
              AND ($2::text IS NULL
                    OR u.full_name ILIKE '%' || $2 || '%'
                    OR u.email ILIKE '%' || $2 || '%')"#,
        company_id,
        search,
    )
    .fetch_one(executor)
    .await?;
    Ok(total)
}

/// Number of live accounts holding `super_admin`. Checked inside the update and
/// delete transactions so the platform can never be left unadministrable.
pub async fn count_active_super_admins(
    executor: impl Executor<'_, Database = Postgres>,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM users
        WHERE 'super_admin' = ANY(roles) AND is_active = TRUE AND deleted_at IS NULL"#,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}

pub async fn get_projection_by_id(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<Option<UserRow>> {
    let user = sqlx::query_as!(
        UserRow,
        "SELECT id, email, full_name, roles, company_id, employee_id, is_active, created_at FROM users WHERE id = $1 AND deleted_at IS NULL",
        id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(user)
}

/// Partial profile/roles update: COALESCE keeps unspecified fields; `roles` is
/// always set. Returns the updated projection, or `None` when no live row
/// matched — previously the row count was discarded, so updating a deleted or
/// non-existent user reported success.
pub async fn update_profile_and_roles(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    full_name: Option<&str>,
    email: Option<&str>,
    roles: &[String],
    is_active: Option<bool>,
) -> AppResult<Option<UserRow>> {
    let row = sqlx::query_as!(
        UserRow,
        r#"UPDATE users SET
            full_name = COALESCE($2, full_name),
            email = COALESCE($3, email),
            roles = $4,
            is_active = COALESCE($5, is_active),
            updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        RETURNING id, email, full_name, roles, company_id, employee_id, is_active, created_at"#,
        id,
        full_name,
        email,
        roles,
        is_active,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

/// Points the active company at `company_id` only if the user is actually a
/// member of it. Returns false when they are not (or the account is deleted),
/// which closes the check-then-write race in `switch_company`.
pub async fn set_active_company_if_member(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    company_id: Uuid,
) -> AppResult<bool> {
    let rows = sqlx::query!(
        r#"UPDATE users SET company_id = $2, updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
          AND EXISTS (
            SELECT 1 FROM user_companies
            WHERE user_id = $1 AND company_id = $2)"#,
        user_id,
        company_id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows > 0)
}

/// Unconditional active-company repoint. Used by the admin reassignment path,
/// which re-points to a link row written earlier in the same transaction (so
/// `set_active_company_if_member` would race its own uncommitted write).
pub async fn update_active_company(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE users SET company_id = $2, updated_at = NOW() WHERE id = $1",
        id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Link an existing (non-employee) account to an employee record.
pub async fn link_to_employee(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    company_id: Uuid,
    user_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE users SET employee_id = $1, company_id = $2 WHERE id = $3",
        employee_id,
        company_id,
        user_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Create the auto-provisioned `employee`-role account for a new employee.
pub async fn insert_employee_user(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    email: &str,
    password_hash: &str,
    full_name: &str,
    company_id: Uuid,
    employee_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash, full_name, roles, company_id, employee_id, must_change_password)
        VALUES ($1, $2, $3, $4, ARRAY['employee']::VARCHAR(50)[], $5, $6, TRUE)"#,
        id,
        email,
        password_hash,
        full_name,
        company_id,
        employee_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn update_last_login(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<()> {
    sqlx::query!("UPDATE users SET last_login = NOW() WHERE id = $1", id)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn update_password(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    password_hash: &str,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE users SET password_hash = $1, must_change_password = FALSE, updated_at = NOW() WHERE id = $2",
        password_hash,
        id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Set the password hash only (does not touch `must_change_password`); used by the
/// password-reset flow. Cf. `update_password`, which also clears that flag.
pub async fn set_password(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    password_hash: &str,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE users SET password_hash = $2, updated_at = NOW() WHERE id = $1",
        id,
        password_hash,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn clear_must_change_password(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE users SET must_change_password = FALSE, updated_at = NOW() WHERE id = $1",
        id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Tombstone a user so imports cannot recreate their account, and revoke their
/// active access in the same transaction at the service layer.
pub async fn soft_delete(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    deleted_by: Uuid,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        "UPDATE users SET is_active = FALSE, deleted_at = NOW(), deleted_by = $2, updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        id,
        deleted_by,
    )
        .execute(executor)
        .await?
        .rows_affected();
    Ok(rows)
}

/// Permanently remove a user only as part of employee lifecycle cleanup.
/// Super-admin user deletion uses `soft_delete` instead.
pub async fn delete(executor: impl Executor<'_, Database = Postgres>, id: Uuid) -> AppResult<u64> {
    let rows = sqlx::query!("DELETE FROM users WHERE id = $1", id)
        .execute(executor)
        .await?
        .rows_affected();
    Ok(rows)
}

/// Remove the self-service account belonging to an employee.
///
/// The `roles <@ ARRAY['employee']` predicate is a hard backstop: this is a
/// *hard* delete driven by employee lifecycle, and a privileged account must
/// never be destroyed through it no matter how `employee_id` came to point at
/// that row. `employee_service` also refuses to create such a link in the first
/// place; this is the second lock on the same door.
/// Retire the portal login belonging to a deleted employee.
///
/// A soft delete, not a `DELETE`. Seventeen foreign keys reference `users` with
/// no ON DELETE policy — `attendance_records.created_by`,
/// `payroll_runs.approved_by`, `leave_requests.reviewed_by`,
/// `teams.created_by`, and so on — so the hard delete this used to be raised
/// 23503 for any user who had ever done anything in the product. Deleting an
/// employee therefore failed outright for every non-trivial account.
///
/// Soft-deleting also matches how the rest of the system already behaves:
/// `admin::delete_user` uses `soft_delete`, and `create_user_for_employee`
/// explicitly refuses to resurrect an account with `is_deleted` set, so
/// tombstones are an expected state rather than a new one.
///
/// The `roles <@ ARRAY['employee']` guard is kept: an account that also holds an
/// administrative role is not merely this employee's login and must survive.
pub async fn soft_delete_by_employee(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    deleted_by: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE users SET
            is_active = FALSE, deleted_at = NOW(), deleted_by = $2, updated_at = NOW()
        WHERE employee_id = $1
            AND deleted_at IS NULL
            AND roles <@ ARRAY['employee']::VARCHAR(50)[]"#,
        employee_id,
        deleted_by,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// The id of the active user account linked to an employee, if any (used to
/// target in-app notifications).
pub async fn active_id_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
) -> AppResult<Option<Uuid>> {
    let user_id = sqlx::query_scalar!(
        "SELECT id FROM users WHERE employee_id = $1 AND is_active = TRUE AND deleted_at IS NULL",
        employee_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(user_id)
}
