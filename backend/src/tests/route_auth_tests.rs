use std::sync::Arc;

use axum::Extension;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use chrono::NaiveDate;
use tower::ServiceExt;
use url::Url;
use webauthn_rs::prelude::*;

use crate::core::app_state::AppState;
use crate::core::auth::{JwtSecret, create_token};
use crate::core::config::AppConfig;
use crate::routes;
use crate::services::payroll_engine;
use crate::services::session_service;
use crate::tests::support::{
    seed_company, seed_employee, seed_payroll_group, seed_user, skip_if_no_db,
};

pub(crate) const JWT_SECRET: &str = "route-auth-test-secret";
pub(crate) const TOTP_ENCRYPTION_KEY: &str = "route-auth-test-totp-key";

fn test_config(database_url: String) -> AppConfig {
    AppConfig {
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
        // Route tests drive the router via `oneshot`, which carries no
        // ConnectInfo; trusting the forwarded header the helper already sets
        // keeps the rate limiters able to extract a key.
        trust_proxy_headers: true,
        client_ip_header: None,
        turnstile_secret_key: None,
        turnstile_verify_url: "https://challenges.cloudflare.com/turnstile/v0/siteverify".into(),
        vapid_public_key: None,
        vapid_private_key: None,
        vapid_subject: None,
    }
}

fn test_webauthn() -> Arc<Webauthn> {
    let origin = Url::parse("http://localhost:5173").expect("valid origin");
    Arc::new(
        WebauthnBuilder::new("localhost", &origin)
            .expect("build webauthn")
            .rp_name("PayrollMY Test")
            .build()
            .expect("finish webauthn"),
    )
}

fn request(method: &str, uri: &str, token: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        // Left entry is what the caller claimed, right entry is what the proxy
        // appended. `app_for` runs with `trust_proxy_headers: true`, so only
        // the right-most value may ever reach the audit trail.
        .header("x-forwarded-for", "198.51.100.99, 203.0.113.10")
        .header(header::USER_AGENT, "PayrollRouteTest/1.0")
        .body(Body::from(body.to_string()))
        .expect("build request")
}

pub(crate) async fn app_for(pool: sqlx::PgPool) -> axum::Router {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://test".to_string());
    let config = test_config(database_url);
    let state = AppState {
        pool,
        config: config.clone(),
        webauthn: test_webauthn(),
        mailer: crate::services::email_service::SharedMailer::disabled(),
        push: crate::services::push_service::SharedPushClient::disabled(),
    };

    routes::create_router(state).layer(Extension(JwtSecret(config.jwt_secret)))
}

pub(crate) async fn token_for(pool: &sqlx::PgPool, company_id: uuid::Uuid, role: &str) -> String {
    token_and_user_for(pool, company_id, role).await.0
}

pub(crate) async fn token_and_user_for(
    pool: &sqlx::PgPool,
    company_id: uuid::Uuid,
    role: &str,
) -> (String, uuid::Uuid) {
    let user_id = seed_user(pool, company_id, role).await;
    let (session_id, _) = session_service::create_session(pool, user_id, None, None)
        .await
        .expect("create test session");
    let token = create_token(
        user_id,
        "route-test@example.invalid",
        &[role.to_string()],
        Some(company_id),
        None,
        session_id,
        JWT_SECRET,
        1,
    )
    .expect("create jwt");
    (token, user_id)
}

#[tokio::test]
async fn non_admin_cannot_change_company_attendance_method() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "finance").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "PUT",
            "/api/attendance/company-method",
            &token,
            r#"{"method":"face_id"}"#,
        ))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn non_hr_admin_cannot_create_geofence_location() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "finance").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "POST",
            "/api/geofence/locations",
            &token,
            r#"{"name":"HQ","latitude":3.139,"longitude":101.6869,"radius_meters":200}"#,
        ))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn non_hr_admin_cannot_update_default_work_schedule() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "finance").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "PUT",
            "/api/work-schedules/default",
            &token,
            r#"{"name":"Default","start_time":"09:00","end_time":"18:00"}"#,
        ))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn audited_route_writes_request_metadata() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (token, user_id) = token_and_user_for(&pool, company_id, "admin").await;

    let response = app_for(pool.clone())
        .await
        .oneshot(request(
            "POST",
            "/api/geofence/locations",
            &token,
            r#"{"name":"HQ","latitude":3.139,"longitude":101.6869,"radius_meters":200}"#,
        ))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::OK);

    let row: (Option<String>, Option<String>) = sqlx::query_as(
        r#"SELECT ip_address, user_agent
           FROM audit_logs
           WHERE user_id = $1 AND entity_type = 'company_location' AND action = 'create'
           ORDER BY created_at DESC
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("audit metadata row");

    // The proxy-appended entry, never the caller's claim of 198.51.100.99.
    // An audit trail that records a caller-chosen address is worse than one
    // with no address, because it reads as evidence.
    assert_eq!(row.0.as_deref(), Some("203.0.113.10"));
    assert_eq!(row.1.as_deref(), Some("PayrollRouteTest/1.0"));
}

