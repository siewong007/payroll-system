use std::io::{Cursor, Write};
use std::net::IpAddr;

use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use chrono::{NaiveDate, Utc};
use rust_decimal_macros::dec;
use tower::ServiceExt;
use uuid::Uuid;

use crate::core::auth::{
    AuthUser, Claims, JWT_AUDIENCE, JWT_ISSUER, create_token_with_exp, verify_token,
};
use crate::core::client_ip::client_ip_with_header;
use crate::core::config::AppConfig;
use crate::core::error::AppError;
use crate::core::http_client::is_blocked_ip;
use crate::core::permission::{Permission, role_permissions};
use crate::core::upload_path;
use crate::models::audit::AuditLogWithUser;
use crate::models::employee::CreateEmployeeRequest;
use crate::models::notification::{PushSubscriptionKeys, UpsertPushSubscriptionRequest};
use crate::models::portal::{
    CreateClaimRequest, CreateLeaveRequest, CreateOvertimeRequest, UpdateClaimRequest,
    UpdateLeaveRequest, UpdateOvertimeRequest,
};
use crate::models::user_company::CreateUserRequest;
use crate::repositories::{
    mfa_challenges, payroll_runs, uploaded_files, user_companies, user_totp, user_totp_backup_codes,
};
use crate::services::approval_service::{self, Reviewer};
use crate::services::{
    audit_service, auth_service, employee_import_service, employee_service, notification_service,
    passkey_service, payroll_lifecycle_service, payroll_service, portal_service, session_service,
    upload_service, user_group_service, user_service,
};
use crate::tests::route_auth_tests::{JWT_SECRET, app_for, token_and_user_for, token_for};
use crate::tests::support::{
    seed_company, seed_employee, seed_payroll_group, seed_user, skip_if_no_db,
};

fn json_request(method: &str, uri: &str, token: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-forwarded-for", "203.0.113.10")
        .header(header::USER_AGENT, "PayrollSecurityTest/1.0")
        .body(Body::from(body.to_string()))
        .expect("build request")
}

fn sec_test_config(database_url: String) -> AppConfig {
    AppConfig {
        database_url,
        jwt_secret: JWT_SECRET.to_string(),
        totp_encryption_key: crate::tests::route_auth_tests::TOTP_ENCRYPTION_KEY.to_string(),
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
    }
}

fn fake_auth(user_id: Uuid, company_id: Uuid, role: &str) -> AuthUser {
    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: user_id,
        email: format!("{user_id}@example.invalid"),
        roles: vec![role.to_string()],
        company_id: Some(company_id),
        employee_id: None,
        sid: Uuid::new_v4(),
        exp: now + 3600,
        iat: now,
        iss: JWT_ISSUER.to_string(),
        aud: JWT_AUDIENCE.to_string(),
    };
    AuthUser(claims, vec![])
}

#[tokio::test]
async fn user_group_blocks_platform_only_conflict_and_enforces_monotonic_delegation() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_id, "super_admin").await;
    let hr_manager_id = seed_user(&pool, company_id, "hr_manager").await;
    let payroll_admin_id = seed_user(&pool, company_id, "payroll_admin").await;
    let exec_id = seed_user(&pool, company_id, "exec").await;

    user_companies::insert(&pool, payroll_admin_id, company_id)
        .await
        .expect("attach payroll_admin to company");
    user_companies::insert(&pool, exec_id, company_id)
        .await
        .expect("attach exec to company");

    let super_auth = fake_auth(super_admin_id, company_id, "super_admin");
    let hr_auth = fake_auth(hr_manager_id, company_id, "hr_manager");

    // 1. Platform-only permissions cannot be granted in company-scoped user groups.
    let err = user_group_service::create_group(
        &pool,
        company_id,
        "Escalate Users",
        None,
        &["manage_users".to_string()],
        &super_auth,
        None,
    )
    .await
    .expect_err("manage_users must be forbidden in user groups");
    assert!(matches!(
        err,
        AppError::Forbidden(_) | AppError::BadRequest(_)
    ));

    // 2. A single group cannot grant both submit_payroll and approve_payroll.
    let conflict = user_group_service::create_group(
        &pool,
        company_id,
        "Both Sides",
        None,
        &["submit_payroll".to_string(), "approve_payroll".to_string()],
        &super_auth,
        None,
    )
    .await
    .expect_err("combining submit_payroll and approve_payroll in one group must fail");
    assert!(matches!(conflict, AppError::BadRequest(_)));

    // 3. Monotonic delegation: hr_manager cannot grant manage_payroll_draft which it lacks.
    let unheld = user_group_service::create_group(
        &pool,
        company_id,
        "Unheld Grant",
        None,
        &["manage_payroll_draft".to_string()],
        &hr_auth,
        None,
    )
    .await
    .expect_err("caller lacking a permission cannot delegate it");
    assert!(matches!(unheld, AppError::Forbidden(_)));

    // 4. Across roles + groups: payroll_admin holds submit_payroll via role, so adding to approve_payroll fails.
    let approve_group = user_group_service::create_group(
        &pool,
        company_id,
        "Payroll Approvers",
        None,
        &["approve_payroll".to_string()],
        &super_auth,
        None,
    )
    .await
    .expect("super_admin can create approve_payroll group");

    let cross_err = user_group_service::add_member(
        &pool,
        company_id,
        approve_group.id,
        payroll_admin_id,
        &super_auth,
        None,
    )
    .await
    .expect_err("payroll_admin must not gain approve_payroll across roles/groups");
    assert!(matches!(cross_err, AppError::BadRequest(_)));

    // 5. Restricted exec/employee accounts cannot be granted payroll capabilities via group.
    let exec_err = user_group_service::add_member(
        &pool,
        company_id,
        approve_group.id,
        exec_id,
        &super_auth,
        None,
    )
    .await
    .expect_err("exec user must not gain payroll capability via group");
    assert!(matches!(exec_err, AppError::BadRequest(_)));
}

