//! Request-time authentication context read model.
//!
//! Collapses the per-request user, employee, session, and group-permission checks
//! performed by `AuthUser` into a single PostgreSQL round trip.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

#[derive(Debug, sqlx::FromRow)]
pub struct AuthRequestContext {
    pub user_active: bool,
    pub must_change_password: bool,
    pub employee_active: Option<bool>,
    pub session_active: bool,
    pub company_member: bool,
    pub group_permissions: Vec<String>,
}

pub async fn resolve_request_context(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    employee_id: Option<Uuid>,
    session_id: Uuid,
    company_id: Option<Uuid>,
) -> AppResult<AuthRequestContext> {
    let ctx = sqlx::query_as!(
        AuthRequestContext,
        r#"SELECT
            (u.id IS NOT NULL) AS "user_active!",
            COALESCE(u.must_change_password, FALSE) AS "must_change_password!",
            e.is_active AS "employee_active?",
            EXISTS(
                SELECT 1 FROM user_sessions
                WHERE id = $3
                  AND user_id = $1
                  AND revoked_at IS NULL
                  AND expires_at > NOW()
                  AND created_at > NOW() - INTERVAL '30 days'
            ) AS "session_active!",
            CASE
                WHEN $4::uuid IS NULL THEN TRUE
                ELSE EXISTS(
                    SELECT 1 FROM user_companies
                    WHERE user_id = $1 AND company_id = $4
                )
            END AS "company_member!",
            CASE
                WHEN $4::uuid IS NULL THEN ARRAY[]::text[]
                ELSE COALESCE(
                    ARRAY(
                        SELECT DISTINCT p.permission
                        FROM user_group_members m
                        JOIN user_groups g ON g.id = m.group_id
                        JOIN user_group_permissions p ON p.group_id = g.id
                        WHERE m.user_id = $1 AND g.company_id = $4 AND g.is_active = TRUE
                    ),
                    ARRAY[]::text[]
                )
            END AS "group_permissions!"
        FROM (VALUES (1)) AS _single(k)
        LEFT JOIN users u
          ON u.id = $1
         AND u.is_active = TRUE
         AND u.deleted_at IS NULL
        LEFT JOIN employees e
          ON e.id = $2
         AND e.deleted_at IS NULL"#,
        user_id,
        employee_id,
        session_id,
        company_id,
    )
    .fetch_one(executor)
    .await?;

    Ok(ctx)
}
