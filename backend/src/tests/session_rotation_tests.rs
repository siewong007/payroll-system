//! Refresh-token rotation: atomicity, reuse detection, and the terminated-
//! employee gate that every session-minting path now shares.

use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::AppError;
use crate::models::session::LoginOutcome;
use crate::repositories::refresh_tokens;
use crate::services::{auth_service, session_service};
use crate::tests::support::{seed_company, seed_employee, seed_user, skip_if_no_db};

const JWT_SECRET: &str = "session-rotation-test-secret";

/// A company + admin user + one live session. Returns `(session_id, raw_token)`.
async fn seed_session(pool: &PgPool) -> (Uuid, String) {
    let company_id = seed_company(pool).await;
    let user_id = seed_user(pool, company_id, "admin").await;
    let session = session_service::create_session(pool, user_id, None).await;
    session.expect("create session")
}

/// A portal account bound to an employee — the shape the employee gate exists
/// for. `support::seed_user` deliberately leaves `employee_id` NULL.
async fn seed_portal_user(pool: &PgPool, company_id: Uuid, employee_id: Uuid) -> Uuid {
    let user_id = Uuid::new_v4();
    let email = format!("portal-{}@example.invalid", &user_id.to_string()[..8]);
    sqlx::query(
        r#"INSERT INTO users (id, email, password_hash, full_name, roles, company_id, employee_id)
           VALUES ($1, $2, 'x', 'Portal User', ARRAY['employee']::VARCHAR(50)[], $3, $4)"#,
    )
    .bind(user_id)
    .bind(&email)
    .bind(company_id)
    .bind(employee_id)
    .execute(pool)
    .await
    .expect("insert portal user");
    user_id
}

async fn live_tokens(pool: &PgPool, session_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM refresh_tokens WHERE session_id = $1 AND revoked = FALSE",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .expect("count live refresh tokens")
}

async fn session_is_revoked(pool: &PgPool, session_id: Uuid) -> bool {
    let revoked_at: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT revoked_at FROM user_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(pool)
            .await
            .expect("read session");
    revoked_at.is_some()
}

/// Two tabs refreshing the same cookie at once. The old SELECT-then-UPDATE let
/// both through, leaving a second live credential on the session that
/// `/auth/sessions` could neither display nor revoke.
#[tokio::test]
async fn concurrent_refresh_leaves_exactly_one_live_token() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (session_id, raw) = seed_session(&pool).await;

    let one = auth_service::refresh_session(&pool, &raw, JWT_SECRET, 1);
    let two = auth_service::refresh_session(&pool, &raw, JWT_SECRET, 1);
    let (first, second) = tokio::join!(one, two);

    let winners = usize::from(first.is_ok()) + usize::from(second.is_ok());
    assert_eq!(winners, 1, "exactly one racer may rotate the token");
    assert_eq!(live_tokens(&pool, session_id).await, 1);

    // The tab that lost must not have taken the session down with it.
    assert!(!session_is_revoked(&pool, session_id).await);
    let winner = first.or(second).expect("one refresh succeeded");
    auth_service::refresh_session(&pool, &winner.refresh_token, JWT_SECRET, 1)
        .await
        .expect("the winner's successor still works");
}

/// Replay of a token rotated away long enough ago that no honest client could
/// still hold it. This is the case the grace window must not swallow.
#[tokio::test]
async fn a_replayed_token_past_the_grace_window_revokes_the_family() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (session_id, raw) = seed_session(&pool).await;

    let rotated = auth_service::refresh_session(&pool, &raw, JWT_SECRET, 1)
        .await
        .expect("first rotation");

    // Age the successor past ROTATION_GRACE_SECONDS so the replay cannot be
    // mistaken for a second tab losing a race.
    sqlx::query(
        "UPDATE refresh_tokens SET created_at = NOW() - INTERVAL '1 minute' WHERE session_id = $1",
    )
    .bind(session_id)
    .execute(&pool)
    .await
    .expect("backdate the successor");

    let err = auth_service::refresh_session(&pool, &raw, JWT_SECRET, 1)
        .await
        .expect_err("a replayed token must fail");
    assert!(matches!(err, AppError::Unauthorized(_)));

    assert!(session_is_revoked(&pool, session_id).await);
    assert_eq!(live_tokens(&pool, session_id).await, 0);

    // The successor the honest client holds dies with the family — that is the
    // point: the user is signed out and re-authenticates, the thief cannot.
    let successor =
        auth_service::refresh_session(&pool, &rotated.refresh_token, JWT_SECRET, 1).await;
    assert!(successor.is_err());
}