#[tokio::test]
async fn self_service_employee_cannot_list_documents() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "employee").await;

    let response = app_for(pool)
        .await
        .oneshot(request("GET", "/api/documents", &token, ""))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn self_service_employee_cannot_list_letter_templates() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "employee").await;

    let response = app_for(pool)
        .await
        .oneshot(request("GET", "/api/email/templates", &token, ""))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn self_service_employee_cannot_send_letter() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "employee").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "POST",
            "/api/email/send",
            &token,
            r#"{"letter_type":"general","subject":"x","body_html":"<b>x</b>","recipient_email":"attacker@example.invalid"}"#,
        ))
        .await
        .expect("route response");

    // Guard rejects before any email is dispatched.
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn non_company_admin_cannot_update_company() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    // `finance` is a back-office role but not a company admin.
    let token = token_for(&pool, company_id, "finance").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "PUT",
            "/api/company",
            &token,
            r#"{"name":"Evil Co"}"#,
        ))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn non_company_admin_cannot_bulk_update_settings() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "finance").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "PUT",
            "/api/settings",
            &token,
            r#"{"settings":[]}"#,
        ))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn finance_can_approve_but_not_prepare_payroll_routes() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let _employee_id = seed_employee(&pool, company_id, Some(group_id), 350_000).await;
    let (_, processor_id) = token_and_user_for(&pool, company_id, "payroll_admin").await;
    let payroll_admin_token = token_for(&pool, company_id, "payroll_admin").await;
    let finance_token = token_for(&pool, company_id, "finance").await;

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        6,
        NaiveDate::from_ymd_opt(2024, 7, 5).unwrap(),
        processor_id,
        None,
        None,
    )
    .await
    .expect("process payroll");

    let finance_submit_response = app_for(pool.clone())
        .await
        .oneshot(request(
            "PUT",
            &format!("/api/payroll/runs/{}/submit-approval", run.id),
            &finance_token,
            "{}",
        ))
        .await
        .expect("finance submit response");
    assert_eq!(finance_submit_response.status(), StatusCode::FORBIDDEN);

    let payroll_admin_submit_response = app_for(pool.clone())
        .await
        .oneshot(request(
            "PUT",
            &format!("/api/payroll/runs/{}/submit-approval", run.id),
            &payroll_admin_token,
            "{}",
        ))
        .await
        .expect("payroll admin submit response");
    assert_eq!(payroll_admin_submit_response.status(), StatusCode::OK);

    let payroll_admin_approve_response = app_for(pool.clone())
        .await
        .oneshot(request(
            "PUT",
            &format!("/api/payroll/runs/{}/approve", run.id),
            &payroll_admin_token,
            "{}",
        ))
        .await
        .expect("payroll admin approve response");
    assert_eq!(
        payroll_admin_approve_response.status(),
        StatusCode::FORBIDDEN
    );

    let finance_approve_response = app_for(pool)
        .await
        .oneshot(request(
            "PUT",
            &format!("/api/payroll/runs/{}/approve", run.id),
            &finance_token,
            "{}",
        ))
        .await
        .expect("finance approve response");
    assert_eq!(finance_approve_response.status(), StatusCode::OK);
}

/// A self-service employee must not be able to read the company employee
/// directory — it exposes every colleague's IC, address and bank account.
#[tokio::test]
async fn self_service_employee_cannot_list_employees() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "employee").await;

    let response = app_for(pool)
        .await
        .oneshot(request("GET", "/api/employees", &token, ""))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// A self-service employee must not be able to delete an employee record.
/// `soft_delete_employee` also hard-deletes the linked user account.
#[tokio::test]
async fn self_service_employee_cannot_delete_employee() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let victim = seed_employee(&pool, company_id, None, 500_000).await;
    let token = token_for(&pool, company_id, "employee").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "DELETE",
            &format!("/api/employees/{}", victim),
            &token,
            "",
        ))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// A self-service employee must not be able to rewrite an employee's bank
/// account, which would redirect that employee's salary payment.
#[tokio::test]
async fn self_service_employee_cannot_change_bank_account() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let victim = seed_employee(&pool, company_id, None, 500_000).await;
    let token = token_for(&pool, company_id, "employee").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "PUT",
            &format!("/api/employees/{}", victim),
            &token,
            r#"{"bank_account_number":"999999999999"}"#,
        ))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// `exec` is read-mostly and must not create employees.
#[tokio::test]
async fn exec_cannot_create_employee() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "exec").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "POST",
            "/api/employees",
            &token,
            r#"{"employee_number":"E999","full_name":"Mallory","date_joined":"2024-01-01","basic_salary":0}"#,
        ))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// ─── Attendance route guards ───
//
// Every attendance guard lives inside its handler body, so only a wired-route
// test catches a handler that drops its gate.

/// A self-service employee must not read company-wide attendance — the list,
/// summary and export carry every colleague's movements and GPS coordinates.
#[tokio::test]
async fn self_service_employee_cannot_read_company_attendance() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "employee").await;
    let app = app_for(pool).await;

    for uri in [
        "/api/attendance/records",
        "/api/attendance/summary?date_from=2026-01-01&date_to=2026-01-31",
        "/api/attendance/export",
    ] {
        let response = app
            .clone()
            .oneshot(request("GET", uri, &token, ""))
            .await
            .expect("route response");
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "employee must be denied {uri}"
        );
    }
}

/// Manual entry and corrections rewrite payroll-feeding data: hr_admin only.
#[tokio::test]
async fn non_hr_admin_cannot_write_attendance_records() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let token = token_for(&pool, company_id, "finance").await;
    let app = app_for(pool).await;

    let manual = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/attendance/manual",
            &token,
            &format!(
                r#"{{"employee_id":"{employee_id}","check_in_at":"2026-06-01T01:00:00Z","status":"present"}}"#
            ),
        ))
        .await
        .expect("route response");
    assert_eq!(manual.status(), StatusCode::FORBIDDEN);

    let correction = app
        .clone()
        .oneshot(request(
            "PUT",
            &format!("/api/attendance/records/{}", uuid::Uuid::new_v4()),
            &token,
            r#"{"status":"present","reason":"test"}"#,
        ))
        .await
        .expect("route response");
    assert_eq!(correction.status(), StatusCode::FORBIDDEN);

    let backfill = app
        .oneshot(request(
            "POST",
            "/api/attendance/absent-run",
            &token,
            r#"{"date":"2026-06-01"}"#,
        ))
        .await
        .expect("route response");
    assert_eq!(backfill.status(), StatusCode::FORBIDDEN);
}

