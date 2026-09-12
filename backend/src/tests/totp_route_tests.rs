//! End-to-end coverage for TOTP 2FA: enroll → confirm → password login is
//! gated → `/auth/2fa/verify` completes the session. Exercises the real
//! HTTP routes so a regression in the `auth_service::complete_login` chokepoint
//! (the single place 2FA is enforced across all login methods) would show up
//! here, not just in unit tests of the pieces.

use std::net::SocketAddr;

use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use totp_rs::{Algorithm, Secret, TOTP};
use tower::ServiceExt;
use uuid::Uuid;

use crate::core::auth::create_token;
use crate::core::crypto;
use crate::repositories::user_totp;
use crate::services::{session_service, totp_service};
use crate::tests::route_auth_tests::{JWT_SECRET, TOTP_ENCRYPTION_KEY, app_for};
use crate::tests::support::{seed_company, skip_if_no_db};

async fn seed_user_with_password(
    pool: &sqlx::PgPool,
    company_id: Uuid,
    email: &str,
    password: &str,
) -> Uuid {
    let user_id = Uuid::new_v4();
    // Low bcrypt cost — this only needs to be a real, verifiable hash, not
    // production-strength; keeps the test fast.
    let hash = bcrypt::hash(password, 4).expect("hash test password");
    sqlx::query(
        r#"INSERT INTO users (id, email, password_hash, full_name, roles, company_id)
           VALUES ($1, $2, $3, 'Totp Test User', ARRAY['admin']::VARCHAR(50)[], $4)"#,
    )
    .bind(user_id)
    .bind(email)
    .bind(&hash)
    .bind(company_id)
    .execute(pool)
    .await
    .expect("insert user");
    user_id
}

fn json_request(method: &str, uri: &str, token: Option<&str>, body: &str) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-forwarded-for", "203.0.113.10, 10.0.0.1")
        .header(header::USER_AGENT, "PayrollTotpTest/1.0");
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let mut req = builder
        .body(Body::from(body.to_string()))
        .expect("build request");
    // tower_governor's default PeerIpKeyExtractor reads this extension —
    // never populated when testing via `Router::oneshot` (no real listener).
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([203, 0, 113, 10], 12345))));
    req
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        panic!(
            "response body should be JSON: {e}; status={status}; raw={:?}",
            String::from_utf8_lossy(&bytes)
        )
    })
}

fn code_for_secret(secret_b32: &str) -> String {
    let bytes = Secret::Encoded(secret_b32.to_string())
        .to_bytes()
        .expect("valid base32 secret");
    let totp =
        TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes, None, "test".to_string()).expect("build totp");
    totp.generate_current().expect("generate code")
}

#[tokio::test]
async fn totp_setup_gates_login_until_code_is_verified() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let email = format!("totp-{}@example.invalid", Uuid::new_v4());
    let password = "Sup3rSecretPassw0rd";
    let user_id = seed_user_with_password(&pool, company_id, &email, password).await;
    let (session_id, _) = session_service::create_session(&pool, user_id, None, None)
        .await
        .expect("create test session");

    let auth_token = create_token(
        user_id,
        &email,
        &["admin".to_string()],
        Some(company_id),
        None,
        session_id,
        JWT_SECRET,
        1,
    )
    .expect("create jwt");

    let app = app_for(pool).await;

    // Enroll.
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/auth/2fa/setup/begin",
            Some(&auth_token),
            "",
        ))
        .await
        .expect("setup/begin response");
    assert_eq!(resp.status(), StatusCode::OK);
    let setup = body_json(resp).await;
    let secret = setup["secret"]
        .as_str()
        .expect("secret present")
        .to_string();
    assert!(
        setup["otpauth_url"]
            .as_str()
            .unwrap()
            .starts_with("otpauth://")
    );

    // Confirm with the first code — enables 2FA and returns backup codes.
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/auth/2fa/setup/confirm",
            Some(&auth_token),
            &format!(r#"{{"code":"{}"}}"#, code_for_secret(&secret)),
        ))
        .await
        .expect("setup/confirm response");
    assert_eq!(resp.status(), StatusCode::OK);
    let confirm = body_json(resp).await;
    let backup_codes = confirm["backup_codes"]
        .as_array()
        .expect("backup codes array");
    assert_eq!(backup_codes.len(), 10);

    // Password login now returns a pending-MFA marker, not a session.
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/auth/login",
            None,
            &format!(r#"{{"email":"{email}","password":"{password}"}}"#),
        ))
        .await
        .expect("login response");
    let login_status = resp.status();
    let login_body = body_json(resp).await;
    assert_eq!(login_status, StatusCode::OK, "login body: {login_body}");
    assert_eq!(login_body["requires_2fa"], serde_json::json!(true));
    let mfa_token = login_body["mfa_token"]
        .as_str()
        .expect("mfa_token present")
        .to_string();
    assert!(login_body.get("token").is_none());

    // A wrong code is rejected and does not grant a session.
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/auth/2fa/verify",
            None,
            &format!(r#"{{"mfa_token":"{mfa_token}","code":"000000"}}"#),
        ))
        .await
        .expect("verify (wrong code) response");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // The correct code completes the session.
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/auth/2fa/verify",
            None,
            &format!(
                r#"{{"mfa_token":"{mfa_token}","code":"{}"}}"#,
                code_for_secret(&secret)
            ),
        ))
        .await
        .expect("verify response");
    assert_eq!(resp.status(), StatusCode::OK);
    let verified = body_json(resp).await;
    assert!(verified["token"].as_str().is_some());
    assert_eq!(verified["user"]["email"], serde_json::json!(email));
}

