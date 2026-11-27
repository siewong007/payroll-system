//! Regression tests for database performance rewrites, concurrency locking,
//! constraint hardening, backup fidelity fields, and domain bug fixes.

use chrono::{Duration, NaiveDate, NaiveTime, Utc};
use rust_decimal_macros::dec;
use serde_json::json;
use uuid::Uuid;

use crate::core::auth::{AuthUser, Claims, JWT_AUDIENCE, JWT_ISSUER};
use crate::core::config::AppConfig;
use crate::core::error::AppError;
use crate::models::attendance::AttendanceSummaryQuery;
use crate::models::employee::UpdateEmployeeRequest;
use crate::models::portal::{CreateLeaveRequest, CreateOvertimeRequest};
use crate::models::user_company::UpdateUserRequest;
use crate::repositories::reads::{ea_form, payroll as payroll_reads, payslip};
use crate::repositories::{
    attendance_records, companies, employees, holidays, overtime_applications, payroll_runs,
    push_outbox,
};
use crate::services::approval_service::{self, Reviewer};
use crate::services::backup_service::{export_company, import_company};
use crate::services::{
    attendance_service, calendar_service, employee_service, payroll_engine,
    payroll_lifecycle_service, portal_service, upload_service, user_service,
};
use crate::tests::support::{
    seed_company, seed_employee, seed_payroll_group, seed_user, skip_if_no_db,
};

fn test_config() -> AppConfig {
    AppConfig {
        database_url: String::new(),
        jwt_secret: "test-secret-that-is-long-enough-for-tests".into(),
        totp_encryption_key: "db-perf-bugfix-test-totp-key-32b".into(),
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
        allow_unconfigured_turnstile: false,
        turnstile_verify_url: "https://challenges.cloudflare.com/turnstile/v0/siteverify".into(),
        vapid_public_key: None,
        vapid_private_key: None,
        vapid_subject: None,
    }
}

fn auth_for(user_id: Uuid, company_id: Option<Uuid>, roles: &[&str]) -> AuthUser {
    AuthUser(
        Claims {
            sub: user_id,
            email: format!("user-{user_id}@example.invalid"),
            roles: roles.iter().map(|r| (*r).to_string()).collect(),
            company_id,
            employee_id: None,
            sid: Uuid::new_v4(),
            exp: (Utc::now() + Duration::hours(1)).timestamp(),
            iat: Utc::now().timestamp(),
            iss: JWT_ISSUER.into(),
            aud: JWT_AUDIENCE.into(),
        },
        Vec::new(),
    )
}

#[tokio::test]
async fn tax_year_lock_and_cross_year_cancel_allowed() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let emp_id = seed_employee(&pool, company_id, Some(group_id), 450_000).await;
    let payroll_admin_id = seed_user(&pool, company_id, "payroll_admin").await;

    // Acquire tax-year lock cleanly inside a transaction.
    let mut tx = pool.begin().await.expect("begin tx");
    payroll_runs::acquire_tax_year_lock(&mut *tx, company_id, 2024)
        .await
        .expect("acquire_tax_year_lock");
    tx.commit().await.expect("commit tx");

    // Process Dec 2024 run.
    let dec_2024_run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        12,
        NaiveDate::from_ymd_opt(2024, 12, 28).unwrap(),
        payroll_admin_id,
        None,
        None,
    )
    .await
    .expect("process Dec 2024 payroll");

    // Insert a committed Jan 2025 payroll run + item (next tax year).
    let jan_2025_run_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO payroll_runs (
            id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status, processed_by, employee_count
        ) VALUES (
            $1, $2, $3, 2025, 1,
            DATE '2025-01-01', DATE '2025-01-31', DATE '2025-01-28',
            'paid'::payroll_status, $4, 1
        )
        "#,
    )
    .bind(jan_2025_run_id)
    .bind(company_id)
    .bind(group_id)
    .bind(payroll_admin_id)
    .execute(&pool)
    .await
    .expect("insert paid Jan 2025 run");

    sqlx::query(
        "INSERT INTO payroll_items (id, payroll_run_id, employee_id, basic_salary, gross_salary, net_salary) VALUES ($1, $2, $3, 450000, 450000, 400000)",
    )
    .bind(Uuid::new_v4())
    .bind(jan_2025_run_id)
    .bind(emp_id)
    .execute(&pool)
    .await
    .expect("insert Jan 2025 payroll item");

    // Cross-year later committed runs must NOT block Dec 2024 checks.
    assert!(
        !payroll_reads::run_has_later_committed_run(&pool, company_id, dec_2024_run.id)
            .await
            .expect("run_has_later_committed_run"),
        "Jan 2025 committed run must not block Dec 2024 run"
    );
    let later =
        payroll_reads::employees_with_later_committed_run(&pool, &[emp_id], company_id, 2024, 12)
            .await
            .expect("employees_with_later_committed_run");
    assert!(later.is_empty(), "Dec 2024 check is tax-year scoped");
    assert!(
        !payroll_reads::employee_has_later_run(&pool, emp_id, company_id, 2024, 12)
            .await
            .expect("employee_has_later_run"),
        "Jan 2025 run is in a different tax year"
    );

    // Cancelling Dec 2024 must succeed even though Jan 2025 is paid.
    let auth = auth_for(payroll_admin_id, Some(company_id), &["payroll_admin"]);
    let cancelled = payroll_lifecycle_service::cancel(
        &pool,
        company_id,
        dec_2024_run.id,
        &auth,
        Some("Cross-year adjustment".into()),
        None,
    )
    .await
    .expect("cancel Dec 2024 run despite Jan 2025 committed run");
    assert_eq!(cancelled.status, "cancelled");
}