/// `exec` is read-mostly: it may view attendance, but generating a QR token
/// retires the console's live code, and kiosk credentials are admin-only.
#[tokio::test]
async fn exec_can_view_attendance_but_not_generate_qr_or_manage_kiosks() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "exec").await;
    let app = app_for(pool).await;

    let list = app
        .clone()
        .oneshot(request("GET", "/api/attendance/records", &token, ""))
        .await
        .expect("route response");
    assert_eq!(list.status(), StatusCode::OK);

    let qr = app
        .clone()
        .oneshot(request("POST", "/api/attendance/qr/generate", &token, "{}"))
        .await
        .expect("route response");
    assert_eq!(qr.status(), StatusCode::FORBIDDEN);

    let kiosks = app
        .oneshot(request("GET", "/api/attendance/kiosks", &token, ""))
        .await
        .expect("route response");
    assert_eq!(kiosks.status(), StatusCode::FORBIDDEN);
}

/// The public kiosk endpoint must reject a missing or garbage secret.
#[tokio::test]
async fn public_kiosk_endpoint_rejects_bad_secret() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let app = app_for(pool).await;

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/attendance/kiosk/qr")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-forwarded-for", "203.0.113.11")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("route response");
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let garbage = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/attendance/kiosk/qr")
                .header(header::AUTHORIZATION, "Kiosk not-a-real-secret")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-forwarded-for", "203.0.113.12")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("route response");
    assert_eq!(garbage.status(), StatusCode::UNAUTHORIZED);
}

/// A company `admin` is deliberately excluded from `ViewPayroll`, so it must not
/// be able to export a backup — the archive carries payroll_items, salary_history
/// and raw employee rows (bank account, IC, TIN).
#[tokio::test]
async fn company_admin_cannot_export_payroll_backup() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "admin").await;

    let response = app_for(pool)
        .await
        .oneshot(request("GET", "/api/admin/backup/export", &token, ""))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// User administration is `super_admin`-only. A company `admin` may read the
/// directory (to see who has access to their tenant) but must not create,
/// modify or remove accounts — those grant and revoke authority platform-wide.
#[tokio::test]
async fn company_admin_cannot_create_update_or_delete_users() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "admin").await;
    let target_id = seed_user(&pool, company_id, "finance").await;
    let app = app_for(pool).await;

    let create_body = format!(
        r#"{{"email":"blocked-{}@example.invalid","password":"Str0ngPassword","full_name":"Blocked","roles":["finance"],"company_ids":["{company_id}"]}}"#,
        uuid::Uuid::new_v4()
    );

    for (method, uri, body) in [
        ("POST", "/api/admin/users".to_string(), create_body.as_str()),
        (
            "PUT",
            format!("/api/admin/users/{target_id}"),
            r#"{"roles":["super_admin"]}"#,
        ),
        ("DELETE", format!("/api/admin/users/{target_id}"), ""),
    ] {
        let response = app
            .clone()
            .oneshot(request(method, &uri, &token, body))
            .await
            .expect("route response");
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "{method} {uri} must be refused for a company admin"
        );
    }
}

/// The directory carries every colleague's name, email and role, so the gate is
/// an explicit allow-list rather than "anyone who is not an employee".
#[tokio::test]
async fn user_directory_is_readable_by_admins_and_closed_to_everyone_else() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let app = app_for(pool.clone()).await;

    for allowed in ["super_admin", "admin"] {
        let token = token_for(&pool, company_id, allowed).await;
        let response = app
            .clone()
            .oneshot(request("GET", "/api/admin/users", &token, ""))
            .await
            .expect("route response");
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{allowed} must keep directory access"
        );
    }

    for denied in ["employee", "finance", "hr_manager", "payroll_admin", "exec"] {
        let token = token_for(&pool, company_id, denied).await;
        let response = app
            .clone()
            .oneshot(request("GET", "/api/admin/users", &token, ""))
            .await
            .expect("route response");
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "{denied} must not read the user directory"
        );
    }
}

/// `ValidatedJson` rejects a malformed email before the handler runs, so junk
/// never reaches the service or the database.
#[tokio::test]
async fn create_user_rejects_a_malformed_email_with_422() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "super_admin").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "POST",
            "/api/admin/users",
            &token,
            &format!(
                r#"{{"email":"not-an-email","password":"Str0ngPassword","full_name":"Junk","roles":["finance"],"company_ids":["{company_id}"]}}"#
            ),
        ))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ─── Uploaded file downloads ───
//
// `GET /api/uploads/{filename}` served any caller that held the URL, so a
// forwarded link or a proxy log entry was enough to read another tenant's claim
// receipt. Access is now decided by the record that references the file; these
// pin every branch of that rule, including the allow path, because a fix that
// over-blocks silently breaks every attachment in the product.

/// Mints a token whose claims carry an employee profile, which `token_for` does
/// not — self-service access to an attachment is decided on exactly that field.
async fn token_for_employee(
    pool: &sqlx::PgPool,
    company_id: uuid::Uuid,
    employee_id: uuid::Uuid,
    role: &str,
) -> String {
    let user_id = seed_user(pool, company_id, role).await;
    let (session_id, _) = session_service::create_session(pool, user_id, None, None)
        .await
        .expect("create test session");
    create_token(
        user_id,
        "upload-test@example.invalid",
        &[role.to_string()],
        Some(company_id),
        Some(employee_id),
        session_id,
        JWT_SECRET,
        1,
    )
    .expect("create jwt")
}

