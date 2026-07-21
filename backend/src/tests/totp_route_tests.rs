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
use crate::tests::route_auth_tests::{JWT_SECRET, app_for};
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

    let auth_token = create_token(
        user_id,
        &email,
        &["admin".to_string()],
        Some(company_id),
        None,
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
