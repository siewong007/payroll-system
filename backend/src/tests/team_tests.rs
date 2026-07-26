//! Tenant-isolation regression tests for the teams subsystem.
//!
//! The member endpoints used to take a bare `team_id` and never compare it
//! against the caller's company, so any administrator could read or edit
//! another tenant's roster by guessing a UUID. Every test here drives the
//! service layer with company A's id against company B's team and asserts the
//! call fails — if the scoping is ever dropped from the SQL these turn red.

use uuid::Uuid;

use crate::core::error::AppError;
use crate::services::team_service;
use crate::tests::support::{seed_company, seed_employee, seed_user, skip_if_no_db};

/// Seeds a company with one team and one employee already in it.
/// Returns `(company_id, team_id, employee_id)`.
async fn seed_team_with_member(pool: &sqlx::PgPool) -> (Uuid, Uuid, Uuid) {
    let company_id = seed_company(pool).await;
    let user_id = seed_user(pool, company_id, "admin").await;
    let employee_id = seed_employee(pool, company_id, None, 500_000).await;

    let team = team_service::create_team(
        pool,
        company_id,
        &format!("Team-{}", &Uuid::new_v4().to_string()[..8]),
        None,
        "general",
        user_id,
    )
    .await
    .expect("create team");

    team_service::add_member(pool, company_id, team.id, employee_id, "member")
        .await
        .expect("seed member");

    (company_id, team.id, employee_id)
}

#[tokio::test]
async fn list_members_rejects_a_team_in_another_company() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (_, victim_team, _) = seed_team_with_member(&pool).await;
    let attacker_company = seed_company(&pool).await;

    let result = team_service::list_members(&pool, attacker_company, victim_team).await;

    assert!(
        matches!(result, Err(AppError::NotFound(_))),
        "another company's roster must not be readable, got {result:?}"
    );
}

#[tokio::test]
async fn remove_member_rejects_a_team_in_another_company() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (victim_company, victim_team, victim_employee) = seed_team_with_member(&pool).await;
    let attacker_company = seed_company(&pool).await;

    let result =
        team_service::remove_member(&pool, attacker_company, victim_team, victim_employee).await;

    assert!(
        matches!(result, Err(AppError::NotFound(_))),
        "cross-company removal must fail, got {result:?}"
    );

    // And the membership must actually still be there — a failing status code
    // would be cold comfort if the DELETE had already run.
    let members = team_service::list_members(&pool, victim_company, victim_team)
        .await
        .expect("owner can still read the roster");
    assert_eq!(
        members.len(),
        1,
        "the member must survive the rejected call"
    );
}

#[tokio::test]
async fn add_member_rejects_a_team_in_another_company() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (victim_company, victim_team, _) = seed_team_with_member(&pool).await;
    let attacker_company = seed_company(&pool).await;
    // An employee of the *victim* company: the DB's
    // `team_members_same_company_trigger` already blocks linking across
    // companies, so using an attacker-owned employee would pass for the wrong
    // reason. This proves the application scope, not the trigger.
    let victim_employee = seed_employee(&pool, victim_company, None, 400_000).await;

    let result = team_service::add_member(
        &pool,
        attacker_company,
        victim_team,
        victim_employee,
        "member",
    )
    .await;

    assert!(
        matches!(result, Err(AppError::NotFound(_))),
        "cross-company insertion must fail, got {result:?}"
    );

    let members = team_service::list_members(&pool, victim_company, victim_team)
        .await
        .expect("owner can still read the roster");
    assert_eq!(members.len(), 1, "no member may have been added");
}

#[tokio::test]
async fn owner_can_still_manage_its_own_team() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, team_id, employee_id) = seed_team_with_member(&pool).await;

    let members = team_service::list_members(&pool, company_id, team_id)
        .await
        .expect("list own members");
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].employee_id, employee_id);

    team_service::remove_member(&pool, company_id, team_id, employee_id)
        .await
        .expect("remove own member");

    let members = team_service::list_members(&pool, company_id, team_id)
        .await
        .expect("list own members after removal");
    assert!(members.is_empty());
}

#[tokio::test]
async fn an_invalid_member_role_is_a_bad_request_not_a_duplicate_conflict() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, team_id, _) = seed_team_with_member(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 300_000).await;

    // `team_members_role_check` allows only 'member' and 'lead'. Every
    // constraint violation used to be reported as "already a member of this
    // team", which tells an administrator nothing about the real problem.
    let result = team_service::add_member(&pool, company_id, team_id, employee_id, "captain").await;

    assert!(
        matches!(result, Err(AppError::BadRequest(_))),
        "an invalid role must report itself, got {result:?}"
    );
}

#[tokio::test]
async fn adding_the_same_employee_twice_is_a_conflict() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, team_id, employee_id) = seed_team_with_member(&pool).await;

    let result = team_service::add_member(&pool, company_id, team_id, employee_id, "lead").await;

    assert!(
        matches!(result, Err(AppError::Conflict(_))),
        "a genuine duplicate must still be a conflict, got {result:?}"
    );
}