// ─── Break-glass reset (super_admin) ───
//
// The scenario: an employee lost their authenticator (or a pre-v2 key bug
// encrypted their secret under a since-rotated JWT secret) and only a
// super_admin can restore access — without direct database access.

/// Enrolls a fresh `admin` user in 2FA through the real service functions.
async fn enroll_target(
    pool: &sqlx::PgPool,
    company_id: Uuid,
) -> (Uuid, String, String, Vec<String>) {
    let email = format!("totp-reset-{}@example.invalid", Uuid::new_v4());
    let password = "Sup3rSecretPassw0rd";
    let user_id = seed_user_with_password(pool, company_id, &email, password).await;
    let user = crate::services::auth_service::get_user_by_id(pool, user_id)
        .await
        .expect("load target user");
    let setup = totp_service::begin_setup(pool, &user, TOTP_ENCRYPTION_KEY)
        .await
        .expect("begin 2FA setup");
    let codes = totp_service::confirm_setup(
        pool,
        user_id,
        &code_for_secret(&setup.secret),
        TOTP_ENCRYPTION_KEY,
        None,
    )
    .await
    .expect("confirm 2FA setup");
    (user_id, email, password.to_string(), codes)
}

/// A super_admin whose stored hash is a real bcrypt hash of `password`, so the
/// handler's re-confirmation step can actually verify it. The shared
/// `seed_user` fixture writes a non-bcrypt placeholder, which no real account
/// would have.
async fn super_admin_with_password(
    pool: &sqlx::PgPool,
    company_id: Uuid,
    password: &str,
) -> (String, Uuid) {
    let (token, user_id) =
        crate::tests::route_auth_tests::token_and_user_for(pool, company_id, "super_admin").await;
    let hash = bcrypt::hash(password, 4).expect("hash test password");
    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(&hash)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("set caller password");
    (token, user_id)
}

#[tokio::test]
async fn break_glass_reset_unlocks_login_and_revokes_sessions() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (target_id, email, _, _) = enroll_target(&pool, company_id).await;

    // A live session the target holds when the reset lands must not survive it.
    let (session_id, _) = session_service::create_session(&pool, target_id, None, None)
        .await
        .expect("create target session");

    let (caller_token, _caller_id) =
        super_admin_with_password(&pool, company_id, "Sup3rSecretPassw0rd").await;
    let app = app_for(pool.clone()).await;

    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/api/admin/users/{target_id}/2fa/reset"),
            Some(&caller_token),
            r#"{"password":"Sup3rSecretPassw0rd"}"#,
        ))
        .await
        .expect("reset response");
    assert_eq!(resp.status(), StatusCode::OK);

    // Password login is no longer gated by a second factor.
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/auth/login",
            None,
            &format!(r#"{{"email":"{email}","password":"Sup3rSecretPassw0rd"}}"#),
        ))
        .await
        .expect("post-reset login response");
    assert_eq!(resp.status(), StatusCode::OK);
    let login = body_json(resp).await;
    assert!(
        login["token"].as_str().is_some(),
        "expected a session token after break-glass reset: {login}"
    );

    // The pre-existing session was revoked with the credential.
    let revoked_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT revoked_at FROM user_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(&pool)
            .await
            .expect("load target session");
    assert!(revoked_at.is_some(), "live session must be revoked");
}

