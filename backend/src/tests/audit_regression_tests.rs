use chrono::{NaiveDate, NaiveTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;

use crate::core::config::AppConfig;
use crate::core::error::AppError;
use crate::models::company::UpdateCompanyRequest;
use crate::models::portal::CreateClaimRequest;
use crate::models::upload::UploadAccess;
use crate::repositories::company_work_schedules;
use crate::services::approval_service::{self, Reviewer};
use crate::services::backup_service::{export_company, import_company};
use crate::services::{
    company_service, payroll_engine, portal_service, team_service, upload_service,
};
use crate::tests::support::{
    seed_company, seed_employee, seed_payroll_group, seed_user, skip_if_no_db,
};

fn test_config() -> AppConfig {
    AppConfig {
        database_url: String::new(),
        jwt_secret: "test-secret-that-is-long-enough-for-tests".into(),
        totp_encryption_key: "audit-regression-test-totp-key".into(),
        jwt_expiry_hours: 1,
        server_host: "0.0.0.0".into(),
        server_port: 8080,
        frontend_url: "http://localhost:5173".into(),
        api_public_url: String::new(),
        google_client_id: None,
        google_client_secret: None,
        webauthn_rp_id: "localhost".into(),
        webauthn_rp_origin: "http://localhost:5173".into(),
        smtp_host: None,
        smtp_port: None,
        smtp_username: None,
        smtp_password: None,
        smtp_from_email: None,
        smtp_from_name: None,
        trust_proxy_headers: false,
        client_ip_header: None,
        turnstile_secret_key: None,
        turnstile_verify_url: "https://challenges.cloudflare.com/turnstile/v0/siteverify".into(),
        vapid_public_key: None,
        vapid_private_key: None,
        vapid_subject: None,
    }
}

#[tokio::test]
async fn approvals_list_and_approve_succeed_when_employee_email_is_null() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let reviewer_id = seed_user(&pool, company_id, "hr_manager").await;
    // `seed_employee` leaves `employees.email` NULL.
    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;

    let claim_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO claims
           (id, employee_id, company_id, title, amount, expense_date, status, submitted_at)
           VALUES ($1, $2, $3, 'Mileage', 4500, $4, 'pending', NOW())"#,
    )
    .bind(claim_id)
    .bind(employee_id)
    .bind(company_id)
    .bind(NaiveDate::from_ymd_opt(2024, 6, 4).unwrap())
    .execute(&pool)
    .await
    .expect("insert pending claim");

    let ot_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO overtime_applications
           (id, employee_id, company_id, ot_date, start_time, end_time, hours, ot_type, status)
           VALUES ($1, $2, $3, $4, '18:00', '20:00', 2.0, 'normal', 'pending')"#,
    )
    .bind(ot_id)
    .bind(employee_id)
    .bind(company_id)
    .bind(NaiveDate::from_ymd_opt(2024, 6, 4).unwrap())
    .execute(&pool)
    .await
    .expect("insert pending overtime");

    let (claims, claims_total) =
        approval_service::get_pending_claims(&pool, company_id, Some("pending"), 20, 0)
            .await
            .expect("list pending claims");
    assert_eq!(claims_total, 1);
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].id, claim_id);

    let (ots, ot_total) =
        approval_service::get_pending_overtime(&pool, company_id, Some("pending"), 20, 0)
            .await
            .expect("list pending overtime");
    assert_eq!(ot_total, 1);
    assert_eq!(ots.len(), 1);
    assert_eq!(ots[0].id, ot_id);

    let approved_claim = approval_service::approve_claim(
        &pool,
        &test_config(),
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
    .expect("approve claim when employee email is null");
    assert_eq!(approved_claim.status, "approved");

    let approved_ot = approval_service::approve_overtime(
        &pool,
        company_id,
        ot_id,
        Reviewer {
            user_id: reviewer_id,
            may_self_approve: false,
        },
        None,
        None,
    )
    .await
    .expect("approve overtime when employee email is null");
    assert_eq!(approved_ot.status, "approved");
}

