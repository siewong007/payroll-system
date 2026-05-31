//! Read-models for teams: member counts and member listings joined across
//! `team_members` / `employees`.
//!
//! NOTE: query indentation matches the byte-exact SQL in the offline `.sqlx` cache.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::team::{Team, TeamMember, TeamWithCount};

/// Teams for a company, each with its member count.
pub async fn list_with_member_count(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<TeamWithCount>> {
    let teams = sqlx::query_as!(
        TeamWithCount,
        r#"SELECT t.id, t.company_id, t.name, t.description, t.tag, t.is_active,
            t.created_at, t.updated_at,
            (SELECT COUNT(*) FROM team_members tm WHERE tm.team_id = t.id) AS member_count
        FROM teams t
        WHERE t.company_id = $1
        ORDER BY t.name"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(teams)
}

/// Members of a team, joined with employee details.
pub async fn list_members(
    executor: impl Executor<'_, Database = Postgres>,
    team_id: Uuid,
) -> AppResult<Vec<TeamMember>> {
    let members = sqlx::query_as!(
        TeamMember,
        r#"SELECT tm.id, tm.team_id, tm.employee_id, tm.role, tm.joined_at,
            e.full_name AS "employee_name?", e.employee_number AS "employee_number?",
            e.department, e.designation
        FROM team_members tm
        JOIN employees e ON tm.employee_id = e.id
        WHERE tm.team_id = $1
        ORDER BY tm.role DESC, e.full_name"#,
        team_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(members)
}

/// Active teams an employee belongs to.
pub async fn list_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
) -> AppResult<Vec<Team>> {
    let teams = sqlx::query_as!(
        Team,
        r#"SELECT t.* FROM teams t
        JOIN team_members tm ON t.id = tm.team_id
        WHERE tm.employee_id = $1 AND t.is_active = TRUE
        ORDER BY t.name"#,
        employee_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(teams)
}