#[tokio::test]
async fn break_glass_reset_is_super_admin_only_and_leaves_2fa_intact_when_denied() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (target_id, email, password, _) = enroll_target(&pool, company_id).await;
    let caller_token = crate::tests::route_auth_tests::token_for(&pool, company_id, "admin").await;

    let resp = app_for(pool.clone())
        .await
        .oneshot(json_request(
            "POST",
            &format!("/api/admin/users/{target_id}/2fa/reset"),
            Some(&caller_token),
            r#"{"password":"whatever"}"#,
        ))
        .await
        .expect("reset response");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // A denied reset must not have touched the enrolment: login still gates.
    let resp = app_for(pool)
        .await
        .oneshot(json_request(
            "POST",
            "/api/auth/login",
            None,
            &format!(r#"{{"email":"{email}","password":"{password}"}}"#),
        ))
        .await
        .expect("login response");
    let login = body_json(resp).await;
    assert_eq!(login["requires_2fa"], serde_json::json!(true));
}

#[tokio::test]
async fn break_glass_reset_requires_the_callers_real_password() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (target_id, _, _, _) = enroll_target(&pool, company_id).await;
    let (caller_token, _) =
        super_admin_with_password(&pool, company_id, "Sup3rSecretPassw0rd").await;

    let resp = app_for(pool)
        .await
        .oneshot(json_request(
            "POST",
            &format!("/api/admin/users/{target_id}/2fa/reset"),
            Some(&caller_token),
            r#"{"password":"NotTheCallersPassword"}"#,
        ))
        .await
        .expect("reset response");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn break_glass_reset_refuses_to_target_the_caller_themselves() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (caller_token, caller_id) =
        crate::tests::route_auth_tests::token_and_user_for(&pool, company_id, "super_admin").await;

    let resp = app_for(pool)
        .await
        .oneshot(json_request(
            "POST",
            &format!("/api/admin/users/{caller_id}/2fa/reset"),
            Some(&caller_token),
            r#"{"password":"Sup3rSecretPassw0rd"}"#,
        ))
        .await
        .expect("reset response");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn login_falls_back_to_backup_codes_when_the_totp_secret_is_unreadable() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (target_id, email, password, backup_codes) = enroll_target(&pool, company_id).await;

    // Simulate ciphertext nobody can open any more (e.g. written under a key
    // that no longer exists).
    sqlx::query(
        "UPDATE user_totp SET secret_encrypted = 'not-valid-ciphertext' WHERE user_id = $1",
    )
    .bind(target_id)
    .execute(&pool)
    .await
    .expect("corrupt stored secret");

    let app = app_for(pool).await;

    // Password login still hands out the pending-MFA marker…
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/auth/login",
            None,
            &format!(r#"{{"email":"{email}","password":"{password}"}}"#),
        ))
        .await
        .expect("login response");
    assert_eq!(resp.status(), StatusCode::OK);
    let login = body_json(resp).await;
    let mfa_token = login["mfa_token"]
        .as_str()
        .expect("mfa_token present")
        .to_string();

    // …and an unused backup code must still get in, even though the TOTP path
    // errored rather than merely rejecting the code.
    let resp = app
        .oneshot(json_request(
            "POST",
            "/api/auth/2fa/verify",
            None,
            &format!(
                r#"{{"mfa_token":"{mfa_token}","code":"{}"}}"#,
                backup_codes[0]
            ),
        ))
        .await
        .expect("verify response");
    assert_eq!(resp.status(), StatusCode::OK);
    let verified = body_json(resp).await;
    assert!(
        verified["token"].as_str().is_some(),
        "backup code must complete login when the TOTP secret is unreadable"
    );
}

// ─── Code sign-in (first-factor TOTP / backup code) ───
//
// `/auth/login/code` lets a user sign in with their authenticator code or a
// backup code instead of a password. The code is both factors in one, so the
// endpoint must issue a real session directly — routing it through
// `complete_login` would hand back a pending-MFA marker and a backup code
// already consumed by the first check would fail the second.

/// Enrolls like `enroll_target` but also returns the raw TOTP secret, so a
/// test can mint a *current* code to sign in with.
async fn enroll_and_get_secret(
    pool: &sqlx::PgPool,
    company_id: Uuid,
) -> (Uuid, String, String, Vec<String>) {
    let email = format!("totp-code-login-{}@example.invalid", Uuid::new_v4());
    let password = "Sup3rSecretPassw0rd";
    let user_id = seed_user_with_password(pool, company_id, &email, password).await;
    let user = crate::services::auth_service::get_user_by_id(pool, user_id)
        .await
        .expect("load target user");
    let setup = totp_service::begin_setup(pool, &user, TOTP_ENCRYPTION_KEY)
        .await
        .expect("begin 2FA setup");
    let codes = totp_service::confirm_setup(
        pool,
        user_id,
        &code_for_secret(&setup.secret),
        TOTP_ENCRYPTION_KEY,
        None,
    )
    .await
    .expect("confirm 2FA setup");
    (user_id, email, setup.secret, codes)
}

