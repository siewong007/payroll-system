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
    pub group_permissions: Vec<String>,
}

pub async fn resolve_request_context(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    employee_id: Option<Uuid>,
    session_id: Uuid,
    company_id: Option<Uuid>,
) -> AppResult<AuthRequestContext> {
    let ctx = sqlx::query_as::<_, AuthRequestContext>(
        r#"SELECT
            EXISTS(
                SELECT 1 FROM users
                WHERE id = $1 AND is_active = TRUE AND deleted_at IS NULL
            ) AS user_active,
            COALESCE((
                SELECT must_change_password FROM users
                WHERE id = $1 AND is_active = TRUE AND deleted_at IS NULL
            ), FALSE) AS must_change_password,
            CASE
                WHEN $2::uuid IS NULL THEN NULL
                ELSE (SELECT is_active FROM employees WHERE id = $2)
            END AS employee_active,
            EXISTS(
                SELECT 1 FROM user_sessions
                WHERE id = $3
                  AND user_id = $1
                  AND revoked_at IS NULL
                  AND expires_at > NOW()
                  AND created_at > NOW() - INTERVAL '30 days'
            ) AS session_active,
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
            END AS group_permissions"#,
    )
    .bind(user_id)
    .bind(employee_id)
    .bind(session_id)
    .bind(company_id)
    .fetch_one(executor)
    .await?;

    Ok(ctx)
}