#[tokio::test]
async fn exec_lacks_write_permissions_and_non_super_admin_blocked_from_platform_routes() {
    assert!(!role_permissions("exec").contains(&Permission::ManageDocuments));
    assert!(!role_permissions("exec").contains(&Permission::ManageEmailTemplates));

    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let admin_token = token_for(&pool, company_id, "admin").await;

    let companies_res = app_for(pool.clone())
        .await
        .oneshot(json_request(
            "GET",
            "/api/admin/companies",
            &admin_token,
            "",
        ))
        .await
        .expect("companies response");
    assert_eq!(companies_res.status(), StatusCode::FORBIDDEN);

    let backup_res = app_for(pool)
        .await
        .oneshot(json_request(
            "GET",
            "/api/admin/backup/export",
            &admin_token,
            "",
        ))
        .await
        .expect("backup response");
    assert_eq!(backup_res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn must_change_password_enforced_and_onboarding_uses_csprng_password() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (token, user_id) = token_and_user_for(&pool, company_id, "admin").await;

    sqlx::query("UPDATE users SET must_change_password = TRUE WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("flag must_change_password");

    let blocked = app_for(pool.clone())
        .await
        .oneshot(json_request("GET", "/api/employees", &token, ""))
        .await
        .expect("blocked response");
    assert_eq!(blocked.status(), StatusCode::FORBIDDEN);
    let body = to_bytes(blocked.into_body(), 16 * 1024)
        .await
        .expect("read body");
    let body_text = String::from_utf8_lossy(&body);
    assert!(
        body_text.contains("Password change required before accessing this resource"),
        "unexpected body: {body_text}"
    );

    let me_ok = app_for(pool.clone())
        .await
        .oneshot(json_request("GET", "/api/auth/me", &token, ""))
        .await
        .expect("/api/auth/me response");
    assert_eq!(me_ok.status(), StatusCode::OK);

    let skip_gone = app_for(pool.clone())
        .await
        .oneshot(json_request(
            "POST",
            "/api/auth/skip-change-password",
            &token,
            "{}",
        ))
        .await
        .expect("skip endpoint response");
    assert!(
        matches!(
            skip_gone.status(),
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
        ),
        "expected 404/405 for removed skip endpoint, got {}",
        skip_gone.status()
    );

    // Password strength checks (<=72 bytes enforced alongside existing strength rule).
    assert!(auth_service::validate_password_strength("Str0ngPassword").is_ok());
    let too_long = "Aa1".repeat(25);
    assert!(too_long.len() > 72);
    assert!(matches!(
        auth_service::validate_password_strength(&too_long),
        Err(AppError::Validation(_))
    ));

    // Employee onboarding issues a 16-char CSPRNG password (never Welcome@123 or IC number).
    let seq = Uuid::new_v4().simple().to_string();
    let req = CreateEmployeeRequest {
        employee_number: format!("SEC-{}", &seq[..8]),
        full_name: "Onboard User".into(),
        ic_number: Some("900101-01-1234".into()),
        passport_number: None,
        date_of_birth: NaiveDate::from_ymd_opt(1990, 1, 1),
        gender: None,
        nationality: None,
        race: None,
        residency_status: None,
        marital_status: None,
        email: Some(format!("onboard-{}@example.invalid", &seq[..8])),
        phone: None,
        address_line1: None,
        address_line2: None,
        city: None,
        state: None,
        postcode: None,
        department: None,
        designation: None,
        cost_centre: None,
        branch: None,
        employment_type: None,
        date_joined: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        probation_start: None,
        probation_end: None,
        basic_salary: 400_000,
        hourly_rate: None,
        daily_rate: None,
        bank_name: None,
        bank_account_number: None,
        bank_account_type: None,
        tax_identification_number: None,
        epf_number: None,
        socso_number: None,
        eis_number: None,
        working_spouse: None,
        num_children: None,
        epf_category: Some("A".into()),
        is_muslim: None,
        zakat_eligible: None,
        zakat_monthly_amount: None,
        ptptn_monthly_amount: None,
        tabung_haji_amount: None,
        payroll_group_id: None,
        salary_group: None,
    };
    let (_emp, account_info) =
        employee_service::create_employee(&pool, company_id, req, user_id, None)
            .await
            .expect("create_employee with email");
    let pw = account_info
        .and_then(|a| a.default_password)
        .expect("default_password generated");
    assert_eq!(pw.len(), 16);
    assert_ne!(pw, "Welcome@123");
    assert_ne!(pw, "900101-01-1234");
}

#[tokio::test]
async fn maker_checker_blocks_payroll_and_email_matched_claim_self_approval() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let processor_id = seed_user(&pool, company_id, "payroll_admin").await;
    let submitter_id = seed_user(&pool, company_id, "finance").await;
    let approver_id = seed_user(&pool, company_id, "finance").await;

    let run_id = Uuid::new_v4();
    let d1 = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();
    payroll_runs::insert_processing(
        &pool,
        run_id,
        company_id,
        group_id,
        2026,
        3,
        d1,
        d2,
        d2,
        processor_id,
        None,
    )
    .await
    .expect("insert_processing");
    payroll_runs::update_totals(
        &pool, run_id, 500_000, 450_000, 560_000, 55_000, 60_000, 0, 0, 0, 0, 0, 0, 1,
    )
    .await
    .expect("update_totals");
    payroll_lifecycle_service::submit_for_approval(&pool, company_id, run_id, submitter_id, None)
        .await
        .expect("submit_for_approval");

    let err_proc = payroll_lifecycle_service::approve(
        &pool,
        company_id,
        run_id,
        &fake_auth(processor_id, company_id, "finance"),
        None,
    )
    .await
    .expect_err("processor must not approve own run");
    assert!(matches!(err_proc, AppError::Forbidden(_)));

    let err_sub = payroll_lifecycle_service::approve(
        &pool,
        company_id,
        run_id,
        &fake_auth(submitter_id, company_id, "finance"),
        None,
    )
    .await
    .expect_err("submitter must not approve run they submitted");
    assert!(matches!(err_sub, AppError::Forbidden(_)));

    payroll_lifecycle_service::approve(
        &pool,
        company_id,
        run_id,
        &fake_auth(approver_id, company_id, "finance"),
        None,
    )
    .await
    .expect("independent approver can approve");

    // Claim maker-checker: user lacks users.employee_id link, but shares employee's email in same company.
    let shared_email = format!("claim-mc-{}@example.invalid", Uuid::new_v4().simple());
    let employee_id = seed_employee(&pool, company_id, None, 350_000).await;
    sqlx::query("UPDATE employees SET email = $1 WHERE id = $2")
        .bind(&shared_email)
        .bind(employee_id)
        .execute(&pool)
        .await
        .expect("set employee email");

    let reviewer_id = seed_user(&pool, company_id, "admin").await;
    sqlx::query("UPDATE users SET email = $1, employee_id = NULL WHERE id = $2")
        .bind(&shared_email)
        .bind(reviewer_id)
        .execute(&pool)
        .await
        .expect("match reviewer email");

    let claim_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO claims (id, employee_id, company_id, title, amount, expense_date, status)
           VALUES ($1, $2, $3, 'Mileage', 2500, $4, 'pending')"#,
    )
    .bind(claim_id)
    .bind(employee_id)
    .bind(company_id)
    .bind(d1)
    .execute(&pool)
    .await
    .expect("insert pending claim");

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://test".to_string());
    let cfg = sec_test_config(db_url);
    let self_claim_err = approval_service::approve_claim(
        &pool,
        &cfg,
        company_id,
        claim_id,
        Reviewer {
            user_id: reviewer_id,
            may_self_approve: false,
        },
        None,
        None,
    )
    .await
    .expect_err("email-matched self-approval must be blocked");
    assert!(matches!(self_claim_err, AppError::Forbidden(_)));
}