#[tokio::test]
async fn payroll_staged_overtime_processed_without_double_count_and_monthly_allowance_included() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let emp_id = seed_employee(&pool, company_id, Some(group_id), 400_000).await;
    let payroll_admin_id = seed_user(&pool, company_id, "payroll_admin").await;

    let ot_entry_id = Uuid::new_v4();
    let ma_entry_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO payroll_entries (
            id, employee_id, company_id, period_year, period_month,
            category, item_type, description, amount, is_taxable, is_processed
        ) VALUES
            ($1, $2, $3, 2024, 6, 'earning', 'overtime', 'Staged attendance OT', 25000, TRUE, FALSE),
            ($4, $2, $3, 2024, 6, 'earning', 'monthly_allowance', 'Special Project Allowance', 30000, TRUE, FALSE)
        "#,
    )
    .bind(ot_entry_id)
    .bind(emp_id)
    .bind(company_id)
    .bind(ma_entry_id)
    .execute(&pool)
    .await
    .expect("stage entries");

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        6,
        NaiveDate::from_ymd_opt(2024, 6, 28).unwrap(),
        payroll_admin_id,
        None,
        None,
    )
    .await
    .expect("process payroll with staged entries");

    // Both entries should be marked processed and linked to run.
    let processed_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM payroll_entries WHERE id = ANY($1) AND is_processed = TRUE AND payroll_run_id = $2",
    )
    .bind(vec![ot_entry_id, ma_entry_id])
    .bind(run.id)
    .fetch_one(&pool)
    .await
    .expect("processed count");
    assert_eq!(processed_count, 2);

    // Total allowances includes monthly_allowance (30_000), while staged 'overtime' does not double-count into gross.
    let (gross, total_allowances, total_overtime): (i64, i64, i64) = sqlx::query_as(
        "SELECT gross_salary, total_allowances, total_overtime FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2",
    )
    .bind(run.id)
    .bind(emp_id)
    .fetch_one(&pool)
    .await
    .expect("fetch payroll item");

    assert_eq!(total_allowances, 30_000);
    assert_eq!(total_overtime, 0);
    assert_eq!(gross, 430_000);
}

#[tokio::test]
async fn ea_form_retains_soft_deleted_paid_employee_and_payslip_uses_bank_snapshot() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let emp_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let payroll_admin_id = seed_user(&pool, company_id, "payroll_admin").await;
    let finance_id = seed_user(&pool, company_id, "finance").await;

    sqlx::query(
        "UPDATE employees SET bank_name = 'Bank Islam', bank_account_number = 'OLD-ACCT-999', bank_account_type = 'savings' WHERE id = $1",
    )
    .bind(emp_id)
    .execute(&pool)
    .await
    .expect("set initial bank");

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        3,
        NaiveDate::from_ymd_opt(2024, 3, 28).unwrap(),
        payroll_admin_id,
        None,
        None,
    )
    .await
    .expect("process payroll");

    payroll_lifecycle_service::submit_for_approval(
        &pool,
        company_id,
        run.id,
        payroll_admin_id,
        None,
    )
    .await
    .expect("submit");
    payroll_lifecycle_service::approve(
        &pool,
        company_id,
        run.id,
        &auth_for(finance_id, Some(company_id), &["finance"]),
        None,
    )
    .await
    .expect("approve");
    payroll_lifecycle_service::lock_as_paid(&pool, company_id, run.id, finance_id, None)
        .await
        .expect("lock as paid");

    let payslip_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2",
    )
    .bind(run.id)
    .bind(emp_id)
    .fetch_one(&pool)
    .await
    .expect("payslip id");

    // Mutate employee bank details & soft-delete employee after payment.
    employee_service::update_employee(
        &pool,
        emp_id,
        company_id,
        UpdateEmployeeRequest {
            full_name: None,
            ic_number: None,
            passport_number: None,
            date_of_birth: None,
            gender: None,
            nationality: None,
            race: None,
            residency_status: None,
            marital_status: None,
            email: None,
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
            probation_start: None,
            probation_end: None,
            confirmation_date: None,
            date_resigned: None,
            resignation_reason: None,
            basic_salary: None,
            salary_change_type: None,
            salary_change_notes: None,
            hourly_rate: None,
            daily_rate: None,
            bank_name: Some("CIMB New".into()),
            bank_account_number: Some("NEW-ACCT-111".into()),
            bank_account_type: None,
            tax_identification_number: None,
            epf_number: None,
            socso_number: None,
            eis_number: None,
            working_spouse: None,
            num_children: None,
            epf_category: None,
            is_muslim: None,
            zakat_eligible: None,
            zakat_monthly_amount: None,
            ptptn_monthly_amount: None,
            tabung_haji_amount: None,
            hrdf_contribution: None,
            payroll_group_id: None,
            salary_group: None,
            is_active: None,
            clear_date_resigned: None,
            clear_epf_category: None,
            clear_payroll_group_id: None,
            clear_department: None,
            clear_designation: None,
            clear_cost_centre: None,
            clear_branch: None,
            clear_phone: None,
            clear_email: None,
            clear_bank_name: None,
            clear_bank_account_number: None,
            clear_probation_end: None,
            clear_confirmation_date: None,
            clear_salary_group: None,
        },
        payroll_admin_id,
        None,
    )
    .await
    .expect("update bank details");

    employee_service::soft_delete_employee(&pool, emp_id, company_id, payroll_admin_id, None)
        .await
        .expect("soft delete employee");

    let (ps_data, _co) = payslip::payslip_and_company_for_employee(&pool, payslip_id, emp_id)
        .await
        .expect("read payslip")
        .expect("payslip present");
    assert_eq!(ps_data.bank_name.as_deref(), Some("Bank Islam"));
    assert_eq!(ps_data.bank_account_number.as_deref(), Some("OLD-ACCT-999"));

    let ea_rows = ea_form::list_employee_summaries(&pool, company_id, 2024)
        .await
        .expect("list EA summaries");
    assert!(
        ea_rows.iter().any(|r| r.employee_id == emp_id),
        "EA summary list must retain soft-deleted employees who were paid during the tax year"
    );
}

