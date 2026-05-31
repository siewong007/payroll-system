//! Data access for the `users` table.
//!
//! Partial: seeded with the operations `employee_service` needs to provision and
//! tear down employee-linked accounts. Other domains (auth, passkey, oauth2, …) will
//! add their own functions here as they migrate.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

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
