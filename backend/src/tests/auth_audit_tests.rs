//! Authentication events in the audit trail (plan item 18).
//!
//! Before this suite existed, 60+ audited actions covered business objects and
//! **zero** covered the credential lifecycle: a successful login, a brute-force
//! attempt, a password change or a 2FA toggle all left no row. These tests pin
//! the rows that close that gap, including the request metadata (right-most,
//! proxy-appended IP) that makes them usable as evidence.

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;
use uuid::Uuid;

use crate::models::audit::AuditRequestMeta;
use crate::services::password_reset_service;
use crate::tests::route_auth_tests::{JWT_SECRET, TOTP_ENCRYPTION_KEY, app_for};
use crate::tests::support::{seed_company, skip_if_no_db};

const XFF: &str = "203.0.113.10, 10.0.0.1";
/// The right-most entry — what the trusted proxy appended — is the only value
/// `AuditRequestMeta` records.
const EXPECTED_IP: &str = "10.0.0.1";

async fn seed_login_user(pool: &sqlx::PgPool, company_id: Uuid) -> (Uuid, String, String) {
    let user_id = Uuid::new_v4();
    let email = format!("auth-audit-{}@example.invalid", Uuid::new_v4());
    let password = "Sup3rSecretPassw0rd";
    let hash = bcrypt::hash(password, 4).expect("hash test password");
    sqlx::query(
        r#"INSERT INTO users (id, email, password_hash, full_name, roles, company_id)
           VALUES ($1, $2, $3, 'Auth Audit User', ARRAY['admin']::VARCHAR(50)[], $4)"#,
    )
    .bind(user_id)
    .bind(&email)
    .bind(&hash)
    .bind(company_id)
    .execute(pool)
    .await
    .expect("insert user");
    (user_id, email, password.to_string())
}

fn json_request(method: &str, uri: &str, token: Option<&str>, body: &str) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-forwarded-for", XFF)
        .header(header::USER_AGENT, "AuthAuditTest/1.0");
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let mut req = builder
        .body(Body::from(body.to_string()))
        .expect("build request");
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([203, 0, 113, 10], 12345))));
    req
}

async fn latest_auth_action(
    pool: &sqlx::PgPool,
    action: &str,
    company_id: Option<Uuid>,
) -> Option<(Option<String>, Option<serde_json::Value>)> {
    sqlx::query_as(
        "SELECT ip_address, new_values FROM audit_logs
         WHERE action = $1
           AND ($2::uuid IS NULL OR company_id = $2)
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(action)
    .bind(company_id)
    .fetch_optional(pool)
    .await
    .expect("query audit row")
}

#[tokio::test]
async fn failed_logins_are_recorded_against_the_account_and_against_nobody() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (_, email, _) = seed_login_user(&pool, company_id).await;
    let app = app_for(pool.clone()).await;

    // Wrong password for an existing account: recorded *against that account*.
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/auth/login",
            None,
            &format!(r#"{{"email":"{email}","password":"WrongPassword1"}}"#),
        ))
        .await
        .expect("login response");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let (ip, new_values) = latest_auth_action(&pool, "login_failed", Some(company_id))
        .await
        .expect("login_failed row for known user");
    assert_eq!(ip.as_deref(), Some(EXPECTED_IP));
    assert_eq!(new_values.unwrap()["reason"], "invalid_password");

    // Unknown address: still recorded, but against no user at all.
    let unknown = format!("nobody-{}@example.invalid", Uuid::new_v4());
    let resp = app
        .oneshot(json_request(
            "POST",
            "/api/auth/login",
            None,
            &format!(r#"{{"email":"{unknown}","password":"Whatever1x"}}"#),
        ))
        .await
        .expect("login response");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let (_, new_values) = latest_auth_action(&pool, "login_failed", None)
        .await
        .expect("login_failed row for unknown email");
    let values = new_values.expect("new_values");
    assert_eq!(values["reason"], "unknown_email");
    assert!(
        values["email"].as_str().is_some(),
        "the attempted address is the evidence"
    );
}

#[tokio::test]
async fn successful_password_login_records_its_method() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (_, email, password) = seed_login_user(&pool, company_id).await;

    let resp = app_for(pool.clone())
        .await
        .oneshot(json_request(
            "POST",
            "/api/auth/login",
            None,
            &format!(r#"{{"email":"{email}","password":"{password}"}}"#),
        ))
        .await
        .expect("login response");
    assert_eq!(resp.status(), StatusCode::OK);

    let (_, new_values) = latest_auth_action(&pool, "login", Some(company_id))
        .await
        .expect("login audit row");
    assert_eq!(new_values.expect("new_values")["method"], "password");
}