/// Records a claim that references `stored_name`, which is what makes the file
/// reachable at all.
async fn seed_claim_with_receipt(
    pool: &sqlx::PgPool,
    company_id: uuid::Uuid,
    employee_id: uuid::Uuid,
    stored_name: &str,
) {
    sqlx::query(
        r#"INSERT INTO claims
           (employee_id, company_id, title, amount, expense_date, receipt_url)
           VALUES ($1, $2, 'Test claim', 1000, $3, $4)"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap())
    .bind(format!("/api/uploads/{stored_name}"))
    .execute(pool)
    .await
    .expect("insert claim");
}

/// Writes real bytes for a stored name, because authorization passing and the
/// file being absent are the same 404 to a caller: an allow-path test that
/// skipped this would pass even if the rule denied everything.
async fn write_test_upload(stored_name: &str) -> std::path::PathBuf {
    tokio::fs::create_dir_all("uploads")
        .await
        .expect("create uploads dir");
    let path = std::path::Path::new("uploads").join(stored_name);
    tokio::fs::write(&path, b"%PDF-1.4 test receipt")
        .await
        .expect("write test upload");
    path
}

#[tokio::test]
async fn downloading_an_upload_requires_authentication() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let response = app_for(pool)
        .await
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/uploads/whatever.pdf")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_upload_is_not_readable_from_another_company() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let owning_company = seed_company(&pool).await;
    let owning_employee = seed_employee(&pool, owning_company, None, 500_000).await;
    let stored_name = format!("{}_receipt.pdf", uuid::Uuid::new_v4());
    seed_claim_with_receipt(&pool, owning_company, owning_employee, &stored_name).await;
    let path = write_test_upload(&stored_name).await;

    // A different tenant, holding the permission that would cover this file if
    // the claim were theirs.
    let other_company = seed_company(&pool).await;
    let token = token_for(&pool, other_company, "hr_manager").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "GET",
            &format!("/api/uploads/{stored_name}"),
            &token,
            "",
        ))
        .await
        .expect("route response");

    let status = response.status();
    tokio::fs::remove_file(&path).await.ok();

    // 404, never 403: a 403 would confirm the filename is real somewhere else.
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_employee_cannot_read_a_colleagues_receipt() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let owner = seed_employee(&pool, company_id, None, 500_000).await;
    let colleague = seed_employee(&pool, company_id, None, 500_000).await;
    let stored_name = format!("{}_receipt.pdf", uuid::Uuid::new_v4());
    seed_claim_with_receipt(&pool, company_id, owner, &stored_name).await;
    let path = write_test_upload(&stored_name).await;

    let token = token_for_employee(&pool, company_id, colleague, "employee").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "GET",
            &format!("/api/uploads/{stored_name}"),
            &token,
            "",
        ))
        .await
        .expect("route response");

    let status = response.status();
    tokio::fs::remove_file(&path).await.ok();

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_employee_reads_the_receipt_on_their_own_claim() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let stored_name = format!("{}_receipt.pdf", uuid::Uuid::new_v4());
    seed_claim_with_receipt(&pool, company_id, employee_id, &stored_name).await;
    let path = write_test_upload(&stored_name).await;

    let token = token_for_employee(&pool, company_id, employee_id, "employee").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "GET",
            &format!("/api/uploads/{stored_name}"),
            &token,
            "",
        ))
        .await
        .expect("route response");

    let status = response.status();
    tokio::fs::remove_file(&path).await.ok();

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn an_approver_reads_a_claim_receipt_in_their_company() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let stored_name = format!("{}_receipt.pdf", uuid::Uuid::new_v4());
    seed_claim_with_receipt(&pool, company_id, employee_id, &stored_name).await;
    let path = write_test_upload(&stored_name).await;

    // hr_manager holds ViewApprovals, which is what covers a claim receipt
    // belonging to somebody else.
    let token = token_for(&pool, company_id, "hr_manager").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "GET",
            &format!("/api/uploads/{stored_name}"),
            &token,
            "",
        ))
        .await
        .expect("route response");

    let status = response.status();
    tokio::fs::remove_file(&path).await.ok();

    assert_eq!(status, StatusCode::OK);
}

/// `finance` holds neither ViewApprovals nor a claim of its own, so a receipt
/// stays closed to it even inside its own company.
#[tokio::test]
async fn a_role_without_approvals_access_cannot_read_a_receipt() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let stored_name = format!("{}_receipt.pdf", uuid::Uuid::new_v4());
    seed_claim_with_receipt(&pool, company_id, employee_id, &stored_name).await;
    let path = write_test_upload(&stored_name).await;

    let token = token_for(&pool, company_id, "finance").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "GET",
            &format!("/api/uploads/{stored_name}"),
            &token,
            "",
        ))
        .await
        .expect("route response");

    let status = response.status();
    tokio::fs::remove_file(&path).await.ok();

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_traversal_filename_is_rejected_before_any_lookup() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "hr_manager").await;

    let response = app_for(pool)
        .await
        .oneshot(request("GET", "/api/uploads/..%2F..%2F.env", &token, ""))
        .await
        .expect("route response");

    // Deliberately strict. Accepting NOT_FOUND here too would make the test
    // unfailable: removing the path validation entirely still yields a 404 from
    // the reference lookup, so only BAD_REQUEST actually pins the claim that the
    // name is rejected *before* any lookup happens.
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "traversal attempt must be refused by path validation, not by lookup"
    );
}

