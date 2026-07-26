//! Integration coverage for admin user administration.
//!
//! Every test is gated on a reachable database via `skip_if_no_db()`. The themes
//! are the invariants that the rework established: nothing is written until every
//! rule passes, each mutation is atomic, and any change to a user's authority
//! ends their live sessions.

use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::core::error::AppError;
use crate::models::user_company::{CreateUserRequest, UpdateUserRequest};
use crate::repositories::{refresh_tokens, user_companies, user_sessions, users};
use crate::services::user_service;
use crate::tests::support::{seed_company, seed_employee, seed_user, skip_if_no_db};

const VALID_PASSWORD: &str = "Str0ngPassword";

fn unique_email(tag: &str) -> String {
    format!("{tag}-{}@example.invalid", Uuid::new_v4())
}

fn roles(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn create_request(email: &str, role: &str, company_ids: Vec<Uuid>) -> CreateUserRequest {
    CreateUserRequest {
        email: email.to_string(),
        password: VALID_PASSWORD.to_string(),
        full_name: "Test Account".into(),
        roles: roles(&[role]),
        company_ids,
    }
}

fn blank_update() -> UpdateUserRequest {
    UpdateUserRequest {
        full_name: None,
        email: None,
        roles: None,
        is_active: None,
        company_ids: None,
    }
}

/// Give a user one live session plus a refresh token, so revocation is observable.
async fn seed_session(pool: &sqlx::PgPool, user_id: Uuid) -> (Uuid, String) {
    let session_id = Uuid::now_v7();
    let token_hash = format!("hash-{}", Uuid::new_v4());
    user_sessions::insert(
        pool,
        session_id,
        user_id,
        None,
        Utc::now() + Duration::days(1),
    )
    .await
    .expect("insert session");
    refresh_tokens::insert(
        pool,
        user_id,
        session_id,
        &token_hash,
        Utc::now() + Duration::days(1),
    )
    .await
    .expect("insert refresh token");
    (session_id, token_hash)
}

async fn session_is_live(pool: &sqlx::PgPool, user_id: Uuid, session_id: Uuid) -> bool {
    user_sessions::is_active(pool, user_id, session_id)
        .await
        .expect("read session state")
}

async fn refresh_is_live(pool: &sqlx::PgPool, token_hash: &str) -> bool {
    refresh_tokens::find_active(pool, token_hash)
        .await
        .expect("read refresh token state")
        .is_some()
}

async fn company_ids_of(pool: &sqlx::PgPool, user_id: Uuid) -> Vec<Uuid> {
    let mut ids: Vec<Uuid> = user_service::get_user_companies(pool, user_id)
        .await
        .expect("read companies")
        .into_iter()
        .map(|company| company.id)
        .collect();
    ids.sort();
    ids
}

// ─── Listing & visibility ───

#[tokio::test]
async fn admin_user_list_includes_employee_only_accounts_in_shared_company() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let admin_id = seed_user(&pool, company_id, "admin").await;
    user_companies::insert(&pool, admin_id, company_id)
        .await
        .expect("link admin company");

    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let employee_user_id = Uuid::now_v7();
    let email = unique_email("employee-list");
    users::insert_employee_user(
        &pool,
        employee_user_id,
        &email,
        "unused-test-hash",
        "Employee Account",
        company_id,
        employee_id,
    )
    .await
    .expect("insert employee user");
    user_companies::insert(&pool, employee_user_id, company_id)
        .await
        .expect("link employee company");

    let (users_page, _) = user_service::list_users(&pool, false, admin_id, None, None, 100, 0)
        .await
        .expect("list users for admin");
    let employee = users_page
        .iter()
        .find(|user| user.id == employee_user_id)
        .expect("employee account should be visible to company admin");

    assert_eq!(employee.roles, ["employee"]);
    assert_eq!(employee.employee_id, Some(employee_id));
    assert!(
        employee
            .companies
            .iter()
            .any(|company| company.id == company_id)
    );
}