#[tokio::test]
async fn confidential_documents_and_cross_tenant_upload_isolation() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_a = seed_company(&pool).await;
    let company_b = seed_company(&pool).await;

    let (exec_token_a, exec_user_a) = token_and_user_for(&pool, company_a, "exec").await;
    let (emp_token_a, _emp_user_a) = token_and_user_for(&pool, company_a, "employee").await;
    let (hr_token_a, hr_user_a) = token_and_user_for(&pool, company_a, "hr_manager").await;
    let (admin_token_b, _admin_user_b) = token_and_user_for(&pool, company_b, "admin").await;

    let stored_name = format!("{}_confidential.pdf", Uuid::now_v7());
    let file_url = format!("/api/uploads/{stored_name}");
    uploaded_files::insert(&pool, &stored_name, company_a, Some(hr_user_a))
        .await
        .expect("insert uploaded_files owner");

    sqlx::query(
        r#"INSERT INTO documents
           (id, company_id, title, file_name, file_url, is_confidential, created_by)
           VALUES ($1, $2, 'Board Comp Memo', 'confidential.pdf', $3, TRUE, $4)"#,
    )
    .bind(Uuid::now_v7())
    .bind(company_a)
    .bind(&file_url)
    .bind(exec_user_a)
    .execute(&pool)
    .await
    .expect("insert confidential doc");

    let disk_path = upload_path::stored_path(&stored_name).expect("stored_path");
    if let Some(parent) = disk_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .expect("create uploads dir");
    }
    tokio::fs::write(&disk_path, b"%PDF-1.4\n%test\n")
        .await
        .expect("write test pdf");

    // 1. Exec sees 0 confidential docs in /api/documents.
    let list_res = app_for(pool.clone())
        .await
        .oneshot(json_request("GET", "/api/documents", &exec_token_a, ""))
        .await
        .expect("documents list");
    assert_eq!(list_res.status(), StatusCode::OK);
    let list_bytes = to_bytes(list_res.into_body(), 64 * 1024).await.unwrap();
    let list_json: serde_json::Value = serde_json::from_slice(&list_bytes).unwrap();
    assert_eq!(list_json["data"].as_array().unwrap().len(), 0);

    // 2. Employee cannot read confidential upload (uniform 404).
    let emp_dl = app_for(pool.clone())
        .await
        .oneshot(json_request("GET", &file_url, &emp_token_a, ""))
        .await
        .expect("employee upload download");
    assert_eq!(emp_dl.status(), StatusCode::NOT_FOUND);

    // 3. Cross-tenant admin in company B receives uniform 404 on GET /api/uploads/{stored_name}.
    let cross_dl = app_for(pool.clone())
        .await
        .oneshot(json_request("GET", &file_url, &admin_token_b, ""))
        .await
        .expect("cross-tenant upload download");
    assert_eq!(cross_dl.status(), StatusCode::NOT_FOUND);

    // 4. Company B cannot bind Company A's uploaded file URL onto a record.
    let bind_err =
        upload_service::ensure_upload_belongs_to_company(&pool, company_b, Some(&file_url))
            .await
            .expect_err("cross-tenant upload bind must fail");
    assert!(matches!(bind_err, AppError::BadRequest(_)));

    // 5. HR manager in Company A can download the confidential document.
    let hr_dl = app_for(pool)
        .await
        .oneshot(json_request("GET", &file_url, &hr_token_a, ""))
        .await
        .expect("hr upload download");
    let hr_status = hr_dl.status();

    let _ = tokio::fs::remove_file(&disk_path).await;
    assert_eq!(hr_status, StatusCode::OK);
}

