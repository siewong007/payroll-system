use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::team::{Team, TeamMember, TeamWithCount};

pub async fn list_teams(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<TeamWithCount>> {
    let teams = sqlx::query_as::<_, TeamWithCount>(
        r#"SELECT t.id, t.company_id, t.name, t.description, t.tag, t.is_active,
            t.created_at, t.updated_at,
            (SELECT COUNT(*) FROM team_members tm WHERE tm.team_id = t.id) as member_count
        FROM teams t
        WHERE t.company_id = $1
        ORDER BY t.name"#,
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;
    Ok(teams)
}

pub async fn get_team(pool: &PgPool, company_id: Uuid, team_id: Uuid) -> AppResult<Team> {
    sqlx::query_as::<_, Team>("SELECT * FROM teams WHERE id = $1 AND company_id = $2")
        .bind(team_id)
        .bind(company_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Team not found".into()))
}

pub async fn create_team(
    pool: &PgPool,
    company_id: Uuid,
    name: &str,
    description: Option<&str>,
    tag: &str,
    created_by: Uuid,
) -> AppResult<Team> {
    let team = sqlx::query_as::<_, Team>(
        r#"INSERT INTO teams (company_id, name, description, tag, created_by)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *"#,
    )
    .bind(company_id)
    .bind(name)
    .bind(description)
    .bind(tag)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.constraint().is_some() => {
            AppError::Conflict(format!("A team named '{}' already exists", name))
        }
        _ => AppError::Database(e),
    })?;
    Ok(team)
}

#[allow(clippy::too_many_arguments)]
pub async fn update_team(
    pool: &PgPool,
    company_id: Uuid,
    team_id: Uuid,
    name: Option<&str>,
    description: Option<&str>,
    tag: Option<&str>,
    is_active: Option<bool>,
    updated_by: Uuid,
) -> AppResult<Team> {
    let team = sqlx::query_as::<_, Team>(
        r#"UPDATE teams SET
            name = COALESCE($3, name),
            description = COALESCE($4, description),
            tag = COALESCE($5, tag),
            is_active = COALESCE($6, is_active),
            updated_by = $7,
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *"#,
    )
    .bind(team_id)
    .bind(company_id)
    .bind(name)
    .bind(description)
    .bind(tag)
    .bind(is_active)
    .bind(updated_by)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Team not found".into()))?;
    Ok(team)
}

pub async fn delete_team(pool: &PgPool, company_id: Uuid, team_id: Uuid) -> AppResult<()> {
    let result =
        sqlx::query("DELETE FROM teams WHERE id = $1 AND company_id = $2")
            .bind(team_id)
            .bind(company_id)
            .execute(pool)
            .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Team not found".into()));
    }
    Ok(())
}

// ─── Members ───

pub async fn list_members(pool: &PgPool, team_id: Uuid) -> AppResult<Vec<TeamMember>> {
    let members = sqlx::query_as::<_, TeamMember>(
        r#"SELECT tm.id, tm.team_id, tm.employee_id, tm.role, tm.joined_at,
            e.full_name as employee_name, e.employee_number,
            e.department, e.designation
        FROM team_members tm
        JOIN employees e ON tm.employee_id = e.id
        WHERE tm.team_id = $1
        ORDER BY tm.role DESC, e.full_name"#,
    )
    .bind(team_id)
    .fetch_all(pool)
    .await?;
    Ok(members)
}

pub async fn add_member(
    pool: &PgPool,
    team_id: Uuid,
    employee_id: Uuid,
    role: &str,
) -> AppResult<TeamMember> {
    let member = sqlx::query_as::<_, TeamMember>(
        r#"INSERT INTO team_members (team_id, employee_id, role)
        VALUES ($1, $2, $3)
        RETURNING id, team_id, employee_id, role, joined_at,
            (SELECT full_name FROM employees WHERE id = $2) as employee_name,
            (SELECT employee_number FROM employees WHERE id = $2) as employee_number,
            (SELECT department FROM employees WHERE id = $2) as department,
            (SELECT designation FROM employees WHERE id = $2) as designation"#,
    )
    .bind(team_id)
    .bind(employee_id)
    .bind(role)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.constraint().is_some() => {
            AppError::Conflict("Employee is already a member of this team".into())
        }
        _ => AppError::Database(e),
    })?;
    Ok(member)
}

pub async fn remove_member(pool: &PgPool, team_id: Uuid, employee_id: Uuid) -> AppResult<()> {
    let result = sqlx::query(
        "DELETE FROM team_members WHERE team_id = $1 AND employee_id = $2",
    )
    .bind(team_id)
    .bind(employee_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Member not found in this team".into()));
    }
    Ok(())
}

/// Get teams an employee belongs to
pub async fn get_employee_teams(pool: &PgPool, employee_id: Uuid) -> AppResult<Vec<Team>> {
    let teams = sqlx::query_as::<_, Team>(
        r#"SELECT t.* FROM teams t
        JOIN team_members tm ON t.id = tm.team_id
        WHERE tm.employee_id = $1 AND t.is_active = TRUE
        ORDER BY t.name"#,
    )
    .bind(employee_id)
    .fetch_all(pool)
    .await?;
    Ok(teams)
}