#[tokio::test]
async fn year_end_carry_forward_carries_system_annual_leave_and_skips_replacement() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let emp_id = seed_employee(&pool, company_id, None, 400_000).await;

    let annual_lt = Uuid::new_v4();
    let repl_lt = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO leave_types (id, company_id, name, default_days, is_paid, is_active, max_carry_forward, carry_forward_expiry_months, is_system)
        VALUES
            ($1, $3, 'Annual Leave', 14, TRUE, TRUE, 5, 3, TRUE),
            ($2, $3, 'Replacement Leave', 0, TRUE, TRUE, 5, 6, TRUE)
        "#,
    )
    .bind(annual_lt)
    .bind(repl_lt)
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("insert leave types");

    sqlx::query(
        r#"
        INSERT INTO leave_balances (id, employee_id, leave_type_id, year, entitled_days, taken_days, pending_days, carried_forward)
        VALUES
            ($1, $3, $4, 2024, 14, 4, 0, 0),
            ($2, $3, $5, 2024, 4, 0, 0, 0)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(emp_id)
    .bind(annual_lt)
    .bind(repl_lt)
    .execute(&pool)
    .await
    .expect("insert 2024 balances");

    portal_service::process_year_end_carry_forward(&pool, company_id, 2024, 2025)
        .await
        .expect("process carry forward");

    let annual_cf: rust_decimal::Decimal = sqlx::query_scalar(
        "SELECT carried_forward FROM leave_balances WHERE employee_id = $1 AND leave_type_id = $2 AND year = 2025",
    )
    .bind(emp_id)
    .bind(annual_lt)
    .fetch_one(&pool)
    .await
    .expect("2025 annual balance");
    assert_eq!(annual_cf, dec!(5));

    let repl_cf: Option<rust_decimal::Decimal> = sqlx::query_scalar(
        "SELECT carried_forward FROM leave_balances WHERE employee_id = $1 AND leave_type_id = $2 AND year = 2025",
    )
    .bind(emp_id)
    .bind(repl_lt)
    .fetch_optional(&pool)
    .await
    .expect("check repl balance");
    assert_eq!(repl_cf.unwrap_or(dec!(0)), dec!(0));
}

#[tokio::test]
async fn overtime_overlap_detects_midnight_crossing() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let emp_id = seed_employee(&pool, company_id, None, 400_000).await;

    sqlx::query(
        r#"
        INSERT INTO overtime_applications (
            id, employee_id, company_id, ot_date, start_time, end_time, hours, ot_type, status
        ) VALUES ($1, $2, $3, DATE '2024-05-10', TIME '22:00:00', TIME '02:00:00', 4.0, 'normal', 'approved')
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(emp_id)
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("insert overnight OT");

    // 2024-05-11 01:00..03:00 overlaps the 22:00..02:00 (+1 day) window!
    let overlaps = overtime_applications::has_overlapping_window(
        &pool,
        emp_id,
        NaiveDate::from_ymd_opt(2024, 5, 11).unwrap(),
        NaiveTime::from_hms_opt(1, 0, 0).unwrap(),
        NaiveTime::from_hms_opt(3, 0, 0).unwrap(),
        None,
    )
    .await
    .expect("check overlap");
    assert!(overlaps, "must detect overlap across midnight");

    // 2024-05-11 03:00..05:00 does NOT overlap.
    let disjoint = overtime_applications::has_overlapping_window(
        &pool,
        emp_id,
        NaiveDate::from_ymd_opt(2024, 5, 11).unwrap(),
        NaiveTime::from_hms_opt(3, 0, 0).unwrap(),
        NaiveTime::from_hms_opt(5, 0, 0).unwrap(),
        None,
    )
    .await
    .expect("check disjoint");
    assert!(
        !disjoint,
        "adjacent non-overlapping window must not conflict"
    );
}

#[tokio::test]
async fn reviewer_may_self_approve_respected() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let cfg = test_config();
    let company_id = seed_company(&pool).await;
    let emp_id = seed_employee(&pool, company_id, None, 400_000).await;
    sqlx::query("UPDATE employees SET email = 'emp-self@example.invalid' WHERE id = $1")
        .bind(emp_id)
        .execute(&pool)
        .await
        .expect("set emp email");

    let hr_user_id = seed_user(&pool, company_id, "hr_manager").await;
    let sa_user_id = seed_user(&pool, company_id, "super_admin").await;

    let ot_for_hr = portal_service::create_overtime_application(
        &pool,
        emp_id,
        company_id,
        hr_user_id,
        CreateOvertimeRequest {
            ot_date: NaiveDate::from_ymd_opt(2026, 3, 2).unwrap(),
            start_time: "18:00".into(),
            end_time: "20:00".into(),
            hours: dec!(2),
            ot_type: Some("normal".into()),
            reason: None,
        },
    )
    .await
    .expect("create OT 1");

    // Regular HR reviewer cannot approve a request they created (`may_self_approve: false`).
    let err = approval_service::approve_overtime(
        &pool,
        company_id,
        ot_for_hr.id,
        Reviewer {
            user_id: hr_user_id,
            may_self_approve: false,
        },
        None,
        None,
    )
    .await;
    assert!(matches!(err, Err(AppError::Forbidden(_))));

    // Leave request created by super_admin can be approved when `may_self_approve: true`.
    let lt_id = Uuid::new_v4();
    sqlx::query("INSERT INTO leave_types (id, company_id, name, default_days, is_paid) VALUES ($1, $2, 'Annual Leave', 14, TRUE)")
        .bind(lt_id)
        .bind(company_id)
        .execute(&pool)
        .await
        .expect("insert lt");

    sqlx::query(
        "INSERT INTO leave_balances (id, employee_id, leave_type_id, year, entitled_days) VALUES ($1, $2, $3, 2026, 14)",
    )
    .bind(Uuid::new_v4())
    .bind(emp_id)
    .bind(lt_id)
    .execute(&pool)
    .await
    .expect("seed leave balance");

    let access = upload_service::access_from_auth(
        &auth_for(sa_user_id, Some(company_id), &["super_admin"]),
        company_id,
    );
    let leave = approval_service::create_leave_request_admin(
        &pool,
        company_id,
        emp_id,
        CreateLeaveRequest {
            leave_type_id: lt_id,
            start_date: NaiveDate::from_ymd_opt(2026, 3, 18).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2026, 3, 18).unwrap(),
            days: dec!(1),
            reason: Some("Test".into()),
            attachment_url: None,
            attachment_name: None,
        },
        sa_user_id,
        &access,
        None,
    )
    .await
    .expect("create leave");

    let approved = approval_service::approve_leave(
        &pool,
        &cfg,
        company_id,
        leave.id,
        Reviewer {
            user_id: sa_user_id,
            may_self_approve: true,
        },
        None,
        None,
    )
    .await
    .expect("override reviewer may self-approve");
    assert_eq!(approved.status, "approved");
}

