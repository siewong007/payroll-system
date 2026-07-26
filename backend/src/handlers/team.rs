use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, Permission};
use crate::core::error::AppResult;
use crate::models::team::{
    AddTeamMemberRequest, CreateTeamRequest, Team, TeamMember, TeamWithCount, UpdateTeamRequest,
};
use crate::services::team_service;

// ─── Teams CRUD ───

pub async fn list_teams(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<TeamWithCount>>> {
    let company_id = auth.authorize(Permission::ViewTeams)?;
    let teams = team_service::list_teams(&state.pool, company_id).await?;
    Ok(Json(teams))
}

pub async fn get_team(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Team>> {
    let company_id = auth.authorize(Permission::ViewTeams)?;
    let team = team_service::get_team(&state.pool, company_id, id).await?;
    Ok(Json(team))
}

pub async fn create_team(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateTeamRequest>,
) -> AppResult<Json<Team>> {
    let (user_id, company_id) = auth.authorize_actor(Permission::ManageTeams)?;
    let team = team_service::create_team(
        &state.pool,
        company_id,
        &req.name,
        req.description.as_deref(),
        req.tag.as_deref().unwrap_or("general"),
        user_id,
    )
    .await?;
    Ok(Json(team))
}

pub async fn update_team(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateTeamRequest>,
) -> AppResult<Json<Team>> {
    let (user_id, company_id) = auth.authorize_actor(Permission::ManageTeams)?;
    let team = team_service::update_team(
        &state.pool,
        company_id,
        id,
        req.name.as_deref(),
        req.description.as_deref(),
        req.tag.as_deref(),
        req.is_active,
        user_id,
    )
    .await?;
    Ok(Json(team))
}

pub async fn delete_team(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<()>> {
    let company_id = auth.authorize(Permission::ManageTeams)?;
    team_service::delete_team(&state.pool, company_id, id).await?;
    Ok(Json(()))
}

// ─── Members ───

pub async fn list_members(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(team_id): Path<Uuid>,
) -> AppResult<Json<Vec<TeamMember>>> {
    let company_id = auth.authorize(Permission::ViewTeams)?;
    let members = team_service::list_members(&state.pool, company_id, team_id).await?;
    Ok(Json(members))
}

pub async fn add_member(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(team_id): Path<Uuid>,
    Json(req): Json<AddTeamMemberRequest>,
) -> AppResult<Json<TeamMember>> {
    let company_id = auth.authorize(Permission::ManageTeams)?;
    let role = req.role.as_deref().unwrap_or("member");
    let member =
        team_service::add_member(&state.pool, company_id, team_id, req.employee_id, role).await?;
    Ok(Json(member))
}

pub async fn remove_member(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((team_id, employee_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<()>> {
    let company_id = auth.authorize(Permission::ManageTeams)?;
    team_service::remove_member(&state.pool, company_id, team_id, employee_id).await?;
    Ok(Json(()))
}