#[tokio::test]
async fn code_login_with_a_totp_code_issues_a_full_session() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (_user_id, email, secret, _codes) = enroll_and_get_secret(&pool, company_id).await;

    let resp = app_for(pool)
        .await
        .oneshot(json_request(
            "POST",
            "/api/auth/login/code",
            None,
            &format!(
                r#"{{"email":"{email}","code":"{}"}}"#,
                code_for_secret(&secret)
            ),
        ))
        .await
        .expect("code login response");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert!(
        body["token"].as_str().is_some(),
        "code login must issue a session, not a pending-MFA marker: {body}"
    );
    assert!(body.get("requires_2fa").is_none());
    assert_eq!(body["user"]["email"], serde_json::json!(email));
}

#[tokio::test]
async fn code_login_with_a_backup_code_issues_a_session_and_consumes_the_code() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (_user_id, email, _secret, backup_codes) = enroll_target(&pool, company_id).await;
    let app = app_for(pool).await;

    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/auth/login/code",
            None,
            &format!(r#"{{"email":"{email}","code":"{}"}}"#, backup_codes[0]),
        ))
        .await
        .expect("code login response");
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_json(resp).await["token"].as_str().is_some());

    // Backup codes are single-use: the same code must not sign in twice.
    let resp = app
        .oneshot(json_request(
            "POST",
            "/api/auth/login/code",
            None,
            &format!(r#"{{"email":"{email}","code":"{}"}}"#, backup_codes[0]),
        ))
        .await
        .expect("replayed backup code response");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn code_login_rejects_bad_code_unknown_email_and_no_2fa_identically() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (_id, email, _secret, _codes) = enroll_target(&pool, company_id).await;
    let no_2fa_email = format!("totp-none-{}@example.invalid", Uuid::new_v4());
    seed_user_with_password(&pool, company_id, &no_2fa_email, "Sup3rSecretPassw0rd").await;
    let app = app_for(pool).await;

    let mut bodies = Vec::new();
    for (probe_email, code) in [
        (email.as_str(), "000000"),
        ("nobody@example.invalid", "123456"),
        (no_2fa_email.as_str(), "123456"),
    ] {
        let resp = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/api/auth/login/code",
                None,
                &format!(r#"{{"email":"{probe_email}","code":"{code}"}}"#),
            ))
            .await
            .expect("code login response");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        bodies.push(body_json(resp).await);
    }
    // Wrong code, unknown email and "no 2FA on this account" must be
    // indistinguishable — the endpoint answers whether (email, code) is
    // valid, nothing else.
    assert_eq!(bodies[0], bodies[1]);
    assert_eq!(bodies[1], bodies[2]);
    assert_eq!(
        bodies[0]["error"],
        serde_json::json!("Invalid email or code")
    );
}

/// The startup migration: a row encrypted under the old JWT-derived key is
/// re-encrypted under `TOTP_ENCRYPTION_KEY`, and rows already under the
/// dedicated key are left byte-identical by a second pass.
#[tokio::test]
async fn startup_reencryption_migrates_legacy_rows_and_is_idempotent() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let email = format!("totp-rekey-{}@example.invalid", Uuid::new_v4());
    let user_id = seed_user_with_password(&pool, company_id, &email, "Sup3rSecretPassw0rd").await;

    // A row exactly as the pre-v2 writer would have left it.
    let secret_b32 = "JBSWY3DPEHPK3PXP";
    let legacy_encrypted =
        crypto::encrypt_with_legacy_jwt_derivation(secret_b32, JWT_SECRET).expect("legacy write");
    user_totp::upsert_pending(&pool, user_id, &legacy_encrypted)
        .await
        .expect("insert legacy row");

    totp_service::reencrypt_all(&pool, TOTP_ENCRYPTION_KEY, JWT_SECRET)
        .await
        .expect("re-encryption pass");

    let row = user_totp::find_by_user(&pool, user_id)
        .await
        .expect("find row")
        .expect("row exists");
    let decrypted = crypto::decrypt_secret(&row.secret_encrypted, TOTP_ENCRYPTION_KEY)
        .expect("decrypt under dedicated key");
    assert_eq!(decrypted, secret_b32);
    // The legacy derivation must no longer be able to read it either way.
    assert!(crypto::decrypt_with_legacy_jwt_derivation(&row.secret_encrypted, JWT_SECRET).is_err());

    // A second pass skips already-migrated rows — the ciphertext is unchanged.
    totp_service::reencrypt_all(&pool, TOTP_ENCRYPTION_KEY, JWT_SECRET)
        .await
        .expect("second pass");
    let row_after = user_totp::find_by_user(&pool, user_id)
        .await
        .expect("find row")
        .expect("row exists");
    assert_eq!(row.secret_encrypted, row_after.secret_encrypted);
}