#[tokio::test]
async fn upload_attach_rejects_cross_company_and_other_users_unreferenced_upload() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_a = seed_company(&pool).await;
    let company_b = seed_company(&pool).await;
    let user_a1 = seed_user(&pool, company_a, "hr_manager").await;
    let user_a2 = seed_user(&pool, company_a, "hr_manager").await;
    let user_b1 = seed_user(&pool, company_b, "hr_manager").await;

    let stored_name = format!("{}-receipt.pdf", Uuid::new_v4());
    let file_url = format!("/api/uploads/{stored_name}");
    sqlx::query(
        r#"INSERT INTO uploaded_files (stored_name, company_id, uploaded_by)
           VALUES ($1, $2, $3)"#,
    )
    .bind(&stored_name)
    .bind(company_a)
    .bind(user_a1)
    .execute(&pool)
    .await
    .expect("seed uploaded_files row");

    let access_a1 = UploadAccess {
        company_id: company_a,
        user_id: user_a1,
        employee_id: None,
        can_view_approvals: true,
        can_view_documents: true,
        can_view_confidential: true,
    };
    upload_service::ensure_upload_attach_authorized(&pool, &access_a1, Some(&file_url), false)
        .await
        .expect("uploader in same company can attach their unreferenced file");

    let access_a2 = UploadAccess {
        company_id: company_a,
        user_id: user_a2,
        employee_id: None,
        can_view_approvals: false,
        can_view_documents: false,
        can_view_confidential: false,
    };
    let same_company_idor =
        upload_service::ensure_upload_attach_authorized(&pool, &access_a2, Some(&file_url), false)
            .await
            .expect_err("another user in the same company cannot attach unreferenced upload");
    assert!(matches!(same_company_idor, AppError::BadRequest(_)));

    let access_b1 = UploadAccess {
        company_id: company_b,
        user_id: user_b1,
        employee_id: None,
        can_view_approvals: true,
        can_view_documents: true,
        can_view_confidential: true,
    };
    let cross_company_idor =
        upload_service::ensure_upload_attach_authorized(&pool, &access_b1, Some(&file_url), false)
            .await
            .expect_err("another company cannot attach upload");
    assert!(matches!(cross_company_idor, AppError::BadRequest(_)));
}

#[tokio::test]
async fn backup_roundtrip_preserves_cancelled_run_salary_approver_and_unpaid_break() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let source_company = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, source_company).await;
    let admin_id = seed_user(&pool, source_company, "admin").await;
    let employee_id = seed_employee(&pool, source_company, Some(group_id), 500_000).await;

    company_work_schedules::upsert_default(
        &pool,
        source_company,
        "Flex Shift",
        NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
        NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
        15,
        4.0,
        "Asia/Kuala_Lumpur",
        45,
    )
    .await
    .expect("seed schedule with unpaid break");

    let cancelled_at = Utc::now();
    sqlx::query(
        r#"INSERT INTO payroll_runs
           (company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status,
            cancelled_at, cancel_reason)
           VALUES ($1, $2, 2024, 6, $3, $4, $4, 'cancelled', $5, 'Plans changed')"#,
    )
    .bind(source_company)
    .bind(group_id)
    .bind(NaiveDate::from_ymd_opt(2024, 6, 1).unwrap())
    .bind(NaiveDate::from_ymd_opt(2024, 6, 30).unwrap())
    .bind(cancelled_at)
    .execute(&pool)
    .await
    .expect("seed cancelled payroll run");

    sqlx::query(
        r#"INSERT INTO salary_history
           (id, employee_id, company_id, old_salary, new_salary, effective_date, change_type, approved_by, approved_at, created_by)
           VALUES ($1, $2, $3, 450000, 500000, $4, 'increment', $5, NOW(), $5)"#,
    )
    .bind(Uuid::new_v4())
    .bind(employee_id)
    .bind(source_company)
    .bind(NaiveDate::from_ymd_opt(2024, 6, 1).unwrap())
    .bind(admin_id)
    .execute(&pool)
    .await
    .expect("seed salary_history with approved_by");

    let mut backup = export_company(&pool, source_company)
        .await
        .expect("export company backup");
    backup.company.name = format!("Restored-{}", Uuid::new_v4());

    let imported = import_company(&pool, backup, None, admin_id, None)
        .await
        .expect("restore backup into new company");
    let restored_company = imported.new_company_id;

    let break_minutes: i32 = sqlx::query_scalar(
        "SELECT unpaid_break_minutes FROM company_work_schedules WHERE company_id = $1 AND is_default = TRUE",
    )
    .bind(restored_company)
    .fetch_one(&pool)
    .await
    .expect("read restored schedule");
    assert_eq!(break_minutes, 45);

    let (restored_cancelled_at, restored_reason): (Option<chrono::DateTime<Utc>>, Option<String>) =
        sqlx::query_as("SELECT cancelled_at, cancel_reason FROM payroll_runs WHERE company_id = $1")
            .bind(restored_company)
            .fetch_one(&pool)
            .await
            .expect("read restored payroll run");
    assert!(restored_cancelled_at.is_some());
    assert_eq!(restored_reason.as_deref(), Some("Plans changed"));

    let (restored_approved_by, restored_approved_at): (
        Option<Uuid>,
        Option<chrono::DateTime<Utc>>,
    ) = sqlx::query_as("SELECT approved_by, approved_at FROM salary_history WHERE company_id = $1")
        .bind(restored_company)
        .fetch_one(&pool)
        .await
        .expect("read restored salary_history");
    assert!(restored_approved_by.is_none());
    assert!(restored_approved_at.is_some());
}