#[tokio::test]
async fn geofence_and_work_schedule_reads_enforce_permissions_while_bootstrap_stays_open() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let emp_id = seed_employee(&pool, company_id, None, 300_000).await;
    let emp_token = token_for_employee(&pool, company_id, emp_id, "employee").await;
    let exec_token = token_for(&pool, company_id, "exec").await;
    let admin_token = token_for(&pool, company_id, "admin").await;
    let app = app_for(pool).await;

    for path in [
        "/api/geofence/locations",
        "/api/geofence/mode",
        "/api/work-schedules",
        "/api/work-schedules/default",
    ] {
        for (role, token) in [("employee", &emp_token), ("exec", &exec_token)] {
            let resp = app
                .clone()
                .oneshot(request("GET", path, token, ""))
                .await
                .expect("gated route response");
            assert_eq!(
                resp.status(),
                StatusCode::FORBIDDEN,
                "{role} must receive 403 on {path}"
            );
        }

        let admin_resp = app
            .clone()
            .oneshot(request("GET", path, &admin_token, ""))
            .await
            .expect("admin route response");
        assert_eq!(
            admin_resp.status(),
            StatusCode::OK,
            "admin must receive 200 on {path}"
        );
    }

    // Employee still gets advisory schedule timing on /api/attendance/method.
    let method_resp = app
        .oneshot(request("GET", "/api/attendance/method", &emp_token, ""))
        .await
        .expect("bootstrap response");
    assert_eq!(method_resp.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(method_resp.into_body(), 1024 * 64)
        .await
        .expect("read body");
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    assert!(body.get("shift_start_time").is_some());
    assert!(body.get("grace_minutes").is_some());
}

#[tokio::test]
async fn ready_and_admin_platform_health_surface_revision_and_job_state() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let admin_token = token_for(&pool, company_id, "admin").await;
    let emp_token = token_for(&pool, company_id, "employee").await;
    let super_token = token_for(&pool, company_id, "super_admin").await;
    let app = app_for(pool).await;

    let ready_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/health/ready")
                .body(Body::empty())
                .expect("build ready request"),
        )
        .await
        .expect("ready response");
    assert_eq!(ready_resp.status(), StatusCode::OK);
    let ready_bytes = axum::body::to_bytes(ready_resp.into_body(), 1024 * 64)
        .await
        .expect("ready body");
    let ready_json: serde_json::Value = serde_json::from_slice(&ready_bytes).expect("json");
    assert!(ready_json["revision"].is_string());
    assert!(ready_json["version"].is_string());
    assert!(ready_json["jobs"]["retention_cleanup"].is_object());
    assert!(ready_json["jobs"]["auto_absent"].is_object());
    assert!(ready_json["jobs"]["email_outbox"].is_object());
    assert!(ready_json["jobs"]["push_outbox"].is_object());

    for (role, token) in [("admin", &admin_token), ("employee", &emp_token)] {
        let resp = app
            .clone()
            .oneshot(request("GET", "/api/admin/platform/health", token, ""))
            .await
            .expect("forbidden response");
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "{role} must receive 403 on /api/admin/platform/health"
        );
    }

    let ok_resp = app
        .oneshot(request(
            "GET",
            "/api/admin/platform/health",
            &super_token,
            "",
        ))
        .await
        .expect("platform health response");
    assert_eq!(ok_resp.status(), StatusCode::OK);
    let ok_bytes = axum::body::to_bytes(ok_resp.into_body(), 1024 * 64)
        .await
        .expect("platform health body");
    let ok_json: serde_json::Value = serde_json::from_slice(&ok_bytes).expect("json");
    assert!(ok_json["revision"].is_string());
    assert!(ok_json["latest_migration_version"].is_i64());
    assert!(ok_json["jobs"]["email_outbox"].is_object());
    assert!(ok_json["jobs"]["push_outbox"].is_object());
    assert!(ok_json["queues"]["email_outbox_pending"].is_i64());
    assert!(ok_json["queues"]["push_outbox_pending"].is_i64());
}