#[tokio::test]
async fn attendance_sargable_today_and_summary_range_validation() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let emp_id = seed_employee(&pool, company_id, None, 400_000).await;

    assert!(
        !attendance_records::exists_active_checkin_today(&pool, emp_id, "Asia/Kuala_Lumpur")
            .await
            .expect("exists_active_checkin_today before insert")
    );
    assert!(
        attendance_records::get_today(&pool, emp_id, "Asia/Kuala_Lumpur")
            .await
            .expect("get_today before insert")
            .is_none()
    );

    // Insert an open check-in right now + test summary date guards.
    sqlx::query(
        r#"
        INSERT INTO attendance_records (id, company_id, employee_id, check_in_at, method, status)
        VALUES ($1, $2, $3, NOW(), 'qr_code', 'present')
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(company_id)
    .bind(emp_id)
    .execute(&pool)
    .await
    .expect("insert today checkin");

    assert!(
        attendance_records::exists_active_checkin_today(&pool, emp_id, "Asia/Kuala_Lumpur")
            .await
            .expect("exists_active_checkin_today after insert")
    );
    assert!(
        attendance_records::get_today(&pool, emp_id, "Asia/Kuala_Lumpur")
            .await
            .expect("get_today after insert")
            .is_some()
    );

    // Real check-in must NOT be deleted by delete_auto_absent_today.
    let deleted = attendance_records::delete_auto_absent_today(&pool, emp_id, "Asia/Kuala_Lumpur")
        .await
        .expect("delete_auto_absent_today");
    assert_eq!(deleted, 0);

    // Bad range: from > to
    let rev_err = attendance_service::get_attendance_summary(
        &pool,
        company_id,
        &AttendanceSummaryQuery {
            date_from: NaiveDate::from_ymd_opt(2026, 5, 10).unwrap(),
            date_to: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            employee_id: None,
            department: None,
        },
    )
    .await;
    assert!(matches!(rev_err, Err(AppError::BadRequest(_))));

    // Bad range: span > 366 days
    let span_err = attendance_service::get_attendance_summary(
        &pool,
        company_id,
        &AttendanceSummaryQuery {
            date_from: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            date_to: NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
            employee_id: None,
            department: None,
        },
    )
    .await;
    assert!(matches!(span_err, Err(AppError::BadRequest(_))));
}

#[tokio::test]
async fn holidays_recurring_span_and_working_days_limit() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    sqlx::query(
        r#"
        INSERT INTO holidays (id, company_id, name, date, holiday_type, is_recurring)
        VALUES ($1, $2, 'Founders Anniversary', DATE '2020-01-15', 'public_holiday', TRUE)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("insert recurring holiday");

    let list = holidays::list_for_range(
        &pool,
        company_id,
        NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
        NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
    )
    .await
    .expect("list holidays in 2026 span");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Founders Anniversary");

    // >7 entries must be rejected.
    let too_many = vec![(0, true); 8];
    let err = calendar_service::update_working_days(&pool, company_id, &too_many).await;
    assert!(matches!(err, Err(AppError::BadRequest(_))));

    // Duplicate day entries are explicitly rejected.
    let dup_err = calendar_service::update_working_days(
        &pool,
        company_id,
        &[(1, false), (1, true), (2, true)],
    )
    .await;
    assert!(matches!(dup_err, Err(AppError::BadRequest(_))));

    // Valid distinct day entries succeed.
    let updated = calendar_service::update_working_days(&pool, company_id, &[(1, true), (2, true)])
        .await
        .expect("update valid working days");
    assert_eq!(updated.len(), 2);
}

