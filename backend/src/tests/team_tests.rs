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

/// A company with one team holding one employee, plus the administrator that
/// created it — audited writes need a real `user_id` for the foreign key.
struct TeamFixture {
    company_id: Uuid,
    team_id: Uuid,
    employee_id: Uuid,
    actor: Uuid,
}

async fn seed_team_with_member(pool: &sqlx::PgPool) -> TeamFixture {
    let company_id = seed_company(pool).await;
    let actor = seed_user(pool, company_id, "admin").await;
    let employee_id = seed_employee(pool, company_id, None, 500_000).await;

    let team = team_service::create_team(
        pool,
        company_id,
        &format!("Team-{}", &Uuid::new_v4().to_string()[..8]),
        None,
        "general",
        actor,
        None,
    )
    .await
    .expect("create team");

    team_service::add_member(
        pool,
        company_id,
        team.id,
        employee_id,
        "member",
        actor,
        None,
    )
    .await
    .expect("seed member");

    TeamFixture {
        company_id,
        team_id: team.id,
        employee_id,
        actor,
    }
}

#[tokio::test]
async fn list_members_rejects_a_team_in_another_company() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let victim = seed_team_with_member(&pool).await;
    let attacker_company = seed_company(&pool).await;

    let result = team_service::list_members(&pool, attacker_company, victim.team_id).await;

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
    let victim = seed_team_with_member(&pool).await;
    let attacker_company = seed_company(&pool).await;
    let attacker = seed_user(&pool, attacker_company, "admin").await;

    let result = team_service::remove_member(
        &pool,
        attacker_company,
        victim.team_id,
        victim.employee_id,
        attacker,
        None,
    )
    .await;

    assert!(
        matches!(result, Err(AppError::NotFound(_))),
        "cross-company removal must fail, got {result:?}"
    );

    // And the membership must actually still be there — a failing status code
    // would be cold comfort if the DELETE had already run.
    let members = team_service::list_members(&pool, victim.company_id, victim.team_id)
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
    let victim = seed_team_with_member(&pool).await;
    let attacker_company = seed_company(&pool).await;
    let attacker = seed_user(&pool, attacker_company, "admin").await;
    // An employee of the *victim* company: the DB's
    // `team_members_same_company_trigger` already blocks linking across
    // companies, so using an attacker-owned employee would pass for the wrong
    // reason. This proves the application scope, not the trigger.
    let victim_employee = seed_employee(&pool, victim.company_id, None, 400_000).await;

    let result = team_service::add_member(
        &pool,
        attacker_company,
        victim.team_id,
        victim_employee,
        "member",
        attacker,
        None,
    )
    .await;

    assert!(
        matches!(result, Err(AppError::NotFound(_))),
        "cross-company insertion must fail, got {result:?}"
    );

    let members = team_service::list_members(&pool, victim.company_id, victim.team_id)
        .await
        .expect("owner can still read the roster");
    assert_eq!(members.len(), 1, "no member may have been added");
}

#[tokio::test]
async fn owner_can_still_manage_its_own_team() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let fixture = seed_team_with_member(&pool).await;

    let members = team_service::list_members(&pool, fixture.company_id, fixture.team_id)
        .await
        .expect("list own members");
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].employee_id, fixture.employee_id);

    team_service::remove_member(
        &pool,
        fixture.company_id,
        fixture.team_id,
        fixture.employee_id,
        fixture.actor,
        None,
    )
    .await
    .expect("remove own member");

    let members = team_service::list_members(&pool, fixture.company_id, fixture.team_id)
        .await
        .expect("list own members after removal");
    assert!(members.is_empty());
}

#[tokio::test]
async fn an_invalid_member_role_is_a_bad_request_not_a_duplicate_conflict() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let fixture = seed_team_with_member(&pool).await;
    let employee_id = seed_employee(&pool, fixture.company_id, None, 300_000).await;

    // `team_members_role_check` allows only 'member' and 'lead'. Every
    // constraint violation used to be reported as "already a member of this
    // team", which tells an administrator nothing about the real problem.
    let result = team_service::add_member(
        &pool,
        fixture.company_id,
        fixture.team_id,
        employee_id,
        "captain",
        fixture.actor,
        None,
    )
    .await;

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
    let fixture = seed_team_with_member(&pool).await;

    let result = team_service::add_member(
        &pool,
        fixture.company_id,
        fixture.team_id,
        fixture.employee_id,
        "lead",
        fixture.actor,
        None,
    )
    .await;

    assert!(
        matches!(result, Err(AppError::Conflict(_))),
        "a genuine duplicate must still be a conflict, got {result:?}"
    );
}

/// Team and membership changes must reach the audit trail. Closing the
/// cross-tenant hole is only half the fix: without a record, a roster edit —
/// whether made before the fix or by a legitimately authorized user — leaves
/// nothing to review.
#[tokio::test]
async fn team_and_membership_changes_are_audited() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let fixture = seed_team_with_member(&pool).await;

    async fn count(pool: &sqlx::PgPool, company_id: Uuid, entity: &str, action: &str) -> i64 {
        let (n,): (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM audit_logs
               WHERE company_id = $1 AND entity_type = $2 AND action = $3"#,
        )
        .bind(company_id)
        .bind(entity)
        .bind(action)
        .fetch_one(pool)
        .await
        .expect("count audit rows");
        n
    }

    assert_eq!(
        count(&pool, fixture.company_id, "team", "create").await,
        1,
        "creating a team must write an audit row"
    );
    assert_eq!(
        count(&pool, fixture.company_id, "team_member", "create").await,
        1,
        "adding a member must write an audit row"
    );

    team_service::remove_member(
        &pool,
        fixture.company_id,
        fixture.team_id,
        fixture.employee_id,
        fixture.actor,
        None,
    )
    .await
    .expect("remove member");

    assert_eq!(
        count(&pool, fixture.company_id, "team_member", "delete").await,
        1,
        "removing a member must write an audit row"
    );
}
