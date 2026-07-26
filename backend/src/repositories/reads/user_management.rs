//! Read-models for user administration: user lists joined across `user_companies`
//! (and `companies`), returning the `UserRow` projection / `CompanySummary`.
//!
//! NOTE: query indentation is matched to the byte-exact SQL in the offline `.sqlx`
//! cache (hashing is whitespace-sensitive). Do not reflow the query strings.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::user_company::{CompanySummary, UserCompanyRow, UserRow};

/// One page of users sharing at least one company with `caller_id`, optionally
/// narrowed to one of those companies and to a name/email search term.
/// Super-admins use `users::list_page` instead.
pub async fn list_page_for_admin(
    executor: impl Executor<'_, Database = Postgres>,
    caller_id: Uuid,
    company_id: Option<Uuid>,
    search: Option<&str>,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<UserRow>> {
    let rows = sqlx::query_as!(
        UserRow,
        r#"SELECT u.id, u.email, u.full_name, u.roles, u.company_id,
                u.employee_id, u.is_active, u.created_at
            FROM users u
            WHERE u.deleted_at IS NULL
              AND EXISTS (
                SELECT 1 FROM user_companies uc
                WHERE uc.user_id = u.id
                  AND uc.company_id IN (
                    SELECT company_id FROM user_companies WHERE user_id = $1)
                  AND ($2::uuid IS NULL OR uc.company_id = $2))
              AND ($3::text IS NULL
                    OR u.full_name ILIKE '%' || $3 || '%'
                    OR u.email ILIKE '%' || $3 || '%')
            ORDER BY u.created_at DESC
            LIMIT $4 OFFSET $5"#,
        caller_id,
        company_id,
        search,
        limit,
        offset,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Total matching `list_page_for_admin`'s filters.
pub async fn count_for_admin(
    executor: impl Executor<'_, Database = Postgres>,
    caller_id: Uuid,
    company_id: Option<Uuid>,
    search: Option<&str>,
) -> AppResult<i64> {
    let total = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!"
            FROM users u
            WHERE u.deleted_at IS NULL
              AND EXISTS (
                SELECT 1 FROM user_companies uc
                WHERE uc.user_id = u.id
                  AND uc.company_id IN (
                    SELECT company_id FROM user_companies WHERE user_id = $1)
                  AND ($2::uuid IS NULL OR uc.company_id = $2))
              AND ($3::text IS NULL
                    OR u.full_name ILIKE '%' || $3 || '%'
                    OR u.email ILIKE '%' || $3 || '%')"#,
        caller_id,
        company_id,
        search,
    )
    .fetch_one(executor)
    .await?;
    Ok(total)
}

/// Company memberships for a batch of users, in one round trip.
///
/// `visible_to = Some(caller_id)` restricts the result to companies the caller is
/// themselves a member of, so a company admin listing a user who also belongs to
/// another tenant does not learn that tenant's name. `None` is the super-admin
/// view. Replaces a per-user query issued inside a loop.
pub async fn list_companies_for_users(
    executor: impl Executor<'_, Database = Postgres>,
    user_ids: &[Uuid],
    visible_to: Option<Uuid>,
) -> AppResult<Vec<UserCompanyRow>> {
    let rows = sqlx::query_as!(
        UserCompanyRow,
        r#"SELECT uc.user_id, c.id AS company_id, c.name AS company_name
        FROM user_companies uc
        JOIN companies c ON c.id = uc.company_id
        WHERE uc.user_id = ANY($1::uuid[])
          AND ($2::uuid IS NULL OR uc.company_id IN (
                SELECT company_id FROM user_companies WHERE user_id = $2))
        ORDER BY uc.user_id, c.name ASC"#,
        user_ids,
        visible_to,
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