#[tokio::test]
async fn half_day_unpaid_leave_deducts_half_daily_rate() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    sqlx::query("UPDATE companies SET unpaid_leave_divisor = 20 WHERE id = $1")
        .bind(company_id)
        .execute(&pool)
        .await
        .expect("set unpaid_leave_divisor = 20");

    let group_id = seed_payroll_group(&pool, company_id).await;
    let reviewer_id = seed_user(&pool, company_id, "hr_manager").await;
    let payroll_user_id = seed_user(&pool, company_id, "payroll_admin").await;
    // Basic salary RM 4,000 -> RM 200/day at divisor 20 -> 0.5 day = RM 100 (10,000 sen)
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 400_000).await;

    let unpaid_type_id: Uuid = sqlx::query_scalar(
        "INSERT INTO leave_types (company_id, name, default_days, is_paid) VALUES ($1, $2, 0, FALSE) RETURNING id",
    )
    .bind(company_id)
    .bind(format!("Unpaid-{}", &Uuid::new_v4().to_string()[..8]))
    .fetch_one(&pool)
    .await
    .expect("seed unpaid leave type");

    let leave_date = NaiveDate::from_ymd_opt(2024, 6, 5).unwrap(); // Wednesday
    let request_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO leave_requests
           (employee_id, company_id, leave_type_id, start_date, end_date, days, status)
           VALUES ($1, $2, $3, $4, $4, 0.5, 'pending') RETURNING id"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(unpaid_type_id)
    .bind(leave_date)
    .fetch_one(&pool)
    .await
    .expect("seed half-day unpaid leave request");

    approval_service::approve_leave(
        &pool,
        &test_config(),
        company_id,
        request_id,
        Reviewer {
            user_id: reviewer_id,
            may_self_approve: false,
        },
        None,
        None,
    )
    .await
    .expect("approve half-day unpaid leave");

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        6,
        NaiveDate::from_ymd_opt(2024, 6, 28).unwrap(),
        payroll_user_id,
        None,
        None,
    )
    .await
    .expect("process payroll with half-day unpaid leave");

    let (deduction, days): (i64, Decimal) = sqlx::query_as(
        "SELECT unpaid_leave_deduction, unpaid_leave_days FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2",
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("read payroll item unpaid leave");

    assert_eq!(deduction, 10_000);
    assert_eq!(days, dec!(0.5));
}

#[tokio::test]
async fn portal_create_claim_rejects_zero_and_negative_amounts() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let user_id = seed_user(&pool, company_id, "employee").await;
    let employee_id = seed_employee(&pool, company_id, None, 300_000).await;
    let access = UploadAccess {
        company_id,
        user_id,
        employee_id: Some(employee_id),
        can_view_approvals: false,
        can_view_documents: false,
        can_view_confidential: false,
    };

    for invalid_amount in [0_i64, -500_i64] {
        let err = portal_service::create_claim(
            &pool,
            employee_id,
            company_id,
            user_id,
            CreateClaimRequest {
                title: "Parking".into(),
                description: None,
                amount: invalid_amount,
                category: None,
                receipt_url: None,
                receipt_file_name: None,
                expense_date: NaiveDate::from_ymd_opt(2024, 6, 10).unwrap(),
            },
            &access,
        )
        .await
        .expect_err("non-positive claim amount must be rejected");
        assert!(matches!(err, AppError::BadRequest(_)), "got {err:?}");
    }
}

#[tokio::test]
async fn company_and_team_updates_clear_optional_text_and_unpaid_leave_divisor() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let admin_id = seed_user(&pool, company_id, "admin").await;
    sqlx::query(
        "UPDATE companies SET phone = '03-12345678', unpaid_leave_divisor = 26 WHERE id = $1",
    )
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("seed phone and divisor");

    let access = UploadAccess {
        company_id,
        user_id: admin_id,
        employee_id: None,
        can_view_approvals: true,
        can_view_documents: true,
        can_view_confidential: true,
    };
    let updated_company = company_service::update_company(
        &pool,
        company_id,
        UpdateCompanyRequest {
            phone: Some("   ".into()),
            clear_unpaid_leave_divisor: Some(true),
            ..Default::default()
        },
        admin_id,
        &access,
        None,
    )
    .await
    .expect("update company with clears");

    assert!(updated_company.phone.is_none());
    assert!(updated_company.unpaid_leave_divisor.is_none());

    let team = team_service::create_team(
        &pool,
        company_id,
        &format!("Ops-{}", &Uuid::new_v4().to_string()[..8]),
        Some("Initial team description"),
        "general",
        admin_id,
        None,
    )
    .await
    .expect("create team with description");
    assert_eq!(
        team.description.as_deref(),
        Some("Initial team description")
    );

    let cleared_team = team_service::update_team(
        &pool,
        company_id,
        team.id,
        None,
        Some(""),
        None,
        None,
        admin_id,
        None,
    )
    .await
    .expect("clear team description");
    assert!(cleared_team.description.is_none());
}
