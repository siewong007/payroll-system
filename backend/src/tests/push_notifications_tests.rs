use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use chrono::NaiveDate;
use tower::ServiceExt;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::create_token;
use crate::core::config::AppConfig;
use crate::routes;
use crate::services::push_service::{self, SharedPushClient};
use crate::services::{
    notification_service, payroll_engine, payroll_lifecycle_service, session_service, user_service,
};
use crate::tests::route_auth_tests::{JWT_SECRET, TOTP_ENCRYPTION_KEY};
use crate::tests::support::{
    seed_company, seed_employee, seed_payroll_group, seed_user, skip_if_no_db,
};

const TEST_VAPID_PUB: &str =
    "BJvxi1WNriIrivViVml_qavLQzTjsf15Owr4ABMG6BkHOYT8-ACmAaf1dr2LhwGfasF3EfqzDL-tmRNRLvXg-vQ";
const TEST_VAPID_PRIV: &str = "8MV6yRoCfcI59xK9aXjf4Njyy9v6a13C4T7Eu_P6K8U";
const TEST_UA_PUB: &str =
    "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
const TEST_UA_AUTH: &str = "BTBZMqHH6r4Tts7J_aSIgg";

fn app_with_push(pool: sqlx::PgPool, push: SharedPushClient) -> axum::Router {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://test".to_string());
    let config = AppConfig {
        database_url,
        jwt_secret: JWT_SECRET.to_string(),
        totp_encryption_key: TOTP_ENCRYPTION_KEY.to_string(),
        jwt_expiry_hours: 1,
        server_host: "127.0.0.1".to_string(),
        server_port: 0,
        frontend_url: "http://localhost:5173".to_string(),
        api_public_url: String::new(),
        google_client_id: None,
        google_client_secret: None,
        webauthn_rp_id: "localhost".to_string(),
        webauthn_rp_origin: "http://localhost:5173".to_string(),
        smtp_host: None,
        smtp_port: None,
        smtp_username: None,
        smtp_password: None,
        smtp_from_email: None,
        smtp_from_name: None,
        trust_proxy_headers: true,
        client_ip_header: None,
        turnstile_secret_key: None,
        turnstile_verify_url: "https://challenges.cloudflare.com/turnstile/v0/siteverify".into(),
        vapid_public_key: None,
        vapid_private_key: None,
        vapid_subject: None,
    };
    let origin = url::Url::parse("http://localhost:5173").unwrap();
    let webauthn = std::sync::Arc::new(
        webauthn_rs::prelude::WebauthnBuilder::new("localhost", &origin)
            .unwrap()
            .rp_name("PayrollMY Test")
            .build()
            .unwrap(),
    );
    let state = AppState {
        pool,
        config: config.clone(),
        webauthn,
        mailer: crate::services::email_service::SharedMailer::disabled(),
        push,
    };
    routes::create_router(state).layer(axum::Extension(crate::core::auth::JwtSecret(
        config.jwt_secret,
    )))
}

async fn session_token_for(
    pool: &sqlx::PgPool,
    company_id: Uuid,
    role: &str,
) -> (String, Uuid, Uuid) {
    let user_id = seed_user(pool, company_id, role).await;
    let (session_id, _) = session_service::create_session(pool, user_id, None, None)
        .await
        .expect("create session");
    let token = create_token(
        user_id,
        "push-test@example.invalid",
        &[role.to_string()],
        Some(company_id),
        None,
        session_id,
        JWT_SECRET,
        1,
    )
    .expect("create jwt");
    (token, user_id, session_id)
}

fn authed_json(method: &str, uri: &str, token: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-forwarded-for", "203.0.113.10")
        .header(header::USER_AGENT, "PushTest/1.0")
        .body(Body::from(body.to_string()))
        .expect("build request")
}

