use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::team::{Team, TeamMember, TeamWithCount};
use crate::repositories::reads::teams as team_reads;
use crate::repositories::{team_members, teams};
use crate::services::audit_service::{self, AuditRequestMeta};

/// Entity type recorded on `audit_logs` rows for teams and their membership.
const TEAM_ENTITY: &str = "team";
const TEAM_MEMBER_ENTITY: &str = "team_member";

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
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Team> {
    let team = teams::insert(pool, company_id, name, description, tag, created_by).await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(created_by),
        "create",
        TEAM_ENTITY,
        Some(team.id),
        None,
        Some(serde_json::to_value(&team).unwrap_or_default()),
        Some("Team created"),
        audit_meta,
    )
    .await;

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
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Team> {
    // Read the prior state first so the audit row carries a before/after pair
    // rather than only the result — otherwise the trail says a team changed
    // without saying what it was.
    let existing = get_team(pool, company_id, team_id).await?;

    let team = teams::update(
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
    .ok_or_else(|| AppError::NotFound("Team not found".into()))?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(updated_by),
        "update",
        TEAM_ENTITY,
        Some(team.id),
        Some(serde_json::to_value(&existing).unwrap_or_default()),
        Some(serde_json::to_value(&team).unwrap_or_default()),
        Some("Team updated"),
        audit_meta,
    )
    .await;

    Ok(team)
}

pub async fn delete_team(
    pool: &PgPool,
    company_id: Uuid,
    team_id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    // Deleting a team cascades to `team_members`, so capture what existed
    // before it is gone; the row itself is the only remaining record of it.
    let existing = get_team(pool, company_id, team_id).await?;

    let rows = teams::delete(pool, company_id, team_id).await?;
    if rows == 0 {
        return Err(AppError::NotFound("Team not found".into()));
    }

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "delete",
        TEAM_ENTITY,
        Some(team_id),
        Some(serde_json::to_value(&existing).unwrap_or_default()),
        None,
        Some("Team deleted"),
        audit_meta,
    )
    .await;

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
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<TeamMember> {
    let member = team_members::insert(pool, company_id, team_id, employee_id, role)
        .await?
        .ok_or_else(|| AppError::NotFound("Team or employee not found".into()))?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "create",
        TEAM_MEMBER_ENTITY,
        Some(member.id),
        None,
        Some(serde_json::to_value(&member).unwrap_or_default()),
        Some("Employee added to team"),
        audit_meta,
    )
    .await;

    Ok(member)
}

pub async fn remove_member(
    pool: &PgPool,
    company_id: Uuid,
    team_id: Uuid,
    employee_id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let rows = team_members::delete(pool, company_id, team_id, employee_id).await?;
    if rows == 0 {
        return Err(AppError::NotFound("Member not found in this team".into()));
    }

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "delete",
        TEAM_MEMBER_ENTITY,
        // The membership row is gone; the team is the durable anchor an auditor
        // can search on, with the employee recorded in the payload.
        Some(team_id),
        Some(serde_json::json!({
            "team_id": team_id,
            "employee_id": employee_id,
        })),
        None,
        Some("Employee removed from team"),
        audit_meta,
    )
    .await;

    Ok(())
}

/// Get teams an employee belongs to
pub async fn get_employee_teams(pool: &PgPool, employee_id: Uuid) -> AppResult<Vec<Team>> {
    team_reads::list_for_employee(pool, employee_id).await
}
