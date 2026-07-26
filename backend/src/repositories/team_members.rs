//! Data access for the `team_members` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::team::TeamMember;

/// Translates a constraint violation to the error that actually describes it.
///
/// Matching on `constraint().is_some()` alone reported every violation as
/// "already a member" — so an invalid `role` (rejected by
/// `team_members_role_check`) and a cross-company link (rejected by the
/// `team_members_same_company_trigger`) both surfaced as a duplicate-member
/// conflict, which is a misleading thing to show an administrator.
fn member_constraint_error(e: sqlx::Error) -> AppError {
    let constraint = match &e {
        sqlx::Error::Database(db_err) => db_err.constraint(),
        _ => None,
    };
    match constraint {
        Some("team_members_team_id_employee_id_key") => {
            AppError::Conflict("Employee is already a member of this team".into())
        }
        Some("team_members_role_check") => {
            AppError::BadRequest("Team member role must be 'member' or 'lead'".into())
        }
        Some("team_members_same_company_check") => {
            AppError::BadRequest("Employee belongs to a different company".into())
        }
        _ => AppError::Database(e),
    }
}

/// Add a member to a team owned by `company_id`.
///
/// The tenant guard is in the SQL rather than at the call site: the row is
/// inserted only if the team *and* the employee both resolve inside
/// `company_id`, so there is no way to reach this function unscoped. Returns
/// `None` when either does not resolve — the caller maps that to `NotFound`,
/// which is deliberately indistinguishable from a genuinely missing team so a
/// caller in another company learns nothing about what exists.
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    team_id: Uuid,
    employee_id: Uuid,
    role: &str,
) -> AppResult<Option<TeamMember>> {
    sqlx::query_as!(
        TeamMember,
        r#"INSERT INTO team_members (team_id, employee_id, role)
        SELECT t.id, e.id, $4
        FROM teams t
        JOIN employees e ON e.company_id = t.company_id
        WHERE t.id = $1 AND e.id = $2 AND t.company_id = $3
            AND e.deleted_at IS NULL
        RETURNING id, team_id, employee_id, role, joined_at,
            (SELECT full_name FROM employees WHERE id = $2) AS employee_name,
            (SELECT employee_number FROM employees WHERE id = $2) AS employee_number,
            (SELECT department FROM employees WHERE id = $2) AS department,
            (SELECT designation FROM employees WHERE id = $2) AS designation"#,
        team_id,
        employee_id,
        company_id,
        role,
    )
    .fetch_optional(executor)
    .await
    .map_err(member_constraint_error)
}

/// Remove a member from a team owned by `company_id`. Scoped in SQL for the
/// same reason as `insert`.
pub async fn delete(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    team_id: Uuid,
    employee_id: Uuid,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        r#"DELETE FROM team_members tm
        USING teams t
        WHERE tm.team_id = t.id
            AND tm.team_id = $1
            AND tm.employee_id = $2
            AND t.company_id = $3"#,
        team_id,
        employee_id,
        company_id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}