#[tokio::test]
async fn push_config_and_subscription_lifecycle_and_device_transfer() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let (token_a, user_a, session_a) = session_token_for(&pool, company_id, "admin").await;
    let (token_b, user_b, _) = session_token_for(&pool, company_id, "employee").await;

    // 1. Disabled push client advertises enabled: false, vapid_public_key: null.
    let disabled_app = app_with_push(pool.clone(), SharedPushClient::disabled());
    let resp = disabled_app
        .oneshot(authed_json(
            "GET",
            "/api/notifications/push-config",
            &token_a,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["enabled"], false);
    assert!(json["vapid_public_key"].is_null());

    // 2. Enabled push client advertises public key and supports subscribe / transfer / delete.
    let push = SharedPushClient::for_test(
        TEST_VAPID_PUB,
        TEST_VAPID_PRIV,
        "mailto:ops@payrollmy.com",
        false,
    );
    let app = app_with_push(pool.clone(), push);

    let cfg_resp = app
        .clone()
        .oneshot(authed_json(
            "GET",
            "/api/notifications/push-config",
            &token_a,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(cfg_resp.status(), StatusCode::OK);
    let cfg_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(cfg_resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(cfg_json["enabled"], true);
    assert_eq!(cfg_json["vapid_public_key"], TEST_VAPID_PUB);

    let endpoint = format!("https://fcm.googleapis.com/fcm/send/{}", Uuid::new_v4());
    let sub_body = serde_json::json!({
        "endpoint": endpoint,
        "keys": {
            "p256dh": TEST_UA_PUB,
            "auth": TEST_UA_AUTH
        }
    })
    .to_string();

    let sub_a = app
        .clone()
        .oneshot(authed_json(
            "POST",
            "/api/notifications/subscriptions",
            &token_a,
            &sub_body,
        ))
        .await
        .unwrap();
    assert_eq!(sub_a.status(), StatusCode::OK);

    // Same browser endpoint re-subscribed by User B on a shared device transfers ownership cleanly.
    let sub_b = app
        .clone()
        .oneshot(authed_json(
            "POST",
            "/api/notifications/subscriptions",
            &token_b,
            &sub_body,
        ))
        .await
        .unwrap();
    assert_eq!(sub_b.status(), StatusCode::OK);

    let active_a: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM push_subscriptions WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    let active_b: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM push_subscriptions WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active_a, 0);
    assert_eq!(active_b, 1);

    // User A re-subscribes, then revoking User A's session revokes their push subscription.
    let _ = app
        .clone()
        .oneshot(authed_json(
            "POST",
            "/api/notifications/subscriptions",
            &token_a,
            &sub_body,
        ))
        .await
        .unwrap();
    session_service::revoke_session(&pool, user_a, session_a)
        .await
        .unwrap();

    let after_revoke: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM push_subscriptions WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after_revoke, 0);

    // Explicit DELETE revokes subscription.
    let _ = app
        .clone()
        .oneshot(authed_json(
            "POST",
            "/api/notifications/subscriptions",
            &token_b,
            &sub_body,
        ))
        .await
        .unwrap();
    let del_b = app
        .clone()
        .oneshot(authed_json(
            "DELETE",
            "/api/notifications/subscriptions",
            &token_b,
            &serde_json::json!({ "endpoint": endpoint }).to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(del_b.status(), StatusCode::OK);
    let del_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(del_b.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(del_json["subscribed"], false);
}

#[tokio::test]
async fn notifications_and_payroll_paid_enqueue_push_outbox_and_deliver_via_worker() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let emp_id = seed_employee(&pool, company_id, Some(group_id), 450_000).await;
    let emp_user_id = seed_user(&pool, company_id, "employee").await;
    sqlx::query("UPDATE users SET employee_id = $1 WHERE id = $2")
        .bind(emp_id)
        .bind(emp_user_id)
        .execute(&pool)
        .await
        .unwrap();
    crate::repositories::user_companies::insert(&pool, emp_user_id, company_id)
        .await
        .unwrap();

    let (emp_session_id, _) = session_service::create_session(&pool, emp_user_id, None, None)
        .await
        .unwrap();

    // Spin up local mock push server.
    let delivered = std::sync::Arc::new(std::sync::Mutex::new(0usize));
    let delivered_clone = delivered.clone();
    let mock_router = axum::Router::new().route(
        "/push/emp",
        axum::routing::post(move || {
            let count = delivered_clone.clone();
            async move {
                *count.lock().unwrap() += 1;
                StatusCode::CREATED
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, mock_router).await.unwrap();
    });

    let endpoint = format!("http://{addr}/push/emp");
    crate::repositories::push_subscriptions::upsert(
        &pool,
        emp_user_id,
        Some(company_id),
        Some(emp_session_id),
        &endpoint,
        TEST_UA_PUB,
        TEST_UA_AUTH,
        Some("MockTest/1.0"),
    )
    .await
    .unwrap();

    // Trigger create_notification -> enqueues into push_outbox.
    notification_service::create_notification(
        &pool,
        emp_user_id,
        company_id,
        "leave_approved",
        "Leave Approved",
        "Your leave request was approved.",
        Some("leave_request"),
        None,
    )
    .await
    .unwrap();

    // Trigger payroll_lifecycle_service::lock_as_paid -> notifies employee & enqueues push_outbox.
    let payroll_admin = seed_user(&pool, company_id, "payroll_admin").await;
    let finance = seed_user(&pool, company_id, "finance").await;
    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        5,
        NaiveDate::from_ymd_opt(2024, 6, 5).unwrap(),
        payroll_admin,
        None,
        None,
    )
    .await
    .unwrap();
    payroll_lifecycle_service::submit_for_approval(&pool, company_id, run.id, payroll_admin, None)
        .await
        .unwrap();
    let fin_auth = crate::core::auth::AuthUser(
        crate::core::auth::Claims {
            sub: finance,
            email: "fin@example.invalid".into(),
            roles: vec!["finance".into()],
            company_id: Some(company_id),
            employee_id: None,
            sid: Uuid::new_v4(),
            exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp(),
            iat: chrono::Utc::now().timestamp(),
            iss: crate::core::auth::JWT_ISSUER.into(),
            aud: crate::core::auth::JWT_AUDIENCE.into(),
        },
        Vec::new(),
    );
    payroll_lifecycle_service::approve(&pool, company_id, run.id, &fin_auth, None)
        .await
        .unwrap();
    payroll_lifecycle_service::lock_as_paid(&pool, company_id, run.id, finance, None)
        .await
        .unwrap();

    let queued_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM push_outbox WHERE user_id = $1 AND status = 'pending'",
    )
    .bind(emp_user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(queued_count, 2);

    let push_client = SharedPushClient::for_test(
        TEST_VAPID_PUB,
        TEST_VAPID_PRIV,
        "mailto:ops@payrollmy.com",
        true,
    );
    let (sent, failed) = push_service::process_outbox_batch(&pool, &push_client, 20)
        .await
        .unwrap();
    assert!(sent >= 2);
    assert_eq!(failed, 0);
    assert_eq!(*delivered.lock().unwrap(), 2);

    let emp_sent: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM push_outbox WHERE user_id = $1 AND status = 'sent'",
    )
    .bind(emp_user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(emp_sent, 2);

    // Direct test push also succeeds, and deactivating user revokes subscription.
    let test_res =
        push_service::send_test_push(&pool, &push_client, emp_user_id, "/portal/notifications")
            .await
            .unwrap();
    assert_eq!(test_res.sent, 1);

    let admin_actor = seed_user(&pool, company_id, "super_admin").await;
    user_service::update_user(
        &pool,
        emp_user_id,
        crate::models::user_company::UpdateUserRequest {
            full_name: None,
            email: None,
            roles: None,
            is_active: Some(false),
            company_ids: None,
        },
        admin_actor,
        None,
    )
    .await
    .unwrap();

    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM push_subscriptions WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(emp_user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0);

    sqlx::query("DELETE FROM push_subscriptions WHERE user_id = $1")
        .bind(emp_user_id)
        .execute(&pool)
        .await
        .unwrap();

    server.abort();
}