#[tokio::test]
async fn employee_soft_delete_idempotent_and_revokes_mfa_and_push() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let emp_id = seed_employee(&pool, company_id, None, 400_000).await;
    let admin_id = seed_user(&pool, company_id, "admin").await;
    let emp_user_id = seed_user(&pool, company_id, "employee").await;

    sqlx::query("UPDATE users SET employee_id = $2 WHERE id = $1")
        .bind(emp_user_id)
        .bind(emp_id)
        .execute(&pool)
        .await
        .expect("link employee user");

    let jti = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO mfa_challenges (jti, user_id, expires_at) VALUES ($1, $2, NOW() + INTERVAL '5 minutes')",
    )
    .bind(jti)
    .bind(emp_user_id)
    .execute(&pool)
    .await
    .expect("insert mfa challenge");

    let sub_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO push_subscriptions (id, user_id, company_id, endpoint, p256dh, auth)
        VALUES ($1, $2, $3, $4, 'p256-key', 'auth-key')
        "#,
    )
    .bind(sub_id)
    .bind(emp_user_id)
    .bind(company_id)
    .bind(format!("https://fcm.googleapis.com/fcm/send/{sub_id}"))
    .execute(&pool)
    .await
    .expect("insert push sub");

    employee_service::soft_delete_employee(&pool, emp_id, company_id, admin_id, None)
        .await
        .expect("soft delete employee");

    let mfa_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM mfa_challenges WHERE jti = $1)")
            .bind(jti)
            .fetch_one(&pool)
            .await
            .expect("check mfa challenge revoked");
    assert!(
        !mfa_exists,
        "mfa_challenges row must be deleted on soft-delete"
    );

    let push_revoked: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT revoked_at FROM push_subscriptions WHERE id = $1")
            .bind(sub_id)
            .fetch_one(&pool)
            .await
            .expect("push revoked_at");
    assert!(push_revoked.is_some());

    // Repeated low-level soft_delete returns 0 rows.
    let second = employees::soft_delete(&pool, emp_id, company_id)
        .await
        .expect("idempotent soft_delete");
    assert_eq!(second, 0);
}

#[tokio::test]
async fn push_outbox_skips_inactive_and_flags_mismatched_and_purges_stale() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let active_user = seed_user(&pool, company_id, "employee").await;
    let inactive_user = seed_user(&pool, company_id, "employee").await;

    let sub_active = Uuid::new_v4();
    let sub_inactive = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO push_subscriptions (id, user_id, company_id, endpoint, p256dh, auth)
        VALUES
            ($1, $2, $3, $4, 'k1', 'a1'),
            ($5, $6, $3, $7, 'k2', 'a2')
        "#,
    )
    .bind(sub_active)
    .bind(active_user)
    .bind(company_id)
    .bind(format!("https://fcm.googleapis.com/fcm/send/{sub_active}"))
    .bind(sub_inactive)
    .bind(inactive_user)
    .bind(format!(
        "https://fcm.googleapis.com/fcm/send/{sub_inactive}"
    ))
    .execute(&pool)
    .await
    .expect("insert push subs");

    sqlx::query("UPDATE users SET is_active = FALSE WHERE id = $1")
        .bind(inactive_user)
        .execute(&pool)
        .await
        .expect("deactivate user");

    let n_active = Uuid::new_v4();
    let n_inactive = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO notifications (id, user_id, company_id, notification_type, title, message)
        VALUES
            ($1, $2, $3, 'system', 'Hi', 'Msg'),
            ($4, $5, $3, 'system', 'Hi', 'Msg')
        "#,
    )
    .bind(n_active)
    .bind(active_user)
    .bind(company_id)
    .bind(n_inactive)
    .bind(inactive_user)
    .execute(&pool)
    .await
    .expect("insert notifications");

    let enqueued = push_outbox::enqueue_for_notifications(
        &pool,
        company_id,
        &[n_active, n_inactive],
        "Test",
        "Body",
        "/portal/notifications",
        None,
    )
    .await
    .expect("enqueue_for_notifications");
    assert_eq!(
        enqueued, 1,
        "only active user's notification should enqueue"
    );

    // Stale pending row older than 30 days should be purged by purge_old(30).
    let stale_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO push_outbox (id, subscription_id, user_id, company_id, title, body, status, created_at)
        VALUES ($1, $2, $3, $4, 'Old', 'Stale pending', 'pending', NOW() - INTERVAL '45 days')
        "#,
    )
    .bind(stale_id)
    .bind(sub_active)
    .bind(active_user)
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("insert stale pending outbox");

    push_outbox::purge_old(&pool, 30).await.expect("purge_old");

    let still_there: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM push_outbox WHERE id = $1)")
            .bind(stale_id)
            .fetch_one(&pool)
            .await
            .expect("check stale outbox deleted");
    assert!(!still_there, "stale pending row must be purged");
}

#[tokio::test]
async fn inactive_super_admin_deletion_and_company_delete_cleans_uploads_and_push_outbox() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let active_sa = seed_user(&pool, company_id, "super_admin").await;
    let inactive_sa = seed_user(&pool, company_id, "super_admin").await;

    sqlx::query("UPDATE users SET is_active = FALSE WHERE id = $1")
        .bind(inactive_sa)
        .execute(&pool)
        .await
        .expect("deactivate sa");

    // Reactivating / updating an inactive non-last SA succeeds cleanly.
    user_service::update_user(
        &pool,
        inactive_sa,
        UpdateUserRequest {
            full_name: Some("Updated Inactive SA".into()),
            email: None,
            roles: Some(vec!["admin".into()]),
            is_active: None,
            company_ids: None,
            actor_password: None,
            actor_totp_code: None,
        },
        active_sa,
        None,
    )
    .await
    .expect("downgrade inactive super_admin");

    // Create another inactive super_admin and delete it directly.
    let second_inactive_sa = seed_user(&pool, company_id, "super_admin").await;
    sqlx::query("UPDATE users SET is_active = FALSE WHERE id = $1")
        .bind(second_inactive_sa)
        .execute(&pool)
        .await
        .expect("deactivate second sa");

    user_service::delete_user(&pool, second_inactive_sa, active_sa, None)
        .await
        .expect("delete inactive super_admin");

    // Seed uploaded_files + push_outbox for company_id and verify delete_company_data removes them.
    let emp_u = seed_user(&pool, company_id, "employee").await;
    let sub_id = Uuid::new_v4();
    let stored_name = format!("deltest-{company_id}.pdf");
    sqlx::query(
        "INSERT INTO uploaded_files (stored_name, company_id, uploaded_by) VALUES ($1, $2, $3)",
    )
    .bind(&stored_name)
    .bind(company_id)
    .bind(emp_u)
    .execute(&pool)
    .await
    .expect("insert uploaded file");

    sqlx::query("INSERT INTO push_subscriptions (id, user_id, company_id, endpoint, p256dh, auth) VALUES ($1, $2, $3, $4, 'k', 'a')")
        .bind(sub_id)
        .bind(emp_u)
        .bind(company_id)
        .bind(format!("https://fcm.googleapis.com/fcm/send/{sub_id}"))
        .execute(&pool)
        .await
        .expect("insert sub");

    sqlx::query("INSERT INTO push_outbox (id, subscription_id, user_id, company_id, title, body) VALUES ($1, $2, $3, $4, 'T', 'B')")
        .bind(Uuid::new_v4())
        .bind(sub_id)
        .bind(emp_u)
        .bind(company_id)
        .execute(&pool)
        .await
        .expect("insert push_outbox");

    let mut tx = pool.begin().await.expect("begin tx");
    let deleted_uploads = companies::delete_company_data(&mut tx, company_id)
        .await
        .expect("delete_company_data");
    assert!(deleted_uploads.contains(&stored_name));
    tx.commit().await.expect("commit tx");

    let rem_uploads: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM uploaded_files WHERE stored_name = $1")
            .bind(&stored_name)
            .fetch_one(&pool)
            .await
            .expect("check uploads deleted");
    let rem_outbox: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM push_outbox WHERE subscription_id = $1")
            .bind(sub_id)
            .fetch_one(&pool)
            .await
            .expect("check outbox deleted");
    assert_eq!(rem_uploads, 0);
    assert_eq!(rem_outbox, 0);
}

