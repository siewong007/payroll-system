//! Data access for the `users` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::user::User;

/// Minimal projection used when resolving an existing account by email.
#[derive(Debug)]
pub struct ExistingUser {
    pub id: Uuid,
    pub roles: Vec<String>,
}

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

pub async fn delete(executor: impl Executor<'_, Database = Postgres>, id: Uuid) -> AppResult<()> {
    sqlx::query!("DELETE FROM users WHERE id = $1", id)
        .execute(executor)
        .await?;
    Ok(())
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