#[tokio::test]
async fn totp_replay_lockout_backup_codes_and_passkey_challenge_binding() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let user_a = seed_user(&pool, company_id, "admin").await;
    let user_b = seed_user(&pool, company_id, "admin").await;

    user_totp::upsert_pending(&pool, user_a, "encrypted-secret")
        .await
        .expect("upsert_pending");
    user_totp::confirm(&pool, user_a, 0).await.expect("confirm");

    assert!(
        user_totp::claim_totp_step(&pool, user_a, 100)
            .await
            .unwrap()
    );
    assert!(
        !user_totp::claim_totp_step(&pool, user_a, 100)
            .await
            .unwrap()
    );
    assert!(!user_totp::claim_totp_step(&pool, user_a, 99).await.unwrap());
    assert!(
        user_totp::claim_totp_step(&pool, user_a, 101)
            .await
            .unwrap()
    );

    for _ in 0..5 {
        user_totp::record_failure(&pool, user_a, 5, 15)
            .await
            .expect("record_failure");
    }
    let row = user_totp::find_by_user(&pool, user_a)
        .await
        .unwrap()
        .expect("totp row");
    assert_eq!(row.failed_attempts, 5);
    assert!(
        row.locked_until.is_some_and(|t| t > Utc::now()),
        "5 failures must trigger future lockout"
    );

    let lookup_hash = "0".repeat(64);
    user_totp_backup_codes::insert_many(
        &pool,
        user_a,
        &[String::from("$2b$10$hash")],
        std::slice::from_ref(&lookup_hash),
    )
    .await
    .expect("insert backup code");
    let code_row = user_totp_backup_codes::find_unused_by_lookup(&pool, user_a, &lookup_hash)
        .await
        .unwrap()
        .expect("inserted backup code");
    let code_id = code_row.id;
    assert!(
        user_totp_backup_codes::mark_used(&pool, code_id)
            .await
            .unwrap()
    );
    assert!(
        !user_totp_backup_codes::mark_used(&pool, code_id)
            .await
            .unwrap()
    );

    // Passkey registration challenge is bound to its creating user.
    let challenge_id = passkey_service::store_challenge(
        &pool,
        Some(user_a),
        None,
        "registration",
        &serde_json::json!({"state": "ok"}),
    )
    .await
    .expect("store registration challenge");

    assert!(
        passkey_service::consume_registration_challenge(&pool, challenge_id, user_b)
            .await
            .is_err(),
        "user B must not consume user A's registration challenge"
    );
    assert!(
        passkey_service::consume_registration_challenge(&pool, challenge_id, user_a)
            .await
            .is_ok(),
        "owner user A should still be able to consume challenge"
    );
    assert!(
        passkey_service::consume_registration_challenge(&pool, challenge_id, user_a)
            .await
            .is_err(),
        "challenge must be single-use"
    );
}

#[test]
fn xlsx_zip_bomb_and_ipv6_ssrf_guards() {
    // 1. ZIP path traversal rejected before calamine opens workbook.
    let mut traversal_buf = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(Cursor::new(&mut traversal_buf));
        let opts = zip::write::SimpleFileOptions::default();
        writer.start_file("../evil.xml", opts).unwrap();
        writer.write_all(b"<x/>").unwrap();
        writer.finish().unwrap();
    }
    let err = employee_import_service::parse_xlsx(&traversal_buf)
        .expect_err("ZIP traversal entry must be rejected");
    assert!(
        matches!(&err, AppError::BadRequest(msg) if msg == "Excel archive contains unsafe entry path"),
        "unexpected error: {err}"
    );

    // 2. ZIP entry-count bomb rejected (>200 entries).
    let mut bomb_buf = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(Cursor::new(&mut bomb_buf));
        let opts = zip::write::SimpleFileOptions::default();
        for idx in 0..201 {
            writer.start_file(format!("e{idx}.xml"), opts).unwrap();
            writer.write_all(b"a").unwrap();
        }
        writer.finish().unwrap();
    }
    let err = employee_import_service::parse_xlsx(&bomb_buf)
        .expect_err("oversized entry-count ZIP must be rejected");
    assert!(
        matches!(&err, AppError::BadRequest(msg) if msg == "Excel archive exceeds maximum entry count"),
        "unexpected error: {err}"
    );

    // 3. IPv6 SSRF ranges (docs, Teredo, 6to4, NAT64, site-local, discard/orchid) vs public.
    for blocked in [
        "2001:db8::1",
        "2001::1",
        "2002:7f00:1::1",
        "64:ff9b::a9fe:a9fe",
        "fec0::1",
        "100::1",
        "169.254.169.254",
        "127.0.0.1",
    ] {
        let ip: IpAddr = blocked.parse().unwrap();
        assert!(is_blocked_ip(ip), "expected SSRF block for {blocked}");
    }
    for allowed in ["2001:4860:4860::8888", "1.1.1.1"] {
        let ip: IpAddr = allowed.parse().unwrap();
        assert!(!is_blocked_ip(ip), "expected public IP allowed: {allowed}");
    }
}

#[tokio::test]
async fn retention_cleanup_purges_aged_resets_and_succeeded_jobs() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let user_id = seed_user(&pool, company_id, "admin").await;

    let old_rejected_reset = Uuid::new_v4();
    let fresh_completed_reset = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO password_reset_requests (id, user_id, status, requested_at)
           VALUES
             ($1, $3, 'rejected', NOW() - INTERVAL '31 days'),
             ($2, $3, 'completed', NOW() - INTERVAL '1 day')"#,
    )
    .bind(old_rejected_reset)
    .bind(fresh_completed_reset)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("seed password_reset_requests");

    let old_succeeded_job = Uuid::new_v4();
    let old_running_job = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO background_jobs (id, company_id, job_type, status, payload, created_by, created_at, finished_at)
           VALUES
             ($1, $3, 'employee_import', 'succeeded', '{}'::jsonb, $4, NOW() - INTERVAL '40 days', NOW() - INTERVAL '31 days'),
             ($2, $3, 'employee_import', 'running', '{}'::jsonb, $4, NOW() - INTERVAL '40 days', NULL)"#,
    )
    .bind(old_succeeded_job)
    .bind(old_running_job)
    .bind(company_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("seed background_jobs");

    let purged_resets =
        crate::repositories::password_reset_requests::delete_expired_older_than_days(&pool, 30)
            .await
            .expect("purge expired resets");
    assert!(purged_resets >= 1);

    let purged_jobs =
        crate::repositories::background_jobs::delete_finished_older_than_days(&pool, 30)
            .await
            .expect("purge finished jobs");
    assert!(purged_jobs >= 1);

    let reset_exists = |id: Uuid| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM password_reset_requests WHERE id = $1)",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    let job_exists = |id: Uuid| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM background_jobs WHERE id = $1)",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };

    assert!(!reset_exists(old_rejected_reset).await);
    assert!(reset_exists(fresh_completed_reset).await);
    assert!(!job_exists(old_succeeded_job).await);
    assert!(job_exists(old_running_job).await);
}