#[tokio::test]
async fn group_and_draft_permissions_unlock_teams_templates_payroll_groups_and_upload_blocks_exec()
{
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let app = app_for(pool.clone()).await;

    // 1. User-group grants: manage_teams -> GET /api/teams; manage_email_templates -> GET /api/email/templates;
    //    manage_payroll_draft -> GET /api/payroll/groups.
    let (emp_token, emp_user_id) = token_and_user_for(&pool, company_id, "employee").await;
    crate::repositories::user_companies::insert(&pool, emp_user_id, company_id)
        .await
        .expect("link employee user to company");
    let group = crate::repositories::user_groups::insert(
        &pool,
        company_id,
        &format!("scoped-{}", &uuid::Uuid::new_v4().to_string()[..8]),
        None,
        emp_user_id,
    )
    .await
    .expect("seed user group");
    crate::repositories::user_groups::insert_permissions(
        &pool,
        group.id,
        &[
            "manage_teams".to_string(),
            "manage_email_templates".to_string(),
        ],
        emp_user_id,
    )
    .await
    .expect("seed group permissions");
    sqlx::query("INSERT INTO user_group_members (group_id, user_id, added_by) VALUES ($1, $2, $2)")
        .bind(group.id)
        .bind(emp_user_id)
        .execute(&pool)
        .await
        .expect("add group member");

    for path in ["/api/teams", "/api/email/templates"] {
        let resp = app
            .clone()
            .oneshot(request("GET", path, &emp_token, ""))
            .await
            .expect("group read request");
        assert_eq!(resp.status(), StatusCode::OK, "expected 200 on {path}");
    }

    // Payroll capabilities require a non-restricted role (`hr_manager` holds no ViewPayroll by default).
    let (hr_token, hr_user_id) = token_and_user_for(&pool, company_id, "hr_manager").await;
    crate::repositories::user_companies::insert(&pool, hr_user_id, company_id)
        .await
        .expect("link hr_manager to company");
    let payroll_draft_group = crate::repositories::user_groups::insert(
        &pool,
        company_id,
        &format!("draft-{}", &uuid::Uuid::new_v4().to_string()[..8]),
        None,
        hr_user_id,
    )
    .await
    .expect("seed payroll draft group");
    crate::repositories::user_groups::insert_permissions(
        &pool,
        payroll_draft_group.id,
        &["manage_payroll_draft".to_string()],
        hr_user_id,
    )
    .await
    .expect("seed payroll draft permission");
    sqlx::query("INSERT INTO user_group_members (group_id, user_id, added_by) VALUES ($1, $2, $2)")
        .bind(payroll_draft_group.id)
        .bind(hr_user_id)
        .execute(&pool)
        .await
        .expect("add hr_manager member");

    let pg_resp = app
        .clone()
        .oneshot(request("GET", "/api/payroll-groups", &hr_token, ""))
        .await
        .expect("payroll-groups request");
    assert_eq!(pg_resp.status(), StatusCode::OK);

    // Also verify send_letters alone grants GET /api/email/templates.
    let (letter_token, letter_user_id) = token_and_user_for(&pool, company_id, "employee").await;
    crate::repositories::user_companies::insert(&pool, letter_user_id, company_id)
        .await
        .expect("link letter user to company");
    let letter_group = crate::repositories::user_groups::insert(
        &pool,
        company_id,
        &format!("letters-{}", &uuid::Uuid::new_v4().to_string()[..8]),
        None,
        letter_user_id,
    )
    .await
    .expect("seed letter group");
    crate::repositories::user_groups::insert_permissions(
        &pool,
        letter_group.id,
        &["send_letters".to_string()],
        letter_user_id,
    )
    .await
    .expect("seed letter permissions");
    sqlx::query("INSERT INTO user_group_members (group_id, user_id, added_by) VALUES ($1, $2, $2)")
        .bind(letter_group.id)
        .bind(letter_user_id)
        .execute(&pool)
        .await
        .expect("add letter member");

    let letter_resp = app
        .clone()
        .oneshot(request("GET", "/api/email/templates", &letter_token, ""))
        .await
        .expect("send_letters templates request");
    assert_eq!(letter_resp.status(), StatusCode::OK);

    // 2. Read-only exec without employee_id must be rejected on POST /api/uploads.
    let exec_token = token_for(&pool, company_id, "exec").await;
    let upload_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/uploads")
                .header(header::AUTHORIZATION, format!("Bearer {exec_token}"))
                .header(header::CONTENT_TYPE, "multipart/form-data; boundary=---BOUNDARY")
                .body(Body::from(
                    "-----BOUNDARY\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.txt\"\r\n\r\nhi\r\n-----BOUNDARY--\r\n",
                ))
                .expect("build upload request"),
        )
        .await
        .expect("upload response");
    assert_eq!(upload_resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn portal_working_days_returns_count_when_range_given_and_validates_params() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let emp_token = token_for(&pool, company_id, "employee").await;
    // Wed 2026-06-10 holiday inside Mon 2026-06-08..Fri 2026-06-12 -> 4 working days.
    sqlx::query(
        "INSERT INTO holidays (company_id, name, date, holiday_type, is_recurring)
         VALUES ($1, 'Midweek Holiday', '2026-06-10', 'public_holiday', FALSE)",
    )
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("seed holiday");

    let app = app_for(pool.clone()).await;
    let ok_resp = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/portal/working-days?start_date=2026-06-08&end_date=2026-06-12",
            &emp_token,
            "",
        ))
        .await
        .expect("working days response");
    assert_eq!(ok_resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(ok_resp.into_body(), 1024 * 64)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["working_days"], 4);

    for bad_uri in [
        "/api/portal/working-days?start_date=2026-06-12&end_date=2026-06-08",
        "/api/portal/working-days?start_date=2026-06-08",
    ] {
        let bad_resp = app
            .clone()
            .oneshot(request("GET", bad_uri, &emp_token, ""))
            .await
            .expect("bad working days response");
        assert_eq!(bad_resp.status(), StatusCode::BAD_REQUEST, "{bad_uri}");
    }
}