#[tokio::test]
async fn change_password_success_and_rejection_are_both_recorded() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (user_id, _, password) = seed_login_user(&pool, company_id).await;

    // Session for the authenticated call.
    let (session_id, _) = crate::services::session_service::create_session(&pool, user_id, None)
        .await
        .expect("create session");
    let token = crate::core::auth::create_token(
        user_id,
        "unused@example.invalid",
        &["admin".to_string()],
        Some(company_id),
        None,
        session_id,
        JWT_SECRET,
        1,
    )
    .expect("jwt");

    let app = app_for(pool.clone()).await;

    // Rejection first: wrong current password.
    let resp = app
        .clone()
        .oneshot(json_request(
            "PUT",
            "/api/auth/change-password",
            Some(&token),
            r#"{"current_password":"NotIt","new_password":"Br4ndNewPassw0rd"}"#,
        ))
        .await
        .expect("change-password response");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let (_, values) = latest_auth_action(&pool, "password_change_failed", Some(company_id))
        .await
        .expect("rejection row");
    assert_eq!(
        values.expect("values")["reason"],
        "invalid_current_password"
    );

    // Success: recorded inside the same transaction as the rotation itself.
    let resp = app
        .oneshot(json_request(
            "PUT",
            "/api/auth/change-password",
            Some(&token),
            &format!(r#"{{"current_password":"{password}","new_password":"Br4ndNewPassw0rd"}}"#),
        ))
        .await
        .expect("change-password response");
    assert_eq!(resp.status(), StatusCode::OK);
    let (_, values) = latest_auth_action(&pool, "password_changed", Some(company_id))
        .await
        .expect("success row");
    assert_eq!(
        values.expect("values")["sessions_revoked"],
        serde_json::json!(true)
    );
}

#[tokio::test]
async fn totp_enable_and_disable_are_recorded() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (user_id, _, _) = seed_login_user(&pool, company_id).await;
    let user = crate::services::auth_service::get_user_by_id(&pool, user_id)
        .await
        .expect("load user");

    let setup = crate::services::totp_service::begin_setup(&pool, &user, TOTP_ENCRYPTION_KEY)
        .await
        .expect("begin setup");
    crate::services::totp_service::confirm_setup(
        &pool,
        user_id,
        &totp_route_tests_support_code(&setup.secret),
        TOTP_ENCRYPTION_KEY,
        None,
    )
    .await
    .expect("confirm setup");

    let enabled = latest_auth_action(&pool, "totp_enabled", Some(company_id))
        .await
        .expect("totp_enabled row");
    assert!(enabled.1.is_some());

    crate::services::totp_service::disable(&pool, user_id, "Sup3rSecretPassw0rd", None)
        .await
        .expect("disable");
    assert!(
        latest_auth_action(&pool, "totp_disabled", Some(company_id))
            .await
            .is_some()
    );
}

#[tokio::test]
async fn password_reset_request_and_completion_are_recorded() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (_, email, _) = seed_login_user(&pool, company_id).await;
    let meta = AuditRequestMeta {
        ip_address: Some(EXPECTED_IP.into()),
        user_agent: Some("AuthAuditTest/1.0".into()),
    };

    let requested = password_reset_service::request_reset(&pool, &email, Some(&meta))
        .await
        .expect("request reset")
        .expect("user exists");
    let token = requested.2;

    let (_, values) = latest_auth_action(&pool, "password_reset_requested", Some(company_id))
        .await
        .expect("requested row");
    assert!(values.is_some());

    password_reset_service::reset_password(&pool, &token, "R3setPassw0rdX", Some(&meta))
        .await
        .expect("complete reset");

    let (_, values) = latest_auth_action(&pool, "password_reset_completed", Some(company_id))
        .await
        .expect("completed row");
    assert_eq!(
        values.expect("values")["sessions_revoked"],
        serde_json::json!(true)
    );
}

// Local helper so the test does not import from `totp_route_tests` (which is
// compiled under the same crate but keeps its helpers private).
fn totp_route_tests_support_code(secret_b32: &str) -> String {
    use totp_rs::{Algorithm, Secret, TOTP};
    let bytes = Secret::Encoded(secret_b32.to_string())
        .to_bytes()
        .expect("valid base32 secret");
    TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes, None, "test".to_string())
        .expect("build totp")
        .generate_current()
        .expect("generate code")
}