#[tokio::test]
async fn switch_company_preserves_jwt_exp_and_revokes_removed_company_access() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_a = seed_company(&pool).await;
    let company_b = seed_company(&pool).await;
    let user_id = seed_user(&pool, company_a, "admin").await;
    user_companies::insert(&pool, user_id, company_b)
        .await
        .expect("grant company_b access");

    let (session_id, _) = session_service::create_session(&pool, user_id, None, None)
        .await
        .expect("create session");
    let fixed_exp = Utc::now().timestamp() + 1800;
    let initial_token = create_token_with_exp(
        user_id,
        "switch@example.invalid",
        &["admin".to_string()],
        Some(company_a),
        None,
        session_id,
        JWT_SECRET,
        fixed_exp,
    )
    .expect("mint fixed-exp token");

    let app = app_for(pool.clone()).await;
    let switch_res = app
        .clone()
        .oneshot(json_request(
            "PUT",
            "/api/auth/switch-company",
            &initial_token,
            &format!(r#"{{"company_id":"{company_b}"}}"#),
        ))
        .await
        .expect("switch-company response");
    assert_eq!(switch_res.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(switch_res.into_body(), 64 * 1024).await.unwrap())
            .unwrap();
    let new_token = body["token"].as_str().expect("switched JWT present");
    let claims = verify_token(new_token, JWT_SECRET).expect("verify switched token");
    assert_eq!(
        claims.exp, fixed_exp,
        "switch-company must not extend JWT exp"
    );
    assert_eq!(claims.company_id, Some(company_b));

    // Removing the user_companies membership immediately invalidates the active-company JWT.
    sqlx::query("DELETE FROM user_companies WHERE user_id = $1 AND company_id = $2")
        .bind(user_id)
        .bind(company_b)
        .execute(&pool)
        .await
        .expect("revoke company_b membership");

    let revoked_res = app
        .oneshot(json_request("GET", "/api/employees", new_token, ""))
        .await
        .expect("request after membership removal");
    assert_eq!(revoked_res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn maker_checker_blocks_creator_and_editor_self_approval_on_leave_claim_and_overtime() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let actor_id = seed_user(&pool, company_id, "hr_manager").await;
    let employee_id = seed_employee(&pool, company_id, None, 400_000).await;
    let emp_user_id = seed_user(&pool, company_id, "employee").await;
    let leave_type_id: Uuid = sqlx::query_scalar(
        "INSERT INTO leave_types (company_id, name, default_days, is_paid) VALUES ($1, $2, 12, TRUE) RETURNING id",
    )
    .bind(company_id)
    .bind(format!("Annual-{}", &Uuid::new_v4().to_string()[..8]))
    .fetch_one(&pool)
    .await
    .expect("seed leave type");
    sqlx::query(
        r#"INSERT INTO leave_balances (employee_id, leave_type_id, year, entitled_days)
           VALUES ($1, $2, 2026, 12)"#,
    )
    .bind(employee_id)
    .bind(leave_type_id)
    .execute(&pool)
    .await
    .expect("seed leave balance");

    let cfg = sec_test_config(std::env::var("DATABASE_URL").unwrap_or_default());
    let access = upload_service::access_from_auth(
        &fake_auth(actor_id, company_id, "hr_manager"),
        company_id,
    );
    let reviewer = Reviewer {
        user_id: actor_id,
        may_self_approve: false,
    };

    // 1. Admin creates a claim -> cannot approve own creation.
    let claim = approval_service::create_claim_admin(
        &pool,
        company_id,
        employee_id,
        CreateClaimRequest {
            title: "Parking".into(),
            description: None,
            amount: 2500,
            category: None,
            receipt_url: None,
            receipt_file_name: None,
            expense_date: NaiveDate::from_ymd_opt(2026, 3, 10).unwrap(),
        },
        actor_id,
        &access,
        None,
    )
    .await
    .expect("create_claim_admin");
    assert!(matches!(
        approval_service::approve_claim(&pool, &cfg, company_id, claim.id, reviewer, None, None)
            .await,
        Err(AppError::Forbidden(_))
    ));

    // 2. Employee submits leave, actor_id edits it (`updated_by`), -> actor_id cannot approve.
    let leave = portal_service::create_leave_request(
        &pool,
        employee_id,
        company_id,
        emp_user_id,
        CreateLeaveRequest {
            leave_type_id,
            start_date: NaiveDate::from_ymd_opt(2026, 3, 11).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2026, 3, 11).unwrap(),
            days: dec!(1),
            reason: Some("Personal".into()),
            attachment_url: None,
            attachment_name: None,
        },
        &upload_service::access_from_auth(
            &fake_auth(emp_user_id, company_id, "employee"),
            company_id,
        ),
    )
    .await
    .expect("create_leave_request");
    approval_service::update_leave_request_admin(
        &pool,
        company_id,
        leave.id,
        UpdateLeaveRequest {
            employee_id: None,
            leave_type_id: None,
            start_date: None,
            end_date: None,
            days: None,
            reason: Some("Edited by reviewer".into()),
            attachment_url: None,
            attachment_name: None,
        },
        actor_id,
        &access,
        None,
    )
    .await
    .expect("update_leave_request_admin");
    assert!(matches!(
        approval_service::approve_leave(&pool, &cfg, company_id, leave.id, reviewer, None, None)
            .await,
        Err(AppError::Forbidden(_))
    ));

    // 3. Employee submits overtime, actor_id edits it -> actor_id cannot approve.
    let ot = portal_service::create_overtime_application(
        &pool,
        employee_id,
        company_id,
        emp_user_id,
        CreateOvertimeRequest {
            ot_date: NaiveDate::from_ymd_opt(2026, 3, 12).unwrap(),
            start_time: "18:00".into(),
            end_time: "20:00".into(),
            hours: dec!(2),
            ot_type: Some("normal".into()),
            reason: Some("Release".into()),
        },
    )
    .await
    .expect("create_overtime_application");
    approval_service::update_overtime_admin(
        &pool,
        company_id,
        ot.id,
        UpdateOvertimeRequest {
            employee_id: None,
            ot_date: None,
            start_time: None,
            end_time: None,
            hours: Some(dec!(2)),
            ot_type: None,
            reason: Some("Edited by reviewer".into()),
        },
        actor_id,
        None,
    )
    .await
    .expect("update_overtime_admin");
    assert!(matches!(
        approval_service::approve_overtime(&pool, company_id, ot.id, reviewer, None, None).await,
        Err(AppError::Forbidden(_))
    ));

    // Confirm claim editor path too.
    let claim_by_emp = approval_service::create_claim_admin(
        &pool,
        company_id,
        employee_id,
        CreateClaimRequest {
            title: "Tolls".into(),
            description: None,
            amount: 1200,
            category: None,
            receipt_url: None,
            receipt_file_name: None,
            expense_date: NaiveDate::from_ymd_opt(2026, 3, 13).unwrap(),
        },
        emp_user_id,
        &upload_service::access_from_auth(
            &fake_auth(emp_user_id, company_id, "employee"),
            company_id,
        ),
        None,
    )
    .await
    .expect("claim by emp");
    approval_service::update_claim_admin(
        &pool,
        company_id,
        claim_by_emp.id,
        UpdateClaimRequest {
            employee_id: None,
            title: Some("Edited Tolls".into()),
            description: None,
            amount: None,
            category: None,
            receipt_url: None,
            receipt_file_name: None,
            expense_date: None,
        },
        actor_id,
        &access,
        None,
    )
    .await
    .expect("update_claim_admin");
    assert!(matches!(
        approval_service::approve_claim(
            &pool,
            &cfg,
            company_id,
            claim_by_emp.id,
            Reviewer {
                user_id: actor_id,
                may_self_approve: false,
            },
            None,
            None,
        )
        .await,
        Err(AppError::Forbidden(_))
    ));
}

