use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::team::{Team, TeamMember, TeamWithCount};
use crate::repositories::reads::teams as team_reads;
use crate::repositories::{team_members, teams};

pub async fn list_teams(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<TeamWithCount>> {
    team_reads::list_with_member_count(pool, company_id).await
}

pub async fn get_team(pool: &PgPool, company_id: Uuid, team_id: Uuid) -> AppResult<Team> {
    teams::get(pool, company_id, team_id)
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
    teams::insert(pool, company_id, name, description, tag, created_by).await
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
    teams::update(
        pool,
        company_id,
        team_id,
        name,
        description,
        tag,
        is_active,
        updated_by,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Team not found".into()))
}

pub async fn delete_team(pool: &PgPool, company_id: Uuid, team_id: Uuid) -> AppResult<()> {
    let rows = teams::delete(pool, company_id, team_id).await?;
    if rows == 0 {
        return Err(AppError::NotFound("Team not found".into()));
    }
    Ok(())
}

// ─── Members ───
//
// Every entry point here takes the caller's `company_id` and threads it into
// the query. A team id alone is not proof of ownership: these endpoints are
// reachable by any administrator, so an unscoped `team_id` was enough to read
// or edit another tenant's roster.

pub async fn list_members(
    pool: &PgPool,
    company_id: Uuid,
    team_id: Uuid,
) -> AppResult<Vec<TeamMember>> {
    // Resolve the team first so a mistyped id is a 404 rather than an empty
    // list. The read below is scoped independently — that is the guarantee
    // that survives if this check is ever refactored away.
    get_team(pool, company_id, team_id).await?;
    team_reads::list_members(pool, company_id, team_id).await
}

pub async fn add_member(
    pool: &PgPool,
    company_id: Uuid,
    team_id: Uuid,
    employee_id: Uuid,
    role: &str,
) -> AppResult<TeamMember> {
    team_members::insert(pool, company_id, team_id, employee_id, role)
        .await?
        .ok_or_else(|| AppError::NotFound("Team or employee not found".into()))
}

pub async fn remove_member(
    pool: &PgPool,
    company_id: Uuid,
    team_id: Uuid,
    employee_id: Uuid,
) -> AppResult<()> {
    let rows = team_members::delete(pool, company_id, team_id, employee_id).await?;
    if rows == 0 {
        return Err(AppError::NotFound("Member not found in this team".into()));
    }
    Ok(())
}

/// Get teams an employee belongs to
pub async fn get_employee_teams(pool: &PgPool, employee_id: Uuid) -> AppResult<Vec<Team>> {
    team_reads::list_for_employee(pool, employee_id).await
}