#[tokio::test]
async fn leave_balances_non_negative_constraint_enforced() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let emp_id = seed_employee(&pool, company_id, None, 400_000).await;
    let lt_id = Uuid::new_v4();
    sqlx::query("INSERT INTO leave_types (id, company_id, name, default_days, is_paid) VALUES ($1, $2, 'Annual Leave', 14, TRUE)")
        .bind(lt_id)
        .bind(company_id)
        .execute(&pool)
        .await
        .expect("insert leave type");

    let res = sqlx::query(
        r#"
        INSERT INTO leave_balances (id, employee_id, leave_type_id, year, entitled_days, taken_days, pending_days, carried_forward)
        VALUES ($1, $2, $3, 2026, 14, -1, 0, 0)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(emp_id)
    .bind(lt_id)
    .execute(&pool)
    .await;
    assert!(
        res.is_err(),
        "leave_balances_non_negative_check must reject negative balances"
    );
}

#[tokio::test]
async fn backup_roundtrip_preserves_network_mode_snapshot_and_entry_links() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let src_company = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO companies (id, name, is_active, attendance_network_mode, auto_absent_last_run_date)
         VALUES ($1, $2, FALSE, 'enforce', DATE '2024-04-20')",
    )
    .bind(src_company)
    .bind(format!("BackupSrc-{}", &src_company.to_string()[..8]))
    .execute(&pool)
    .await
    .expect("insert inactive src company");

    let dst_company = Uuid::new_v4();
    sqlx::query("INSERT INTO companies (id, name, is_active) VALUES ($1, $2, FALSE)")
        .bind(dst_company)
        .bind(format!("BackupDst-{}", &dst_company.to_string()[..8]))
        .execute(&pool)
        .await
        .expect("insert inactive dst company");

    let group_id = seed_payroll_group(&pool, src_company).await;
    let emp_id = seed_employee(&pool, src_company, Some(group_id), 450_000).await;
    let admin_id = seed_user(&pool, src_company, "payroll_admin").await;
    let dst_admin_id = seed_user(&pool, dst_company, "admin").await;

    let lt_id = Uuid::new_v4();
    sqlx::query("INSERT INTO leave_types (id, company_id, name, default_days, is_paid) VALUES ($1, $2, 'Unpaid Leave', 30, FALSE)")
        .bind(lt_id)
        .bind(src_company)
        .execute(&pool)
        .await
        .expect("insert lt");

    let lr_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO leave_requests (id, employee_id, company_id, leave_type_id, start_date, end_date, days, status)
        VALUES ($1, $2, $3, $4, DATE '2024-04-10', DATE '2024-04-10', 1, 'approved')
        "#,
    )
    .bind(lr_id)
    .bind(emp_id)
    .bind(src_company)
    .bind(lt_id)
    .execute(&pool)
    .await
    .expect("insert lr");

    let ot_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO overtime_applications (id, employee_id, company_id, ot_date, start_time, end_time, hours, ot_type, status)
        VALUES ($1, $2, $3, DATE '2024-04-11', TIME '18:00:00', TIME '20:00:00', 2, 'normal', 'approved')
        "#,
    )
    .bind(ot_id)
    .bind(emp_id)
    .bind(src_company)
    .execute(&pool)
    .await
    .expect("insert ot");

    let pe_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO payroll_entries (
            id, employee_id, company_id, period_year, period_month,
            category, item_type, description, amount, is_processed,
            overtime_application_id, leave_request_id
        ) VALUES ($1, $2, $3, 2024, 4, 'earning', 'bonus', 'Linked Entry', 10000, FALSE, $4, $5)
        "#,
    )
    .bind(pe_id)
    .bind(emp_id)
    .bind(src_company)
    .bind(ot_id)
    .bind(lr_id)
    .execute(&pool)
    .await
    .expect("insert linked entry");

    let run = payroll_engine::process_payroll(
        &pool,
        src_company,
        group_id,
        2024,
        4,
        NaiveDate::from_ymd_opt(2024, 4, 28).unwrap(),
        admin_id,
        Some("Snapshot roundtrip".into()),
        None,
    )
    .await
    .expect("process run");

    let snap = json!({"source": "backup_roundtrip_test", "rule_year": 2024});
    sqlx::query("UPDATE payroll_runs SET calculation_snapshot = $2 WHERE id = $1")
        .bind(run.id)
        .bind(&snap)
        .execute(&pool)
        .await
        .expect("set snapshot");

    let backup = export_company(&pool, src_company)
        .await
        .expect("export company");
    assert_eq!(
        backup.company.attendance_network_mode.as_deref(),
        Some("enforce")
    );
    assert_eq!(
        backup.company.auto_absent_last_run_date,
        Some(NaiveDate::from_ymd_opt(2024, 4, 20).unwrap())
    );
    assert!(
        backup
            .payroll_runs
            .iter()
            .any(|r| r.calculation_snapshot.as_ref() == Some(&snap))
    );
    assert!(
        backup
            .payroll_entries
            .iter()
            .any(|e| e.overtime_application_id == Some(ot_id) && e.leave_request_id == Some(lr_id))
    );

    import_company(&pool, backup, Some(dst_company), dst_admin_id, None)
        .await
        .expect("import company");

    let (dst_net_mode, dst_bookmark): (String, Option<NaiveDate>) = sqlx::query_as(
        "SELECT attendance_network_mode, auto_absent_last_run_date FROM companies WHERE id = $1",
    )
    .bind(dst_company)
    .fetch_one(&pool)
    .await
    .expect("read dst company");
    assert_eq!(dst_net_mode, "enforce");
    assert_eq!(
        dst_bookmark,
        Some(NaiveDate::from_ymd_opt(2024, 4, 20).unwrap())
    );

    let dst_snap: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT calculation_snapshot FROM payroll_runs WHERE company_id = $1 LIMIT 1",
    )
    .bind(dst_company)
    .fetch_one(&pool)
    .await
    .expect("read dst snapshot");
    assert_eq!(dst_snap, Some(snap));

    let (dst_ot, dst_lr): (Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT overtime_application_id, leave_request_id FROM payroll_entries WHERE company_id = $1 LIMIT 1",
    )
    .bind(dst_company)
    .fetch_one(&pool)
    .await
    .expect("read dst entry links");
    assert!(dst_ot.is_some());
    assert!(dst_lr.is_some());
}

