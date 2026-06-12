//! Data access for the `team_members` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::team::TeamMember;

/// Add a member. A unique-constraint violation is translated to `Conflict`.
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    team_id: Uuid,
    employee_id: Uuid,
    role: &str,
) -> AppResult<TeamMember> {
    sqlx::query_as!(
        TeamMember,
        r#"INSERT INTO team_members (team_id, employee_id, role)
        VALUES ($1, $2, $3)
        RETURNING id, team_id, employee_id, role, joined_at,
            (SELECT full_name FROM employees WHERE id = $2) AS employee_name,
            (SELECT employee_number FROM employees WHERE id = $2) AS employee_number,
            (SELECT department FROM employees WHERE id = $2) AS department,
            (SELECT designation FROM employees WHERE id = $2) AS designation"#,
        team_id,
        employee_id,
        role,
    )
    .fetch_one(executor)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.constraint().is_some() => {
            AppError::Conflict("Employee is already a member of this team".into())
        }
        _ => AppError::Database(e),
    })
}

pub async fn delete(
    executor: impl Executor<'_, Database = Postgres>,
    team_id: Uuid,
    employee_id: Uuid,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        "DELETE FROM team_members WHERE team_id = $1 AND employee_id = $2",
        team_id,
        employee_id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}
