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

fn test_config(database_url: String) -> AppConfig {
    AppConfig {
        database_url,
        jwt_secret: JWT_SECRET.to_string(),
        jwt_expiry_hours: 1,
        server_host: "127.0.0.1".to_string(),
        server_port: 0,
        frontend_url: "http://localhost:5173".to_string(),
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
    };

    routes::create_router(state).layer(Extension(JwtSecret(config.jwt_secret)))
}

async fn token_for(pool: &sqlx::PgPool, company_id: uuid::Uuid, role: &str) -> String {
    token_and_user_for(pool, company_id, role).await.0
}

async fn token_and_user_for(
    pool: &sqlx::PgPool,
    company_id: uuid::Uuid,
    role: &str,
) -> (String, uuid::Uuid) {
    let user_id = seed_user(pool, company_id, role).await;
    let (session_id, _) = session_service::create_session(pool, user_id, None)
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
    let (session_id, _) = session_service::create_session(pool, user_id, None)
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

    assert!(
        matches!(
            response.status(),
            StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND
        ),
        "traversal attempt returned {}",
        response.status()
    );
}