#[tokio::test]
async fn admin_list_does_not_leak_companies_outside_the_callers_tenant() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let shared_company = seed_company(&pool).await;
    let foreign_company = seed_company(&pool).await;

    let admin_id = seed_user(&pool, shared_company, "admin").await;
    user_companies::insert(&pool, admin_id, shared_company)
        .await
        .expect("link admin");

    // A user who belongs to both the shared company and a company the caller
    // has no part in.
    let multi_tenant_id = seed_user(&pool, shared_company, "payroll_admin").await;
    user_companies::insert(&pool, multi_tenant_id, shared_company)
        .await
        .expect("link shared");
    user_companies::insert(&pool, multi_tenant_id, foreign_company)
        .await
        .expect("link foreign");

    let (page, _) = user_service::list_users(&pool, false, admin_id, None, None, 100, 0)
        .await
        .expect("list users for admin");
    let seen = page
        .iter()
        .find(|user| user.id == multi_tenant_id)
        .expect("shared user visible");

    assert!(
        seen.companies.iter().all(|c| c.id != foreign_company),
        "a company admin must not learn the names of tenants they do not belong to"
    );
    assert!(seen.companies.iter().any(|c| c.id == shared_company));
}

#[tokio::test]
async fn list_users_paginates_and_reports_the_total() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_id, "super_admin").await;
    for _ in 0..3 {
        let id = seed_user(&pool, company_id, "finance").await;
        user_companies::insert(&pool, id, company_id)
            .await
            .expect("link");
    }

    let (page, total) =
        user_service::list_users(&pool, true, super_admin_id, Some(company_id), None, 2, 0)
            .await
            .expect("list page");

    assert_eq!(page.len(), 2, "limit must bound the returned rows");
    assert_eq!(total, 3, "total counts every match, not just the page");
}

#[tokio::test]
async fn list_users_search_matches_name_and_email() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_id, "super_admin").await;
    let email = unique_email("searchable-target");
    let created = user_service::create_user(
        &pool,
        create_request(&email, "finance", vec![company_id]),
        super_admin_id,
        None,
    )
    .await
    .expect("create user");

    let (hits, total) = user_service::list_users(
        &pool,
        true,
        super_admin_id,
        Some(company_id),
        Some("searchable-target"),
        50,
        0,
    )
    .await
    .expect("search users");

    assert_eq!(total, 1);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, created.id);
}

// ─── Creation ───

#[tokio::test]
async fn create_user_normalizes_email_and_forces_a_password_change() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_id, "super_admin").await;

    let raw = format!("  MiXeD-{}@Example.COM  ", Uuid::new_v4());
    let created = user_service::create_user(
        &pool,
        create_request(&raw, "hr_manager", vec![company_id]),
        super_admin_id,
        None,
    )
    .await
    .expect("create user");

    assert_eq!(created.email, raw.trim().to_lowercase());

    let stored = users::get_by_id(&pool, created.id)
        .await
        .expect("load user")
        .expect("user exists");
    assert!(
        stored.must_change_password,
        "an admin chose this password, so the account holder must replace it"
    );
}

#[tokio::test]
async fn create_user_rejects_the_employee_role() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_id, "super_admin").await;

    let result = user_service::create_user(
        &pool,
        create_request(&unique_email("portal"), "employee", vec![company_id]),
        super_admin_id,
        None,
    )
    .await;

    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[tokio::test]
async fn create_user_rolls_back_entirely_when_a_company_link_fails() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_id, "super_admin").await;
    let email = unique_email("rollback");

    let result = user_service::create_user(
        &pool,
        CreateUserRequest {
            roles: roles(&["admin"]),
            company_ids: vec![company_id, Uuid::now_v7()], // second company does not exist
            ..create_request(&email, "admin", vec![])
        },
        super_admin_id,
        None,
    )
    .await;

    assert!(
        matches!(result, Err(AppError::BadRequest(_))),
        "a dangling company id is a client error, not a 500"
    );
    assert!(
        users::find_id_by_email(&pool, &email)
            .await
            .expect("lookup")
            .is_none(),
        "the users row must roll back with the failed link, leaving the email reusable"
    );
}

