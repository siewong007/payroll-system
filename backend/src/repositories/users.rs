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
        "SELECT id, roles FROM users WHERE email = $1",
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
        FROM users WHERE id = $1 AND is_active = TRUE"#,
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
        FROM users WHERE email = $1 AND is_active = TRUE"#,
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
        "SELECT id, email, full_name FROM users WHERE email = $1 AND is_active = TRUE",
        email,
    )
    .fetch_optional(executor)
    .await?;
    Ok(user)
}

pub async fn find_id_by_email(
    executor: impl Executor<'_, Database = Postgres>,
    email: &str,
) -> AppResult<Option<Uuid>> {
    let id = sqlx::query_scalar!("SELECT id FROM users WHERE email = $1", email)
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
        "SELECT id FROM users WHERE email = $1 AND id != $2",
        email,
        exclude_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(id)
}

/// Insert an admin-created user (first company as active), returning the projection.
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
        r#"INSERT INTO users (email, password_hash, full_name, roles, company_id)
        VALUES ($1, $2, $3, $4, $5)
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

/// All users, newest first (super-admin view).
pub async fn list_all(executor: impl Executor<'_, Database = Postgres>) -> AppResult<Vec<UserRow>> {
    let users = sqlx::query_as!(
        UserRow,
        r#"SELECT id, email, full_name, roles, company_id, employee_id, is_active, created_at
            FROM users
            ORDER BY created_at DESC"#,
    )
    .fetch_all(executor)
    .await?;
    Ok(users)
}

pub async fn get_projection_by_id(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<Option<UserRow>> {
    let user = sqlx::query_as!(
        UserRow,
        "SELECT id, email, full_name, roles, company_id, employee_id, is_active, created_at FROM users WHERE id = $1",
        id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(user)
}

/// Partial profile/roles update: COALESCE keeps unspecified fields; `roles` is always set.
pub async fn update_profile_and_roles(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    full_name: Option<&str>,
    email: Option<&str>,
    roles: &[String],
    is_active: Option<bool>,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE users SET
            full_name = COALESCE($2, full_name),
            email = COALESCE($3, email),
            roles = $4,
            is_active = COALESCE($5, is_active),
            updated_at = NOW()
        WHERE id = $1"#,
        id,
        full_name,
        email,
        roles,
        is_active,
    )
    .execute(executor)
    .await?;
    Ok(())
}

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

/// Delete a user, returning the number of rows removed (0 ⇒ not found).
pub async fn delete(executor: impl Executor<'_, Database = Postgres>, id: Uuid) -> AppResult<u64> {
    let rows = sqlx::query!("DELETE FROM users WHERE id = $1", id)
        .execute(executor)
        .await?
        .rows_affected();
    Ok(rows)
}

pub async fn delete_by_employee(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
) -> AppResult<()> {
    sqlx::query!("DELETE FROM users WHERE employee_id = $1", employee_id)
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
        "SELECT id FROM users WHERE employee_id = $1 AND is_active = TRUE",
        employee_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(user_id)
}