#[tokio::test]
async fn deleting_active_company_preserves_super_admin_session_and_rotates_token() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_a = seed_company(&pool).await;
    let company_b = seed_company(&pool).await;
    let (super_token, super_id) = token_and_user_for(&pool, company_a, "super_admin").await;
    crate::repositories::user_companies::insert(&pool, super_id, company_b)
        .await
        .expect("link second company");

    let app = app_for(pool).await;
    let del_resp = app
        .clone()
        .oneshot(request(
            "DELETE",
            &format!("/api/admin/companies/{company_a}"),
            &super_token,
            "",
        ))
        .await
        .expect("delete company response");
    assert_eq!(del_resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(del_resp.into_body(), 1024 * 64)
        .await
        .expect("delete body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("delete json");
    let new_token = json["token"].as_str().expect("fresh token");
    assert_eq!(
        json["user"]["company_id"].as_str(),
        Some(company_b.to_string().as_str())
    );

    let me_resp = app
        .oneshot(request("GET", "/api/auth/me", new_token, ""))
        .await
        .expect("me with fresh token");
    assert_eq!(me_resp.status(), StatusCode::OK);
}

// ─── Bank detail redaction ───
//
// Banking decides where salary lands. A role that manages employees but holds
// no `ViewPayroll` (`hr_manager`, `manager`) must never receive an employee's
// bank fields, and its edits must not be able to wipe them. Payroll roles and
// the employee themself (portal) keep seeing them.

const SEEDED_BANK_NAME: &str = "Maybank";
const SEEDED_BANK_ACCOUNT: &str = "514011223344";
const SEEDED_BANK_TYPE: &str = "savings";
const SEEDED_IC: &str = "900101-14-5566";

async fn seed_employee_with_bank(pool: &sqlx::PgPool, company_id: uuid::Uuid) -> uuid::Uuid {
    let employee_id = seed_employee(pool, company_id, None, 500_000).await;
    sqlx::query(
        "UPDATE employees
            SET bank_name = $2, bank_account_number = $3, bank_account_type = $4,
                ic_number = $5
          WHERE id = $1",
    )
    .bind(employee_id)
    .bind(SEEDED_BANK_NAME)
    .bind(SEEDED_BANK_ACCOUNT)
    .bind(SEEDED_BANK_TYPE)
    .bind(SEEDED_IC)
    .execute(pool)
    .await
    .expect("seed bank details");
    employee_id
}

async fn stored_bank(
    pool: &sqlx::PgPool,
    employee_id: uuid::Uuid,
) -> (Option<String>, Option<String>, Option<String>) {
    sqlx::query_as(
        "SELECT bank_name, bank_account_number, bank_account_type FROM employees WHERE id = $1",
    )
    .bind(employee_id)
    .fetch_one(pool)
    .await
    .expect("read stored bank details")
}

async fn json_body(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    serde_json::from_slice(&bytes).expect("json body")
}

fn assert_no_bank_fields(value: &serde_json::Value, context: &str) {
    let object = value.as_object().expect("employee object");
    for (key, field) in object {
        if key.starts_with("bank_") {
            assert!(field.is_null(), "{context}: `{key}` leaked as {field}");
        }
    }
    let raw = value.to_string();
    assert!(
        !raw.contains(SEEDED_BANK_ACCOUNT),
        "{context}: bank account number leaked"
    );
}

#[tokio::test]
async fn non_payroll_roles_do_not_receive_bank_details() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee_with_bank(&pool, company_id).await;
    let app = app_for(pool.clone()).await;

    for role in ["hr_manager", "manager"] {
        let token = token_for(&pool, company_id, role).await;

        let detail = app
            .clone()
            .oneshot(request(
                "GET",
                &format!("/api/employees/{employee_id}"),
                &token,
                "",
            ))
            .await
            .expect("detail response");
        assert_eq!(detail.status(), StatusCode::OK, "{role} detail");
        let detail = json_body(detail).await;
        assert_no_bank_fields(&detail, &format!("{role} detail"));
        // Identity details stay visible to employee managers: only banking
        // moved behind the payroll gate.
        assert_eq!(detail["ic_number"], SEEDED_IC, "{role} keeps IC");

        let list = app
            .clone()
            .oneshot(request("GET", "/api/employees?per_page=100", &token, ""))
            .await
            .expect("list response");
        assert_eq!(list.status(), StatusCode::OK, "{role} list");
        let list = json_body(list).await;
        let row = list["data"]
            .as_array()
            .expect("list data")
            .iter()
            .find(|e| e["id"] == employee_id.to_string())
            .expect("seeded employee in list")
            .clone();
        assert_no_bank_fields(&row, &format!("{role} list"));

        let search = app
            .clone()
            .oneshot(request(
                "GET",
                "/api/employees?search=Test%20Employee",
                &token,
                "",
            ))
            .await
            .expect("search response");
        assert_eq!(search.status(), StatusCode::OK, "{role} search");
        let search = json_body(search).await;
        for row in search["data"].as_array().expect("search data") {
            assert_no_bank_fields(row, &format!("{role} search"));
        }
    }
}

#[tokio::test]
async fn non_payroll_role_updates_preserve_bank_details() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee_with_bank(&pool, company_id).await;
    let app = app_for(pool.clone()).await;

    for role in ["hr_manager", "manager"] {
        let token = token_for(&pool, company_id, role).await;

        let update = app
            .clone()
            .oneshot(request(
                "PUT",
                &format!("/api/employees/{employee_id}"),
                &token,
                &format!(r#"{{"full_name":"Renamed by {role}","department":"Ops"}}"#),
            ))
            .await
            .expect("update response");
        assert_eq!(update.status(), StatusCode::OK, "{role} plain update");
        let updated = json_body(update).await;
        assert_eq!(updated["full_name"], format!("Renamed by {role}"));
        assert_no_bank_fields(&updated, &format!("{role} update response"));
        assert_eq!(
            stored_bank(&pool, employee_id).await,
            (
                Some(SEEDED_BANK_NAME.to_string()),
                Some(SEEDED_BANK_ACCOUNT.to_string()),
                Some(SEEDED_BANK_TYPE.to_string()),
            ),
            "{role} plain update must keep stored banking"
        );

        // Wiping banking is as sensitive as rewriting it.
        for body in [
            r#"{"clear_bank_name":true}"#,
            r#"{"clear_bank_account_number":true}"#,
            r#"{"bank_account_number":""}"#,
            r#"{"bank_name":null,"bank_account_number":"999999999999"}"#,
        ] {
            let response = app
                .clone()
                .oneshot(request(
                    "PUT",
                    &format!("/api/employees/{employee_id}"),
                    &token,
                    body,
                ))
                .await
                .expect("bank write response");
            assert_eq!(
                response.status(),
                StatusCode::FORBIDDEN,
                "{role} must not send {body}"
            );
        }
        assert_eq!(
            stored_bank(&pool, employee_id).await,
            (
                Some(SEEDED_BANK_NAME.to_string()),
                Some(SEEDED_BANK_ACCOUNT.to_string()),
                Some(SEEDED_BANK_TYPE.to_string()),
            ),
            "{role} bank writes must leave stored banking intact"
        );
    }
}