#[test]
fn xlsx_coordinate_bomb_and_csv_bounds_are_rejected_without_oom() {
    let mut xlsx_buf = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(Cursor::new(&mut xlsx_buf));
        let opts = zip::write::SimpleFileOptions::default();
        writer.start_file("[Content_Types].xml", opts).unwrap();
        writer
            .write_all(
                br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#,
            )
            .unwrap();
        writer.start_file("_rels/.rels", opts).unwrap();
        writer
            .write_all(
                br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
            )
            .unwrap();
        writer.start_file("xl/workbook.xml", opts).unwrap();
        writer
            .write_all(
                br#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>
</workbook>"#,
            )
            .unwrap();
        writer
            .start_file("xl/_rels/workbook.xml.rels", opts)
            .unwrap();
        writer
            .write_all(
                br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#,
            )
            .unwrap();
        writer.start_file("xl/worksheets/sheet1.xml", opts).unwrap();
        writer
            .write_all(
                br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:XFD1048576"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Name</t></is></c></row>
    <row r="1048576"><c r="XFD1048576" t="inlineStr"><is><t>boom</t></is></c></row>
  </sheetData>
</worksheet>"#,
            )
            .unwrap();
        writer.finish().unwrap();
    }
    assert!(matches!(
        employee_import_service::parse_xlsx(&xlsx_buf),
        Err(AppError::BadRequest(_))
    ));

    let mut csv_rows = String::from("Name\n");
    for i in 0..1002 {
        csv_rows.push_str(&format!("Employee {i}\n"));
    }
    assert!(matches!(
        employee_import_service::parse_csv(csv_rows.as_bytes()),
        Err(AppError::BadRequest(_))
    ));

    let wide_csv = format!(
        "{}\n{}",
        vec!["col"; 205].join(","),
        vec!["x"; 205].join(",")
    );
    assert!(matches!(
        employee_import_service::parse_csv(wide_csv.as_bytes()),
        Err(AppError::BadRequest(_))
    ));
}

#[test]
fn client_ip_with_header_trusts_only_configured_header_over_xff() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", "198.51.100.77".parse().unwrap());
    headers.insert("cf-connecting-ip", "203.0.113.44".parse().unwrap());
    let peer: IpAddr = "10.0.0.5".parse().unwrap();

    assert_eq!(
        client_ip_with_header(&headers, Some(peer), true, Some("cf-connecting-ip")),
        Some("203.0.113.44".parse().unwrap())
    );

    // When preferred header is configured but missing, do not fall back to XFF.
    let mut xff_only = HeaderMap::new();
    xff_only.insert("x-forwarded-for", "198.51.100.77".parse().unwrap());
    assert_eq!(
        client_ip_with_header(&xff_only, Some(peer), true, Some("cf-connecting-ip")),
        Some(peer)
    );
}

#[tokio::test]
async fn confidential_sibling_ref_blocks_read_and_unindexed_upload_attach_fails_closed() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let emp_id = seed_employee(&pool, company_id, None, 350_000).await;
    let emp_user = seed_user(&pool, company_id, "employee").await;
    let hr_user = seed_user(&pool, company_id, "hr_manager").await;

    let stored_name = format!("{}_dualref.pdf", Uuid::now_v7());
    let file_url = format!("/api/uploads/{stored_name}");
    uploaded_files::insert(&pool, &stored_name, company_id, Some(hr_user))
        .await
        .expect("insert uploaded_files");

    let disk_path = upload_path::stored_path(&stored_name).expect("stored_path");
    if let Some(parent) = disk_path.parent() {
        tokio::fs::create_dir_all(parent).await.unwrap();
    }
    tokio::fs::write(&disk_path, b"%PDF-1.4 test")
        .await
        .unwrap();

    // Claim belongs to emp_id, but same upload URL is also attached to a confidential document.
    sqlx::query(
        r#"INSERT INTO claims (employee_id, company_id, title, amount, expense_date, receipt_url)
           VALUES ($1, $2, 'Dual Ref', 500, $3, $4)"#,
    )
    .bind(emp_id)
    .bind(company_id)
    .bind(NaiveDate::from_ymd_opt(2026, 3, 1).unwrap())
    .bind(&file_url)
    .execute(&pool)
    .await
    .expect("insert claim");
    sqlx::query(
        r#"INSERT INTO documents (company_id, employee_id, title, file_name, file_url, is_confidential, created_by)
           VALUES ($1, $2, 'Confidential Dual', 'dualref.pdf', $3, TRUE, $4)"#,
    )
    .bind(company_id)
    .bind(emp_id)
    .bind(&file_url)
    .bind(hr_user)
    .execute(&pool)
    .await
    .expect("insert confidential doc");

    let mut emp_access =
        upload_service::access_from_auth(&fake_auth(emp_user, company_id, "employee"), company_id);
    emp_access.employee_id = Some(emp_id);
    let dl_err = upload_service::read_authorized(&pool, &stored_name, &emp_access)
        .await
        .expect_err("confidential ref must block non-confidential read even with own claim ref");
    let _ = tokio::fs::remove_file(&disk_path).await;
    assert!(matches!(dl_err, AppError::NotFound(_)));

    // Unknown unindexed local upload path fails closed on attach and company checks.
    assert!(matches!(
        upload_service::ensure_upload_attach_authorized(
            &pool,
            &emp_access,
            Some("/api/uploads/unindexed-random-file.pdf"),
            false,
        )
        .await,
        Err(AppError::BadRequest(_))
    ));
}