#[tokio::test]
async fn monthly_allowance_staged_entry_paid_and_money_bounds_enforced() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    use crate::models::payroll::{MAX_MONEY_SEN, checked_round_sen};
    use rust_decimal::Decimal;

    assert!(checked_round_sen(Decimal::from(i64::MIN)).is_err());
    assert!(checked_round_sen(Decimal::from(MAX_MONEY_SEN + 1)).is_err());
    assert!(checked_round_sen(Decimal::from(-MAX_MONEY_SEN - 1)).is_err());
    assert!(checked_round_sen(Decimal::MAX).is_err());
    assert_eq!(
        checked_round_sen(Decimal::from(MAX_MONEY_SEN)).unwrap(),
        MAX_MONEY_SEN
    );
    assert_eq!(
        checked_round_sen(Decimal::from(-MAX_MONEY_SEN)).unwrap(),
        -MAX_MONEY_SEN
    );

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let emp_id = seed_employee(&pool, company_id, Some(group_id), 400_000).await;
    let admin_id = seed_user(&pool, company_id, "payroll_admin").await;

    sqlx::query(
        r#"INSERT INTO payroll_entries (
            id, employee_id, company_id, period_year, period_month,
            category, item_type, description, amount, is_taxable, is_processed, created_by
        ) VALUES ($1, $2, $3, 2024, 4, 'earning', 'monthly_allowance', 'Meal Allowance', 25000, TRUE, FALSE, $4)"#,
    )
    .bind(Uuid::new_v4())
    .bind(emp_id)
    .bind(company_id)
    .bind(admin_id)
    .execute(&pool)
    .await
    .expect("stage monthly_allowance");

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        4,
        NaiveDate::from_ymd_opt(2024, 4, 28).unwrap(),
        admin_id,
        None,
        None,
    )
    .await
    .expect("process payroll with monthly_allowance");

    let total_allowances: i64 = sqlx::query_scalar(
        "SELECT total_allowances FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2",
    )
    .bind(run.id)
    .bind(emp_id)
    .fetch_one(&pool)
    .await
    .expect("read total_allowances");
    assert_eq!(total_allowances, 25_000);
}

#[tokio::test]
async fn attendance_network_observations_allow_zero_to_one_transitions_within_12h() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    use crate::repositories::attendance_network_observations as obs;

    let company_id = seed_company(&pool).await;
    let emp_id = seed_employee(&pool, company_id, None, 350_000).await;

    // 1. Denial -> unanchored observation -> anchored observation within same second.
    obs::record_denial(&pool, company_id, emp_id, "203.0.113.0", 24)
        .await
        .expect("record denial");
    obs::record(&pool, company_id, emp_id, "203.0.113.0", 24, false)
        .await
        .expect("record 0->1 observation");
    obs::record(&pool, company_id, emp_id, "203.0.113.0", 24, true)
        .await
        .expect("record 0->1 anchored");

    // Repeating each within the 12h window must keep all three at 1.
    obs::record_denial(&pool, company_id, emp_id, "203.0.113.0", 24)
        .await
        .expect("repeat denial");
    obs::record(&pool, company_id, emp_id, "203.0.113.0", 24, true)
        .await
        .expect("repeat anchored");

    let counts: (i32, i32, i32) = sqlx::query_as(
        "SELECT observation_count, anchored_count, denied_count FROM attendance_network_observations WHERE company_id = $1 AND employee_id = $2",
    )
    .bind(company_id)
    .bind(emp_id)
    .fetch_one(&pool)
    .await
    .expect("read observation counts");
    assert_eq!(counts, (1, 1, 1));
}