#[tokio::test]
async fn create_user_rejects_a_duplicate_email_as_a_conflict() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_id, "super_admin").await;
    let email = unique_email("duplicate");

    user_service::create_user(
        &pool,
        create_request(&email, "finance", vec![company_id]),
        super_admin_id,
        None,
    )
    .await
    .expect("first create succeeds");

    let second = user_service::create_user(
        &pool,
        create_request(&email.to_uppercase(), "finance", vec![company_id]),
        super_admin_id,
        None,
    )
    .await;

    assert!(
        matches!(second, Err(AppError::Conflict(_))),
        "uniqueness is enforced on the normalized email, regardless of case"
    );
}

// ─── Updates: validate before writing ───

#[tokio::test]
async fn update_user_rejects_exec_with_multiple_companies_without_writing_roles() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_a = seed_company(&pool).await;
    let company_b = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_a, "super_admin").await;
    let target = user_service::create_user(
        &pool,
        create_request(&unique_email("exec-guard"), "admin", vec![company_a]),
        super_admin_id,
        None,
    )
    .await
    .expect("create target");

    let result = user_service::update_user(
        &pool,
        target.id,
        UpdateUserRequest {
            roles: Some(roles(&["exec"])),
            company_ids: Some(vec![company_a, company_b]),
            ..blank_update()
        },
        super_admin_id,
        None,
    )
    .await;

    assert!(matches!(result, Err(AppError::BadRequest(_))));

    let after = users::get_projection_by_id(&pool, target.id)
        .await
        .expect("reload")
        .expect("still exists");
    assert_eq!(
        after.roles,
        ["admin"],
        "a rejected request must leave the roles column untouched"
    );
}

#[tokio::test]
async fn update_user_rejects_narrowing_to_exec_without_narrowing_companies() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_a = seed_company(&pool).await;
    let company_b = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_a, "super_admin").await;
    let target = user_service::create_user(
        &pool,
        create_request(&unique_email("narrow"), "admin", vec![company_a, company_b]),
        super_admin_id,
        None,
    )
    .await
    .expect("create multi-company admin");

    // No company_ids in the request at all: the rule must be evaluated against
    // the user's *existing* membership, not skipped.
    let result = user_service::update_user(
        &pool,
        target.id,
        UpdateUserRequest {
            roles: Some(roles(&["exec"])),
            ..blank_update()
        },
        super_admin_id,
        None,
    )
    .await;

    assert!(
        matches!(result, Err(AppError::BadRequest(_))),
        "an exec may hold exactly one company, so this narrowing must be refused"
    );
}

#[tokio::test]
async fn update_user_rejects_an_empty_company_set() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_id, "super_admin").await;
    let target = user_service::create_user(
        &pool,
        create_request(&unique_email("empty-set"), "finance", vec![company_id]),
        super_admin_id,
        None,
    )
    .await
    .expect("create target");

    let result = user_service::update_user(
        &pool,
        target.id,
        UpdateUserRequest {
            company_ids: Some(vec![]),
            ..blank_update()
        },
        super_admin_id,
        None,
    )
    .await;

    assert!(
        matches!(result, Err(AppError::BadRequest(_))),
        "an empty set must be rejected, not silently ignored"
    );
    assert_eq!(company_ids_of(&pool, target.id).await, vec![company_id]);
}

#[tokio::test]
async fn update_user_deduplicates_repeated_company_ids() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_a = seed_company(&pool).await;
    let company_b = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_a, "super_admin").await;
    let target = user_service::create_user(
        &pool,
        create_request(&unique_email("dedupe"), "admin", vec![company_a]),
        super_admin_id,
        None,
    )
    .await
    .expect("create target");

    let updated = user_service::update_user(
        &pool,
        target.id,
        UpdateUserRequest {
            company_ids: Some(vec![company_a, company_a, company_b]),
            ..blank_update()
        },
        super_admin_id,
        None,
    )
    .await
    .expect("duplicates are deduplicated, not a primary-key 500");

    let mut got: Vec<Uuid> = updated.companies.iter().map(|c| c.id).collect();
    got.sort();
    let mut want = vec![company_a, company_b];
    want.sort();
    assert_eq!(got, want);
}