#[tokio::test]
async fn sod_rejection_mfa_challenge_single_use_and_step_up_passwords() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_a = seed_company(&pool).await;
    let company_b = seed_company(&pool).await;
    let admin_id = seed_user(&pool, company_a, "admin").await;

    // 1. M5: create_user unconditionally rejects finance + payroll_admin SoD roles.
    let sod_err = user_service::create_user(
        &pool,
        CreateUserRequest {
            email: format!("sod-{}@example.invalid", Uuid::new_v4()),
            password: "Sup3rSecretPassw0rd".into(),
            full_name: "SoD Violation".into(),
            roles: vec!["finance".into(), "payroll_admin".into()],
            company_ids: vec![company_a],
        },
        admin_id,
        None,
    )
    .await
    .expect_err("finance + payroll_admin SoD must be refused on create_user");
    assert!(matches!(sod_err, AppError::BadRequest(_)));

    // 2. M6: mfa_challenges consume is strictly single-use.
    let jti = Uuid::new_v4();
    mfa_challenges::insert(
        &pool,
        jti,
        admin_id,
        Utc::now() + chrono::Duration::minutes(5),
    )
    .await
    .expect("insert mfa challenge");
    assert!(
        mfa_challenges::consume(&pool, jti, admin_id, 5)
            .await
            .unwrap()
    );
    assert!(
        !mfa_challenges::consume(&pool, jti, admin_id, 5)
            .await
            .unwrap()
    );

    // 3. M7: setup/confirm and passkey/register/begin require real current password.
    let email = format!("stepup-{}@example.invalid", Uuid::new_v4());
    let password = "Sup3rSecretPassw0rd";
    let pw_user_id = Uuid::new_v4();
    let hash = bcrypt::hash(password, 4).unwrap();
    sqlx::query(
        r#"INSERT INTO users (id, email, password_hash, full_name, roles, company_id)
           VALUES ($1, $2, $3, 'StepUp User', ARRAY['employee']::VARCHAR(50)[], $4)"#,
    )
    .bind(pw_user_id)
    .bind(&email)
    .bind(&hash)
    .bind(company_a)
    .execute(&pool)
    .await
    .unwrap();
    user_companies::insert(&pool, pw_user_id, company_a)
        .await
        .unwrap();
    user_companies::insert(&pool, pw_user_id, company_b)
        .await
        .unwrap();

    let (sid, _) = session_service::create_session(&pool, pw_user_id, None, None)
        .await
        .unwrap();
    let tok = create_token_with_exp(
        pw_user_id,
        &email,
        &["employee".to_string()],
        Some(company_a),
        None,
        sid,
        JWT_SECRET,
        Utc::now().timestamp() + 3600,
    )
    .unwrap();

    let app = app_for(pool.clone()).await;
    let bad_totp_confirm = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/auth/2fa/setup/confirm",
            &tok,
            r#"{"code":"123456","password":"WrongPassword1"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(bad_totp_confirm.status(), StatusCode::BAD_REQUEST);

    let bad_passkey_begin = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/auth/passkey/register/begin",
            &tok,
            r#"{"password":"WrongPassword1"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(bad_passkey_begin.status(), StatusCode::BAD_REQUEST);

    // 4. M5: switch-company into company_b where user holds ApprovePayroll group perm
    // under enforce_2fa = "true" must fail 422 without 2FA enrolled.
    let _policy_guard = crate::tests::totp_route_tests::POLICY_SENSITIVE_LOGINS
        .lock()
        .await;
    let super_auth = fake_auth(admin_id, company_b, "super_admin");
    let group = user_group_service::create_group(
        &pool,
        company_b,
        "Approvers",
        None,
        &["approve_payroll".to_string()],
        &super_auth,
        None,
    )
    .await
    .expect("create group in company_b");
    sqlx::query("INSERT INTO user_group_members (group_id, user_id, added_by) VALUES ($1, $2, $3)")
        .bind(group.id)
        .bind(pw_user_id)
        .bind(admin_id)
        .execute(&pool)
        .await
        .expect("add member to company_b group");

    crate::repositories::platform_settings::set_value(
        &pool,
        auth_service::ENFORCE_2FA_KEY,
        "true",
        None,
    )
    .await
    .unwrap();
    let switch_blocked = app
        .oneshot(json_request(
            "PUT",
            "/api/auth/switch-company",
            &tok,
            &format!(r#"{{"company_id":"{company_b}"}}"#),
        ))
        .await
        .unwrap();
    crate::repositories::platform_settings::set_value(
        &pool,
        auth_service::ENFORCE_2FA_KEY,
        "false",
        None,
    )
    .await
    .unwrap();
    assert_eq!(switch_blocked.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn push_subscription_quota_caps_active_devices_at_20() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let user_id = seed_user(&pool, company_id, "employee").await;
    let p256dh =
        "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
    let auth = "BTBZMqHH6r4Tts7J_aSIgg";

    let mut first_endpoint = String::new();
    for i in 0..20 {
        let endpoint = format!(
            "https://fcm.googleapis.com/fcm/send/device-{}-{i}",
            Uuid::new_v4()
        );
        if i == 0 {
            first_endpoint = endpoint.clone();
        }
        let status = notification_service::upsert_push_subscription(
            &pool,
            user_id,
            Some(company_id),
            None,
            UpsertPushSubscriptionRequest {
                endpoint,
                keys: PushSubscriptionKeys {
                    p256dh: p256dh.into(),
                    auth: auth.into(),
                },
            },
            None,
        )
        .await
        .expect("subscribe within 20-device quota");
        assert_eq!(status.active_subscriptions, (i + 1) as i64);
    }

    // Re-upserting an existing active endpoint succeeds at cap.
    let updated = notification_service::upsert_push_subscription(
        &pool,
        user_id,
        Some(company_id),
        None,
        UpsertPushSubscriptionRequest {
            endpoint: first_endpoint,
            keys: PushSubscriptionKeys {
                p256dh: p256dh.into(),
                auth: auth.into(),
            },
        },
        None,
    )
    .await
    .expect("refresh existing endpoint at quota");
    assert_eq!(updated.active_subscriptions, 20);

    // A 21st distinct endpoint is rejected.
    let over = notification_service::upsert_push_subscription(
        &pool,
        user_id,
        Some(company_id),
        None,
        UpsertPushSubscriptionRequest {
            endpoint: format!(
                "https://fcm.googleapis.com/fcm/send/device-21-{}",
                Uuid::new_v4()
            ),
            keys: PushSubscriptionKeys {
                p256dh: p256dh.into(),
                auth: auth.into(),
            },
        },
        None,
    )
    .await
    .expect_err("21st endpoint must fail quota check");
    assert!(matches!(over, AppError::BadRequest(_)));
}

#[tokio::test]
async fn audit_redacts_extended_sensitive_keys_and_run_audit_masks_privileged_actor() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let admin_id = seed_user(&pool, company_id, "admin").await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let run_id = Uuid::new_v4();
    let d1 = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2026, 4, 30).unwrap();
    payroll_runs::insert_processing(
        &pool, run_id, company_id, group_id, 2026, 4, d1, d2, d2, admin_id, None,
    )
    .await
    .unwrap();

    let meta = crate::models::audit::AuditRequestMeta {
        ip_address: Some("203.0.113.55".into()),
        user_agent: Some("AdminBrowser/1.0".into()),
    };
    audit_service::log_action_with_metadata(
        &pool,
        Some(company_id),
        Some(admin_id),
        "update",
        "payroll_run",
        Some(run_id),
        None,
        Some(serde_json::json!({
            "notes": "Secret bonus note",
            "description": "Private run adjustment"
        })),
        Some("Edited run"),
        Some(&meta),
    )
    .await
    .unwrap();

    let unmasked = payroll_service::list_run_audit_logs(&pool, company_id, run_id, false)
        .await
        .expect("list_run_audit_logs unmasked");
    assert_eq!(unmasked.len(), 1);
    assert_eq!(unmasked[0].user_id, Some(admin_id));
    assert!(unmasked[0].user_email.is_some());

    let masked = payroll_service::list_run_audit_logs(&pool, company_id, run_id, true)
        .await
        .expect("list_run_audit_logs masked");
    assert_eq!(masked.len(), 1);
    assert!(masked[0].user_id.is_none());
    assert!(masked[0].user_email.is_none());
    assert!(masked[0].user_full_name.is_none());
    assert!(masked[0].ip_address.is_none());
    assert!(masked[0].user_agent.is_none());

    let mut rows = vec![
        AuditLogWithUser {
            id: Uuid::new_v4(),
            user_id: Some(admin_id),
            action: "update".into(),
            entity_type: "employee".into(),
            entity_id: None,
            old_values: None,
            new_values: Some(serde_json::json!({
                "working_spouse": true,
                "is_muslim": true,
                "city": "Kuala Lumpur",
                "state": "WP",
                "full_name": "Aisyah"
            })),
            ip_address: None,
            user_agent: None,
            description: None,
            created_at: Utc::now(),
            user_email: None,
            user_full_name: None,
        },
        masked.into_iter().next().unwrap(),
    ];

    audit_service::redact_audit_logs(&mut rows, true, true);
    assert_eq!(
        rows[0].new_values.as_ref().unwrap()["working_spouse"],
        "[REDACTED]"
    );
    assert_eq!(
        rows[0].new_values.as_ref().unwrap()["is_muslim"],
        "[REDACTED]"
    );
    assert_eq!(rows[0].new_values.as_ref().unwrap()["city"], "[REDACTED]");
    assert_eq!(rows[0].new_values.as_ref().unwrap()["state"], "[REDACTED]");
    assert_eq!(rows[0].new_values.as_ref().unwrap()["full_name"], "Aisyah");
    assert_eq!(rows[1].new_values.as_ref().unwrap()["notes"], "[REDACTED]");
    assert_eq!(
        rows[1].new_values.as_ref().unwrap()["description"],
        "[REDACTED]"
    );
}