#[tokio::test]
async fn audit_and_trigram_indexes_are_planner_usable() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    fn collect_index_names(val: &serde_json::Value, out: &mut Vec<String>) {
        match val {
            serde_json::Value::Object(map) => {
                if let Some(serde_json::Value::String(name)) = map.get("Index Name") {
                    out.push(name.clone());
                }
                for child in map.values() {
                    collect_index_names(child, out);
                }
            }
            serde_json::Value::Array(arr) => {
                for child in arr {
                    collect_index_names(child, out);
                }
            }
            _ => {}
        }
    }

    let company_id = seed_company(&pool).await;
    let run_id_str = Uuid::new_v4().to_string();

    let mut tx = pool.begin().await.expect("begin tx");
    sqlx::query("SET LOCAL enable_seqscan = off")
        .execute(&mut *tx)
        .await
        .expect("disable seqscan");

    let plan_new: serde_json::Value = sqlx::query_scalar(
        "EXPLAIN (FORMAT JSON) SELECT id FROM audit_logs WHERE company_id = $1 AND entity_type = 'payroll_item' AND new_values->>'payroll_run_id' = $2",
    )
    .bind(company_id)
    .bind(&run_id_str)
    .fetch_one(&mut *tx)
    .await
    .expect("explain audit new");
    let mut new_idxs = Vec::new();
    collect_index_names(&plan_new, &mut new_idxs);
    assert!(
        new_idxs
            .iter()
            .any(|n| n == "idx_audit_logs_payroll_item_run_new"),
        "expected idx_audit_logs_payroll_item_run_new, got {new_idxs:?} in {plan_new}"
    );

    let plan_old: serde_json::Value = sqlx::query_scalar(
        "EXPLAIN (FORMAT JSON) SELECT id FROM audit_logs WHERE company_id = $1 AND entity_type = 'payroll_item' AND old_values->>'payroll_run_id' = $2",
    )
    .bind(company_id)
    .bind(&run_id_str)
    .fetch_one(&mut *tx)
    .await
    .expect("explain audit old");
    let mut old_idxs = Vec::new();
    collect_index_names(&plan_old, &mut old_idxs);
    assert!(
        old_idxs
            .iter()
            .any(|n| n == "idx_audit_logs_payroll_item_run_old"),
        "expected idx_audit_logs_payroll_item_run_old, got {old_idxs:?} in {plan_old}"
    );

    // Seed 5,000 rows so the GIN trigram bitmap scan (~56-60 cost) beats
    // `idx_documents_company` (~260 cost) even under PostgreSQL's default
    // `random_page_cost = 4.0` used in CI (with 1,000 rows, B-tree ~52.65 wins).
    sqlx::query(
        r#"INSERT INTO documents (company_id, title, file_name, file_url, status)
           SELECT $1, 'Doc ' || g, 'file_' || g || '.pdf', '/api/uploads/doc-' || g || '.pdf', 'active'
           FROM generate_series(1, 5000) g"#,
    )
    .bind(company_id)
    .execute(&mut *tx)
    .await
    .expect("seed documents for trigram explain");
    sqlx::query("SELECT gin_clean_pending_list('idx_documents_search_trgm'::regclass)")
        .execute(&mut *tx)
        .await
        .expect("flush documents trigram pending list");
    sqlx::query("ANALYZE documents")
        .execute(&mut *tx)
        .await
        .expect("analyze documents");

    let plan_docs: serde_json::Value = sqlx::query_scalar(
        r#"EXPLAIN (FORMAT JSON) SELECT id FROM documents WHERE company_id = $1 AND deleted_at IS NULL AND lower((title::text || ' ' || file_name::text)) LIKE '%' || lower($2) || '%' ESCAPE '\'"#,
    )
    .bind(company_id)
    .bind("contract")
    .fetch_one(&mut *tx)
    .await
    .expect("explain documents trgm");
    let mut docs_idxs = Vec::new();
    collect_index_names(&plan_docs, &mut docs_idxs);
    assert!(
        docs_idxs.iter().any(|n| n == "idx_documents_search_trgm"),
        "expected idx_documents_search_trgm, got {docs_idxs:?} in {plan_docs}"
    );

    sqlx::query(
        r#"INSERT INTO users (email, password_hash, full_name, roles, company_id)
           SELECT 'trgm-user-' || g || '-' || $1::text || '@example.invalid', 'x', 'Trigram User ' || g, ARRAY['employee']::varchar(50)[], $1
           FROM generate_series(1, 1000) g"#,
    )
    .bind(company_id)
    .execute(&mut *tx)
    .await
    .expect("seed users for trigram explain");
    sqlx::query("SELECT gin_clean_pending_list('idx_users_search_trgm'::regclass)")
        .execute(&mut *tx)
        .await
        .expect("flush users trigram pending list");
    sqlx::query("ANALYZE users")
        .execute(&mut *tx)
        .await
        .expect("analyze users");

    let plan_users: serde_json::Value = sqlx::query_scalar(
        r#"EXPLAIN (FORMAT JSON) SELECT id FROM users WHERE deleted_at IS NULL AND lower((full_name::text || ' ' || email::text)) LIKE '%' || lower($1) || '%' ESCAPE '\'"#,
    )
    .bind("alice")
    .fetch_one(&mut *tx)
    .await
    .expect("explain users trgm");
    let mut users_idxs = Vec::new();
    collect_index_names(&plan_users, &mut users_idxs);
    assert!(
        users_idxs.iter().any(|n| n == "idx_users_search_trgm"),
        "expected idx_users_search_trgm, got {users_idxs:?} in {plan_users}"
    );

    tx.rollback().await.expect("rollback explain tx");
}