#[tokio::test]
async fn payroll_roles_and_the_employee_still_see_bank_details() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee_with_bank(&pool, company_id).await;
    let app = app_for(pool.clone()).await;

    for role in ["payroll_admin", "super_admin"] {
        let token = token_for(&pool, company_id, role).await;
        let detail = app
            .clone()
            .oneshot(request(
                "GET",
                &format!("/api/employees/{employee_id}"),
                &token,
                "",
            ))
            .await
            .expect("detail response");
        assert_eq!(detail.status(), StatusCode::OK, "{role} detail");
        let detail = json_body(detail).await;
        assert_eq!(detail["bank_name"], SEEDED_BANK_NAME, "{role}");
        assert_eq!(detail["bank_account_number"], SEEDED_BANK_ACCOUNT, "{role}");
        assert_eq!(detail["bank_account_type"], SEEDED_BANK_TYPE, "{role}");

        let list = app
            .clone()
            .oneshot(request("GET", "/api/employees?per_page=100", &token, ""))
            .await
            .expect("list response");
        let list = json_body(list).await;
        let row = list["data"]
            .as_array()
            .expect("list data")
            .iter()
            .find(|e| e["id"] == employee_id.to_string())
            .expect("seeded employee in list")
            .clone();
        assert_eq!(
            row["bank_account_number"], SEEDED_BANK_ACCOUNT,
            "{role} list"
        );
    }

    // The employee reading their own record in the portal keeps their banking.
    let own_token = token_for_employee(&pool, company_id, employee_id, "employee").await;
    let profile = app
        .clone()
        .oneshot(request("GET", "/api/portal/profile", &own_token, ""))
        .await
        .expect("portal profile response");
    assert_eq!(profile.status(), StatusCode::OK);
    let profile = json_body(profile).await;
    assert_eq!(profile["bank_name"], SEEDED_BANK_NAME);
    assert_eq!(profile["bank_account_number"], SEEDED_BANK_ACCOUNT);
}

/// `admin` holds `ViewAuditLog` but no payroll permission; the employee diffs
/// it can read must not carry banking, including bank keys not known today.
#[tokio::test]
async fn non_payroll_audit_log_redacts_bank_details() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee_with_bank(&pool, company_id).await;

    crate::services::audit_service::log_action_with_metadata(
        &pool,
        Some(company_id),
        None,
        "update",
        "employee",
        Some(employee_id),
        Some(serde_json::json!({
            "full_name": "Test Employee",
            "bank_name": "CIMB Bank",
            "bank_account_number": "700000000001",
        })),
        Some(serde_json::json!({
            "full_name": "Test Employee",
            "bank_name": SEEDED_BANK_NAME,
            "bank_account_number": SEEDED_BANK_ACCOUNT,
            "bank_account_type": SEEDED_BANK_TYPE,
            "bank_account_holder": "TEST EMPLOYEE",
        })),
        None,
        None,
    )
    .await
    .expect("log employee bank change");

    let app = app_for(pool.clone()).await;
    let uri = format!("/api/audit-logs?entity_type=employee&entity_id={employee_id}");

    let viewer_token = token_for(&pool, company_id, "admin").await;
    let response = app
        .clone()
        .oneshot(request("GET", &uri, &viewer_token, ""))
        .await
        .expect("audit response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    let row = body["data"]
        .as_array()
        .expect("audit data")
        .first()
        .expect("employee audit row")
        .clone();
    for side in ["old_values", "new_values"] {
        let values = row[side].as_object().expect("audit values");
        for (key, value) in values {
            if key.starts_with("bank_") {
                assert_eq!(value, "[REDACTED]", "admin audit {side}.{key}");
            }
        }
        assert_eq!(values["full_name"], "Test Employee");
    }
    assert!(!row.to_string().contains(SEEDED_BANK_ACCOUNT));
    assert!(!row.to_string().contains("700000000001"));

    // A payroll role reading the same trail still sees the change.
    let admin_token = token_for(&pool, company_id, "super_admin").await;
    let response = app
        .clone()
        .oneshot(request("GET", &uri, &admin_token, ""))
        .await
        .expect("audit response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(
        body["data"][0]["new_values"]["bank_account_number"],
        SEEDED_BANK_ACCOUNT
    );
}

/// The built-in `manager` role does not include the audit trail: every audit
/// endpoint answers 403, while `admin` keeps access.
#[tokio::test]
async fn manager_cannot_read_audit_logs() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let app = app_for(pool.clone()).await;
    let manager_token = token_for(&pool, company_id, "manager").await;
    let admin_token = token_for(&pool, company_id, "admin").await;
    let run_id = uuid::Uuid::new_v4();

    for uri in [
        "/api/audit-logs".to_string(),
        "/api/audit-logs/filters".to_string(),
        format!("/api/payroll/runs/{run_id}/audit-logs"),
    ] {
        let response = app
            .clone()
            .oneshot(request("GET", &uri, &manager_token, ""))
            .await
            .expect("audit response");
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "manager must be denied {uri}"
        );
    }

    for uri in ["/api/audit-logs", "/api/audit-logs/filters"] {
        let response = app
            .clone()
            .oneshot(request("GET", uri, &admin_token, ""))
            .await
            .expect("audit response");
        assert_eq!(response.status(), StatusCode::OK, "admin keeps {uri}");
    }
}