#[tokio::test]
async fn email_preview_and_send_with_employee_id_require_view_employees() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let admin_id = seed_user(&pool, company_id, "admin").await;
    let sender_id = seed_user(&pool, company_id, "employee").await;
    let emp_id = seed_employee(&pool, company_id, None, 350_000).await;
    sqlx::query("UPDATE employees SET email = 'emp-letter@example.invalid' WHERE id = $1")
        .bind(emp_id)
        .execute(&pool)
        .await
        .unwrap();

    let super_auth = fake_auth(admin_id, company_id, "super_admin");
    let group = user_group_service::create_group(
        &pool,
        company_id,
        "LetterSendersWithoutEmpView",
        None,
        &[
            "manage_email_templates".to_string(),
            "send_letters".to_string(),
        ],
        &super_auth,
        None,
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO user_group_members (group_id, user_id, added_by) VALUES ($1, $2, $3)")
        .bind(group.id)
        .bind(sender_id)
        .bind(admin_id)
        .execute(&pool)
        .await
        .unwrap();

    let (sid, _) = session_service::create_session(&pool, sender_id, None, None)
        .await
        .unwrap();
    let tok = create_token_with_exp(
        sender_id,
        "letter@example.invalid",
        &["employee".to_string()],
        Some(company_id),
        None,
        sid,
        JWT_SECRET,
        Utc::now().timestamp() + 3600,
    )
    .unwrap();

    let app = app_for(pool).await;
    let preview_res = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/email/preview",
            &tok,
            &format!(r#"{{"employee_id":"{emp_id}","subject":"Hi","body_html":"<p>Test</p>"}}"#),
        ))
        .await
        .unwrap();
    assert_eq!(preview_res.status(), StatusCode::FORBIDDEN);

    let send_res = app
        .oneshot(json_request(
            "POST",
            "/api/email/send",
            &tok,
            &format!(
                r#"{{"employee_id":"{emp_id}","letter_type":"general","subject":"Hi","body_html":"<p>Test</p>"}}"#
            ),
        ))
        .await
        .unwrap();
    assert_eq!(send_res.status(), StatusCode::FORBIDDEN);
}