#[tokio::test]
async fn update_user_rolls_back_when_a_company_link_fails() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_id, "super_admin").await;
    let target = user_service::create_user(
        &pool,
        create_request(&unique_email("tx-rollback"), "finance", vec![company_id]),
        super_admin_id,
        None,
    )
    .await
    .expect("create target");

    let result = user_service::update_user(
        &pool,
        target.id,
        UpdateUserRequest {
            roles: Some(roles(&["payroll_admin"])),
            company_ids: Some(vec![company_id, Uuid::now_v7()]),
            ..blank_update()
        },
        super_admin_id,
        None,
    )
    .await;

    assert!(matches!(result, Err(AppError::BadRequest(_))));

    let after = users::get_projection_by_id(&pool, target.id)
        .await
        .expect("reload")
        .expect("exists");
    assert_eq!(
        after.roles,
        ["finance"],
        "the role write must roll back with the failed link"
    );
    assert_eq!(company_ids_of(&pool, target.id).await, vec![company_id]);
}

// ─── Session revocation ───

#[tokio::test]
async fn a_name_only_edit_preserves_live_sessions() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_id, "super_admin").await;
    let target = user_service::create_user(
        &pool,
        create_request(&unique_email("rename"), "finance", vec![company_id]),
        super_admin_id,
        None,
    )
    .await
    .expect("create target");
    let (session_id, token_hash) = seed_session(&pool, target.id).await;

    user_service::update_user(
        &pool,
        target.id,
        UpdateUserRequest {
            full_name: Some("Renamed Person".into()),
            // Same membership, resubmitted — must not count as a change.
            company_ids: Some(vec![company_id]),
            ..blank_update()
        },
        super_admin_id,
        None,
    )
    .await
    .expect("rename succeeds");

    assert!(
        session_is_live(&pool, target.id, session_id).await,
        "changing only a display name must not sign the user out of every device"
    );
    assert!(refresh_is_live(&pool, &token_hash).await);
}

#[tokio::test]
async fn role_company_and_deactivation_changes_all_revoke_access() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_a = seed_company(&pool).await;
    let company_b = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_a, "super_admin").await;

    let cases = [
        (
            "role change",
            UpdateUserRequest {
                roles: Some(roles(&["payroll_admin"])),
                ..blank_update()
            },
        ),
        (
            "company change",
            UpdateUserRequest {
                company_ids: Some(vec![company_a, company_b]),
                ..blank_update()
            },
        ),
        (
            "deactivation",
            UpdateUserRequest {
                is_active: Some(false),
                ..blank_update()
            },
        ),
    ];

    for (label, change) in cases {
        let target = user_service::create_user(
            &pool,
            create_request(&unique_email("revoke"), "finance", vec![company_a]),
            super_admin_id,
            None,
        )
        .await
        .expect("create target");
        let (session_id, token_hash) = seed_session(&pool, target.id).await;

        user_service::update_user(&pool, target.id, change, super_admin_id, None)
            .await
            .unwrap_or_else(|e| panic!("{label} should succeed: {e:?}"));

        assert!(
            !session_is_live(&pool, target.id, session_id).await,
            "{label} must revoke live sessions"
        );
        assert!(
            !refresh_is_live(&pool, &token_hash).await,
            "{label} must revoke refresh tokens"
        );
    }
}

// ─── Self-lockout and last-super-admin guards ───

#[tokio::test]
async fn a_super_admin_cannot_deactivate_or_demote_their_own_account() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let actor = seed_user(&pool, company_id, "super_admin").await;
    user_companies::insert(&pool, actor, company_id)
        .await
        .expect("link actor");

    let deactivate = user_service::update_user(
        &pool,
        actor,
        UpdateUserRequest {
            is_active: Some(false),
            ..blank_update()
        },
        actor,
        None,
    )
    .await;
    assert!(matches!(deactivate, Err(AppError::BadRequest(_))));

    let demote = user_service::update_user(
        &pool,
        actor,
        UpdateUserRequest {
            roles: Some(roles(&["finance"])),
            ..blank_update()
        },
        actor,
        None,
    )
    .await;
    assert!(matches!(demote, Err(AppError::BadRequest(_))));

    let after = users::get_active_by_id(&pool, actor)
        .await
        .expect("reload actor");
    let after = after.expect("actor must still be able to sign in");
    assert!(after.roles.iter().any(|role| role == "super_admin"));
}