/// A token this deployment never issued tells us nothing, so it must revoke
/// nothing. Otherwise anyone could sign any user out by guessing.
#[tokio::test]
async fn an_unknown_refresh_token_revokes_nothing() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (session_id, _raw) = seed_session(&pool).await;

    let err = auth_service::refresh_session(&pool, "rt_not_a_real_token", JWT_SECRET, 1)
        .await
        .expect_err("unknown token must fail");
    assert!(matches!(err, AppError::Unauthorized(_)));

    assert!(!session_is_revoked(&pool, session_id).await);
    assert_eq!(live_tokens(&pool, session_id).await, 1);
}

/// The revoke has to be usable as a guard, which means reporting how many rows
/// it actually retired. Without `AND revoked = FALSE` it always claimed one.
#[tokio::test]
async fn revoking_an_already_revoked_hash_reports_no_rows() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let user_id = seed_user(&pool, company_id, "admin").await;
    let (session_id, _raw) = session_service::create_session(&pool, user_id, None)
        .await
        .expect("create session");

    let token_hash = format!("test-hash-{}", Uuid::new_v4());
    refresh_tokens::insert(
        &pool,
        user_id,
        session_id,
        &token_hash,
        Utc::now() + Duration::days(1),
    )
    .await
    .expect("insert token");

    let first = refresh_tokens::revoke_by_hash(&pool, &token_hash).await;
    let second = refresh_tokens::revoke_by_hash(&pool, &token_hash).await;
    assert_eq!(first.expect("first revoke"), 1);
    assert_eq!(
        second.expect("second revoke"),
        0,
        "already-dead is not a hit"
    );
}

/// Deactivating an employee must end the session they are holding, not merely
/// refuse the next login — and it must take the whole family, not just the one
/// token presented.
#[tokio::test]
async fn a_deactivated_employee_loses_the_whole_session_on_refresh() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let user_id = seed_portal_user(&pool, company_id, employee_id).await;
    let (session_id, raw) = session_service::create_session(&pool, user_id, None)
        .await
        .expect("create session");

    sqlx::query("UPDATE employees SET is_active = FALSE WHERE id = $1")
        .bind(employee_id)
        .execute(&pool)
        .await
        .expect("deactivate employee");

    let err = auth_service::refresh_session(&pool, &raw, JWT_SECRET, 1)
        .await
        .expect_err("a deactivated employee must not refresh");
    match err {
        AppError::Unauthorized(msg) => assert!(msg.contains("employee account"), "{msg}"),
        other => panic!("expected Unauthorized, got {other:?}"),
    }

    assert!(session_is_revoked(&pool, session_id).await);
    assert_eq!(live_tokens(&pool, session_id).await, 0);
}

/// `complete_login` is the chokepoint for password, passkey and Google, and
/// `get_active_user` is what the 2FA second stage in `handlers/totp.rs` calls
/// directly. Both must refuse a deactivated employee, which is why the gate
/// sits in the loader rather than in `complete_login`.
#[tokio::test]
async fn a_deactivated_employee_cannot_mint_a_session_by_any_path() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let user_id = seed_portal_user(&pool, company_id, employee_id).await;

    sqlx::query("UPDATE employees SET is_active = FALSE WHERE id = $1")
        .bind(employee_id)
        .execute(&pool)
        .await
        .expect("deactivate employee");

    let completed = auth_service::complete_login(&pool, user_id, JWT_SECRET, 1, None)
        .await
        .expect_err("complete_login must refuse");
    match completed {
        AppError::Unauthorized(msg) => assert!(msg.contains("employee account"), "{msg}"),
        other => panic!("expected Unauthorized, got {other:?}"),
    }

    let loaded = auth_service::get_active_user(&pool, user_id)
        .await
        .expect_err("the 2FA second stage loads through here");
    assert!(matches!(loaded, AppError::Unauthorized(_)));
}

/// The gate must not touch accounts with no linked employee — every admin,
/// finance and super_admin login goes through the same loader.
#[tokio::test]
async fn an_account_with_no_linked_employee_still_logs_in() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let user_id = seed_user(&pool, company_id, "admin").await;

    let outcome = auth_service::complete_login(&pool, user_id, JWT_SECRET, 1, None)
        .await
        .expect("an admin has no employee row to gate on");
    assert!(matches!(outcome, LoginOutcome::Session(_)));
}
