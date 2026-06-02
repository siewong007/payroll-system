//! Read-models for user administration: user lists joined across `user_companies`
//! (and `companies`), returning the `UserRow` projection / `CompanySummary`.
//!
//! NOTE: query indentation is matched to the byte-exact SQL in the offline `.sqlx`
//! cache (hashing is whitespace-sensitive). Do not reflow the query strings.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::user_company::{CompanySummary, UserRow};

/// Users sharing at least one company with `caller_id`, excluding pure-employee
/// accounts. (Super-admins use `users::list_all` instead.)
pub async fn list_for_admin(
    executor: impl Executor<'_, Database = Postgres>,
    caller_id: Uuid,
) -> AppResult<Vec<UserRow>> {
    let rows = sqlx::query_as!(
        UserRow,
        r#"SELECT DISTINCT u.id, u.email, u.full_name, u.roles, u.company_id,
                u.employee_id, u.is_active, u.created_at
            FROM users u
            JOIN user_companies uc ON u.id = uc.user_id
            WHERE uc.company_id IN (
                SELECT company_id FROM user_companies WHERE user_id = $1
            )
            AND NOT (u.roles = ARRAY['employee']::VARCHAR(50)[])
            ORDER BY u.created_at DESC"#,
        caller_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Companies a user belongs to, for the `UserWithCompanies.companies` list and the
/// portal company-switcher.
pub async fn list_companies_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<Vec<CompanySummary>> {
    let companies = sqlx::query_as!(
        CompanySummary,
        r#"SELECT c.id, c.name
        FROM user_companies uc
        JOIN companies c ON uc.company_id = c.id
        WHERE uc.user_id = $1
        ORDER BY c.name ASC"#,
        user_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(companies)
}