#[tokio::test]
async fn delete_user_rejects_self_deletion() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let actor = seed_user(&pool, company_id, "super_admin").await;

    let result = user_service::delete_user(&pool, actor, actor, None).await;

    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(
        users::get_active_by_id(&pool, actor)
            .await
            .expect("reload")
            .is_some()
    );
}

// ─── Deletion ───

#[tokio::test]
async fn deleted_user_is_hidden_and_cannot_be_restored_by_a_matching_email() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_id, "super_admin").await;
    let user_id = seed_user(&pool, company_id, "employee").await;
    user_companies::insert(&pool, user_id, company_id)
        .await
        .expect("link employee company");

    user_service::delete_user(&pool, user_id, super_admin_id, None)
        .await
        .expect("soft delete user");

    assert!(
        users::get_active_by_id(&pool, user_id)
            .await
            .expect("load deleted user")
            .is_none(),
        "a deleted user must not authenticate"
    );
    let (page, _) =
        user_service::list_users(&pool, true, super_admin_id, Some(company_id), None, 100, 0)
            .await
            .expect("list filtered users");
    assert!(
        page.iter().all(|user| user.id != user_id),
        "a deleted user must not reappear in the company user list"
    );

    let deleted = users::find_by_email(
        &pool,
        &users::get_by_id(&pool, user_id)
            .await
            .expect("load deleted tombstone")
            .expect("tombstone exists")
            .email,
    )
    .await
    .expect("find deleted tombstone")
    .expect("deleted tombstone exists");
    assert!(deleted.is_deleted);
}

#[tokio::test]
async fn delete_user_revokes_sessions_as_well_as_refresh_tokens() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_id, "super_admin").await;
    let target = seed_user(&pool, company_id, "finance").await;
    user_companies::insert(&pool, target, company_id)
        .await
        .expect("link");
    let (session_id, token_hash) = seed_session(&pool, target).await;

    user_service::delete_user(&pool, target, super_admin_id, None)
        .await
        .expect("delete");

    assert!(
        !session_is_live(&pool, target, session_id).await,
        "a deleted user's access token must stop working immediately"
    );
    assert!(!refresh_is_live(&pool, &token_hash).await);
}

// ─── Company switching ───

#[tokio::test]
async fn switch_company_rejects_a_company_the_user_does_not_belong_to() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let home = seed_company(&pool).await;
    let foreign = seed_company(&pool).await;
    let user_id = seed_user(&pool, home, "admin").await;
    user_companies::insert(&pool, user_id, home)
        .await
        .expect("link home");

    let result = user_service::switch_company(&pool, user_id, foreign).await;

    assert!(matches!(result, Err(AppError::Forbidden(_))));
    let after = users::get_projection_by_id(&pool, user_id)
        .await
        .expect("reload")
        .expect("exists");
    assert_eq!(
        after.company_id,
        Some(home),
        "a refused switch must leave the active company alone"
    );
}

// ─── Audit trail ───

#[tokio::test]
async fn user_administration_writes_audit_rows() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_id, "super_admin").await;

    let created = user_service::create_user(
        &pool,
        create_request(&unique_email("audited"), "finance", vec![company_id]),
        super_admin_id,
        None,
    )
    .await
    .expect("create");

    user_service::update_user(
        &pool,
        created.id,
        UpdateUserRequest {
            full_name: Some("Audited Person".into()),
            ..blank_update()
        },
        super_admin_id,
        None,
    )
    .await
    .expect("update");

    user_service::delete_user(&pool, created.id, super_admin_id, None)
        .await
        .expect("delete");

    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_logs WHERE entity_type = 'user' AND entity_id = $1 ORDER BY created_at",
    )
    .bind(created.id)
    .fetch_all(&pool)
    .await
    .expect("read audit rows");

    for expected in ["create", "update", "delete"] {
        assert!(
            actions.iter().any(|action| action == expected),
            "expected a '{expected}' audit row, got {actions:?}"
        );
    }
}
