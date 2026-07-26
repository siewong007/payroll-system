use chrono::NaiveDate;
use rust_decimal_macros::dec;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::config::AppConfig;
use crate::core::error::AppError;
use crate::models::portal::{CreateOvertimeRequest, UpdateOvertimeRequest};
use crate::services::approval_service::{self, Reviewer};
use crate::services::{payroll_engine, payroll_service};
use crate::tests::support::{
    seed_company, seed_employee, seed_payroll_group, seed_user, skip_if_no_db,
};

/// SMTP and OAuth left off, so the notification and email tails of the approval
/// services are no-ops rather than network calls.
fn test_config() -> AppConfig {
    AppConfig {
        database_url: String::new(),
        jwt_secret: "test-secret-that-is-long-enough-for-tests".into(),
        jwt_expiry_hours: 1,
        server_host: "0.0.0.0".into(),
        server_port: 8080,
        frontend_url: "http://localhost:5173".into(),
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
    }
}

/// `seed_employee` leaves `email` NULL, and the approval services read it to
/// send the notification email (`e.email AS "email!"`), so anything that drives
/// a real approval needs one.
async fn seed_employee_with_email(
    pool: &PgPool,
    company_id: Uuid,
    group_id: Option<Uuid>,
    basic_salary_sen: i64,
) -> Uuid {
    let employee_id = seed_employee(pool, company_id, group_id, basic_salary_sen).await;
    sqlx::query("UPDATE employees SET email = $2 WHERE id = $1")
        .bind(employee_id)
        .bind(format!(
            "emp-{}@example.invalid",
            &employee_id.to_string()[..8]
        ))
        .execute(pool)
        .await
        .expect("set employee email");
    employee_id
}

/// Link a user account to an employee record — the link the maker-checker guard
/// reads. Deliberately applied *after* the user row exists, which is exactly the
/// case a token-sourced `employee_id` would miss.
async fn link_user_to_employee(pool: &PgPool, user_id: Uuid, employee_id: Uuid) {
    sqlx::query("UPDATE users SET employee_id = $2 WHERE id = $1")
        .bind(user_id)
        .bind(employee_id)
        .execute(pool)
        .await
        .expect("link user to employee");
}

async fn seed_claim(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    amount: i64,
    expense_date: NaiveDate,
    status: &str,
) -> Uuid {
    let claim_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO claims
           (id, employee_id, company_id, title, amount, expense_date, status, submitted_at)
           VALUES ($1, $2, $3, 'Taxi fare', $4, $5, $6, NOW())"#,
    )
    .bind(claim_id)
    .bind(employee_id)
    .bind(company_id)
    .bind(amount)
    .bind(expense_date)
    .bind(status)
    .execute(pool)
    .await
    .expect("insert claim");
    claim_id
}

async fn claim_state(pool: &PgPool, claim_id: Uuid) -> (String, Option<Uuid>) {
    sqlx::query_as("SELECT status, payroll_run_id FROM claims WHERE id = $1")
        .bind(claim_id)
        .fetch_one(pool)
        .await
        .expect("read claim state")
}

async fn total_claims_for(pool: &PgPool, run_id: Uuid, employee_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT total_claims FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2",
    )
    .bind(run_id)
    .bind(employee_id)
    .fetch_one(pool)
    .await
    .expect("read total_claims")
}

async fn run_payroll(
    pool: &PgPool,
    company_id: Uuid,
    group_id: Uuid,
    year: i32,
    month: u32,
    user_id: Uuid,
) -> crate::models::payroll::PayrollRun {
    let pay_date = NaiveDate::from_ymd_opt(year, month, 28).expect("valid pay date");
    payroll_engine::process_payroll(
        pool,
        company_id,
        group_id,
        year,
        month as i32,
        pay_date,
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll")
}

/// An `approved` claim with an `expense_date` inside the payroll period must
/// flow through to `payroll_items.total_claims` and get marked `processed`.
#[tokio::test]
async fn approved_claim_flows_into_payroll_item() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    // Approved claim for RM150 with expense_date inside April 2024.
    let claim_id = uuid::Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO claims
           (id, employee_id, company_id, title, amount, expense_date, status)
           VALUES ($1, $2, $3, 'Taxi fare', 15000, $4, 'approved')"#,
    )
    .bind(claim_id)
    .bind(employee_id)
    .bind(company_id)
    .bind(NaiveDate::from_ymd_opt(2024, 4, 10).unwrap())
    .execute(&pool)
    .await
    .expect("insert claim");

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        4,
        NaiveDate::from_ymd_opt(2024, 5, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    let (total_claims, net_salary, gross_salary, total_deductions): (i64, i64, i64, i64) =
        sqlx::query_as(
            r#"SELECT total_claims, net_salary, gross_salary, total_deductions
               FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2"#,
        )
        .bind(run.id)
        .bind(employee_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(
        total_claims, 15_000,
        "claim amount must appear in total_claims"
    );
    // Claims are reimbursements — they add to net, not gross.
    assert_eq!(gross_salary, 500_000, "gross should not include claims");
    assert_eq!(
        net_salary,
        gross_salary - total_deductions + total_claims,
        "net = gross - deductions + reimbursable claims"
    );

    // The claim row must be marked processed AND linked to the run that paid
    // it. The link is what makes "not yet paid" expressible and what a run
    // delete reverts by; status alone cannot say which run took the money.
    let (status, paid_by) = claim_state(&pool, claim_id).await;
    assert_eq!(status, "processed");
    assert_eq!(
        paid_by,
        Some(run.id),
        "claim must name the run that paid it"
    );
}

/// An `approved` overtime_application with `ot_type='rest_day'` contributes
/// 2× hourly-rate pay to `payroll_items.total_overtime` (and therefore gross).
#[tokio::test]
async fn approved_overtime_rest_day_adds_2x_to_gross() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    // RM5,000 basic → default hourly rate = 500_000 / 26 / 8 = 2403.846… sen.
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    // 2h rest-day OT → expected pay = 2403.846… × 2.0 × 2 = 9615 sen.
    sqlx::query(
        r#"INSERT INTO overtime_applications
           (employee_id, company_id, ot_date, start_time, end_time, hours, ot_type, status)
           VALUES ($1, $2, $3, '09:00', '11:00', 2, 'rest_day', 'approved')"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(NaiveDate::from_ymd_opt(2024, 5, 11).unwrap()) // a Saturday
    .execute(&pool)
    .await
    .expect("insert OT");

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        5,
        NaiveDate::from_ymd_opt(2024, 6, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    let (total_overtime, gross_salary, basic_salary): (i64, i64, i64) = sqlx::query_as(
        r#"SELECT total_overtime, gross_salary, basic_salary
           FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Hourly rate is computed in Decimal from the company's configured divisor and
    // effective hours, then only the final amount is rounded: 500_000 / 26 / 8 =
    // 2403.846… sen, × 2.0 (rest day) × 2h = 9615.38 → 9615. The previous
    // `basic / 26 / 8` integer division truncated the rate to 2403 and paid 9612,
    // losing 3 sen per event against the employee.
    let expected_ot = 9_615_i64;
    assert_eq!(
        total_overtime, expected_ot,
        "rest-day OT should be 2× the unrounded hourly rate × hours"
    );
    assert_eq!(
        gross_salary,
        basic_salary + total_overtime,
        "gross = basic + OT (no allowances in this scenario)"
    );
}

/// A `pending` overtime_application must NOT flow into payroll — only
/// approvals get paid.
#[tokio::test]
async fn pending_overtime_does_not_affect_payroll() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    sqlx::query(
        r#"INSERT INTO overtime_applications
           (employee_id, company_id, ot_date, start_time, end_time, hours, ot_type, status)
           VALUES ($1, $2, $3, '09:00', '13:00', 4, 'normal', 'pending')"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(NaiveDate::from_ymd_opt(2024, 6, 15).unwrap())
    .execute(&pool)
    .await
    .unwrap();

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        6,
        NaiveDate::from_ymd_opt(2024, 7, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    let total_overtime: i64 = sqlx::query_scalar(
        "SELECT total_overtime FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2",
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(total_overtime, 0, "pending OT must not be paid");
}

/// Approving overtime leaves BOTH an `approved` overtime_applications row and a
/// staged `payroll_entries` (earning/overtime) row. The engine recomputes OT
/// from the application, so the staged row must not be summed into gross as
/// well — otherwise every approved OT is paid roughly twice.
#[tokio::test]
async fn approved_overtime_with_staged_entry_is_not_double_paid() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    sqlx::query(
        r#"INSERT INTO overtime_applications
           (employee_id, company_id, ot_date, start_time, end_time, hours, ot_type, status)
           VALUES ($1, $2, $3, '09:00', '11:00', 2, 'rest_day', 'approved')"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(NaiveDate::from_ymd_opt(2024, 5, 11).unwrap())
    .execute(&pool)
    .await
    .expect("insert OT");

    // What approve_overtime also writes.
    sqlx::query(
        r#"INSERT INTO payroll_entries
           (employee_id, company_id, period_year, period_month, category, item_type,
            description, amount, created_by)
           VALUES ($1, $2, 2024, 5, 'earning', 'overtime', 'OT 2h rest_day', 9612, $3)"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("stage OT entry");

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        5,
        NaiveDate::from_ymd_opt(2024, 6, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    let (total_overtime, gross_salary, basic_salary): (i64, i64, i64) = sqlx::query_as(
        r#"SELECT total_overtime, gross_salary, basic_salary
           FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Same Decimal-exact figure as approved_overtime_rest_day_adds_2x_to_gross.
    let expected_ot = 9_615_i64;
    assert_eq!(total_overtime, expected_ot, "OT counted once");
    assert_eq!(
        gross_salary,
        basic_salary + expected_ot,
        "staged OT entry must not be added to gross on top of the recomputed OT"
    );
}

/// Approving a claim used to write a parallel `payroll_entries` reimbursement
/// keyed to the approval month. It never reached gross or net, but the next run
/// flipped it to processed and the cancel path then refused to cancel a claim
/// nobody had paid. `claims` is the single authority now: approval writes no
/// entry, and the reimbursement still lands in net exactly once.
#[tokio::test]
async fn approving_a_claim_stages_no_payroll_entry_and_pays_it_once() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee_with_email(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let claim_id = seed_claim(
        &pool,
        company_id,
        employee_id,
        15_000,
        NaiveDate::from_ymd_opt(2024, 4, 10).unwrap(),
        "pending",
    )
    .await;

    approval_service::approve_claim(
        &pool,
        &test_config(),
        company_id,
        claim_id,
        Reviewer {
            user_id,
            may_self_approve: false,
        },
        None,
        None,
    )
    .await
    .expect("approve_claim");

    let staged: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM payroll_entries WHERE employee_id = $1 AND item_type = 'claim_reimbursement'",
    )
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(staged, 0, "approval must not stage a parallel entry");

    let run = run_payroll(&pool, company_id, group_id, 2024, 4, user_id).await;

    let (total_claims, net_salary, gross_salary, total_deductions): (i64, i64, i64, i64) =
        sqlx::query_as(
            r#"SELECT total_claims, net_salary, gross_salary, total_deductions
               FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2"#,
        )
        .bind(run.id)
        .bind(employee_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(total_claims, 15_000, "claim counted once");
    assert_eq!(
        gross_salary, 500_000,
        "a reimbursement must not enter gross"
    );
    assert_eq!(
        net_salary,
        gross_salary - total_deductions + total_claims,
        "reimbursement added to net exactly once"
    );
}

// ─── R1-C1: claims are the single payment authority ───

/// The defect, end to end. A claim incurred in April but approved after the
/// April run closed used to be payable by nothing: April refuses a second run,
/// and May's window (`expense_date BETWEEN period_start AND period_end`)
/// excluded an April date. Selection is carry-forward now, so the next run
/// sweeps it.
#[tokio::test]
async fn claim_approved_after_its_expense_month_is_paid_by_the_next_run() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let paid_on_time = seed_claim(
        &pool,
        company_id,
        employee_id,
        15_000,
        NaiveDate::from_ymd_opt(2024, 4, 10).unwrap(),
        "approved",
    )
    .await;
    let april_run = run_payroll(&pool, company_id, group_id, 2024, 4, user_id).await;
    assert_eq!(
        total_claims_for(&pool, april_run.id, employee_id).await,
        15_000
    );
    assert_eq!(
        claim_state(&pool, paid_on_time).await,
        ("processed".to_string(), Some(april_run.id))
    );

    // Approved only now — after the April run has closed.
    let late = seed_claim(
        &pool,
        company_id,
        employee_id,
        22_500,
        NaiveDate::from_ymd_opt(2024, 4, 20).unwrap(),
        "approved",
    )
    .await;

    let may_run = run_payroll(&pool, company_id, group_id, 2024, 5, user_id).await;

    assert_eq!(
        total_claims_for(&pool, may_run.id, employee_id).await,
        22_500,
        "a claim approved after its expense month must be swept by the next run"
    );
    assert_eq!(
        claim_state(&pool, late).await,
        ("processed".to_string(), Some(may_run.id))
    );
}

/// Carry-forward selection must not turn into carry-forever: once a run has paid
/// a claim, `payroll_run_id IS NULL` excludes it from every later run.
#[tokio::test]
async fn paid_claim_is_not_paid_twice() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    seed_claim(
        &pool,
        company_id,
        employee_id,
        15_000,
        NaiveDate::from_ymd_opt(2024, 4, 10).unwrap(),
        "approved",
    )
    .await;

    let april_run = run_payroll(&pool, company_id, group_id, 2024, 4, user_id).await;
    assert_eq!(
        total_claims_for(&pool, april_run.id, employee_id).await,
        15_000
    );

    let may_run = run_payroll(&pool, company_id, group_id, 2024, 5, user_id).await;
    assert_eq!(
        total_claims_for(&pool, may_run.id, employee_id).await,
        0,
        "an already-paid claim must not be swept again"
    );
}

/// The case a period-bounded revert cannot express: a run legitimately pays
/// claims incurred before its own period, so reverting by period would
/// un-process claims an earlier run already paid.
#[tokio::test]
async fn deleting_a_run_reverts_only_its_own_claims() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let april_claim = seed_claim(
        &pool,
        company_id,
        employee_id,
        15_000,
        NaiveDate::from_ymd_opt(2024, 4, 10).unwrap(),
        "approved",
    )
    .await;
    let april_run = run_payroll(&pool, company_id, group_id, 2024, 4, user_id).await;

    let may_claim = seed_claim(
        &pool,
        company_id,
        employee_id,
        22_500,
        NaiveDate::from_ymd_opt(2024, 5, 6).unwrap(),
        "approved",
    )
    .await;
    let may_run = run_payroll(&pool, company_id, group_id, 2024, 5, user_id).await;

    payroll_service::delete_run(&pool, company_id, may_run.id, user_id, None)
        .await
        .expect("delete the May run");

    assert_eq!(
        claim_state(&pool, may_claim).await,
        ("approved".to_string(), None),
        "the deleted run's claim goes back to payable"
    );
    assert_eq!(
        claim_state(&pool, april_claim).await,
        ("processed".to_string(), Some(april_run.id)),
        "an earlier run's claim must be untouched"
    );
}

/// `expense_date <= period_end` is the whole of the carry-forward rule: an
/// expense that has not happened yet still waits.
#[tokio::test]
async fn future_dated_approved_claim_is_not_paid_early() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let future = seed_claim(
        &pool,
        company_id,
        employee_id,
        15_000,
        NaiveDate::from_ymd_opt(2024, 5, 3).unwrap(),
        "approved",
    )
    .await;

    let april_run = run_payroll(&pool, company_id, group_id, 2024, 4, user_id).await;

    assert_eq!(total_claims_for(&pool, april_run.id, employee_id).await, 0);
    assert_eq!(
        claim_state(&pool, future).await,
        ("approved".to_string(), None)
    );
}

/// The bug's user-visible face. An approved-but-unpaid claim was uncancellable
/// because a staged entry keyed to the approval month had been flipped to
/// processed; a genuinely paid one must still be refused.
#[tokio::test]
async fn approved_claim_can_be_cancelled_until_a_run_pays_it() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let cancellable = seed_claim(
        &pool,
        company_id,
        employee_id,
        15_000,
        NaiveDate::from_ymd_opt(2024, 4, 10).unwrap(),
        "approved",
    )
    .await;
    let cancelled =
        approval_service::cancel_claim_admin(&pool, company_id, cancellable, user_id, None)
            .await
            .expect("an approved claim no run has paid must be cancellable");
    assert_eq!(cancelled.status, "cancelled");

    let paid = seed_claim(
        &pool,
        company_id,
        employee_id,
        22_500,
        NaiveDate::from_ymd_opt(2024, 4, 12).unwrap(),
        "approved",
    )
    .await;
    run_payroll(&pool, company_id, group_id, 2024, 4, user_id).await;

    let err = approval_service::cancel_claim_admin(&pool, company_id, paid, user_id, None)
        .await
        .expect_err("a paid claim must not be cancellable");
    assert!(matches!(err, AppError::BadRequest(_)), "got {err:?}");
}

/// Two claims must produce two named payslip lines, not one aggregate. A July
/// payslip can legitimately reimburse a June expense, and the line is the only
/// place the employee can see why.
#[tokio::test]
async fn each_reimbursed_claim_gets_its_own_payslip_line() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    for (amount, day) in [(15_000_i64, 10_u32), (22_500, 20)] {
        seed_claim(
            &pool,
            company_id,
            employee_id,
            amount,
            NaiveDate::from_ymd_opt(2024, 4, day).unwrap(),
            "approved",
        )
        .await;
    }

    let run = run_payroll(&pool, company_id, group_id, 2024, 4, user_id).await;

    let (line_count, line_total): (i64, i64) = sqlx::query_as(
        r#"SELECT COUNT(*)::BIGINT, COALESCE(SUM(d.amount), 0)::BIGINT
           FROM payroll_item_details d
           JOIN payroll_items i ON i.id = d.payroll_item_id
           WHERE i.payroll_run_id = $1 AND i.employee_id = $2
             AND d.item_type = 'claim_reimbursement'"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(line_count, 2, "one line per claim");
    assert_eq!(
        line_total,
        total_claims_for(&pool, run.id, employee_id).await,
        "the lines must reconcile to total_claims"
    );
}

// ─── R1-C3: overtime application bounds ───

/// The exploit from the report: 999.99 hours declared over a one-hour window.
/// The admin create path had no bound at all.
#[tokio::test]
async fn admin_created_overtime_rejects_unbounded_hours() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let user_id = seed_user(&pool, company_id, "hr_manager").await;

    let err = approval_service::create_overtime_admin(
        &pool,
        company_id,
        employee_id,
        CreateOvertimeRequest {
            ot_date: NaiveDate::from_ymd_opt(2024, 4, 10).unwrap(),
            start_time: "09:00".into(),
            end_time: "10:00".into(),
            hours: dec!(999.99),
            ot_type: Some("public_holiday".into()),
            reason: None,
        },
        user_id,
        None,
    )
    .await
    .expect_err("999.99 hours over a one-hour window must be refused");
    assert!(matches!(err, AppError::BadRequest(_)), "got {err:?}");

    let rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM overtime_applications WHERE employee_id = $1")
            .bind(employee_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rows, 0, "nothing may be written when validation fails");
}

#[tokio::test]
async fn admin_overtime_update_rejects_negative_hours() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let user_id = seed_user(&pool, company_id, "hr_manager").await;

    let created = approval_service::create_overtime_admin(
        &pool,
        company_id,
        employee_id,
        CreateOvertimeRequest {
            ot_date: NaiveDate::from_ymd_opt(2024, 4, 10).unwrap(),
            start_time: "18:00".into(),
            end_time: "20:00".into(),
            hours: dec!(2),
            ot_type: None,
            reason: None,
        },
        user_id,
        None,
    )
    .await
    .expect("a two-hour application inside a two-hour window is valid");

    let err = approval_service::update_overtime_admin(
        &pool,
        company_id,
        created.id,
        UpdateOvertimeRequest {
            employee_id: None,
            ot_date: None,
            start_time: None,
            end_time: None,
            hours: Some(dec!(-50)),
            ot_type: None,
            reason: None,
        },
        user_id,
        None,
    )
    .await
    .expect_err("negative hours must be refused");
    assert!(matches!(err, AppError::BadRequest(_)), "got {err:?}");

    let hours: rust_decimal::Decimal =
        sqlx::query_scalar("SELECT hours FROM overtime_applications WHERE id = $1")
            .bind(created.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(hours, dec!(2), "the row must be left unchanged");
}

/// `update_full` COALESCEs, so validating only what the request carried would
/// let an edit keep the hours and shrink the window around them.
#[tokio::test]
async fn admin_overtime_update_validates_effective_hours() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let user_id = seed_user(&pool, company_id, "hr_manager").await;

    let created = approval_service::create_overtime_admin(
        &pool,
        company_id,
        employee_id,
        CreateOvertimeRequest {
            ot_date: NaiveDate::from_ymd_opt(2024, 4, 10).unwrap(),
            start_time: "18:00".into(),
            end_time: "22:00".into(),
            hours: dec!(4),
            ot_type: None,
            reason: None,
        },
        user_id,
        None,
    )
    .await
    .expect("create overtime");

    let err = approval_service::update_overtime_admin(
        &pool,
        company_id,
        created.id,
        UpdateOvertimeRequest {
            employee_id: None,
            ot_date: None,
            start_time: None,
            // Four hours no longer fit the window, though `hours` was not touched.
            end_time: Some("19:00".into()),
            hours: None,
            ot_type: None,
            reason: None,
        },
        user_id,
        None,
    )
    .await
    .expect_err("the effective hours must be validated, not just the supplied ones");
    assert!(matches!(err, AppError::BadRequest(_)), "got {err:?}");
}

/// The database CHECK is the outer bound (0 < h <= 24); the window rule is
/// tighter and the database cannot see it. A row inside the CHECK but far
/// outside its declared window — 20 hours over a two-hour window — is still
/// insertable, and without a service-side re-check at approval it would still
/// stage the money.
#[tokio::test]
async fn approving_a_legacy_out_of_range_overtime_is_refused() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let user_id = seed_user(&pool, company_id, "hr_manager").await;

    let ot_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO overtime_applications
           (id, employee_id, company_id, ot_date, start_time, end_time, hours, ot_type, status)
           VALUES ($1, $2, $3, $4, '09:00', '11:00', 20, 'normal', 'pending')"#,
    )
    .bind(ot_id)
    .bind(employee_id)
    .bind(company_id)
    .bind(NaiveDate::from_ymd_opt(2024, 4, 10).unwrap())
    .execute(&pool)
    .await
    .expect("20 h satisfies the database CHECK but not the declared window");

    let err = approval_service::approve_overtime(
        &pool,
        company_id,
        ot_id,
        Reviewer {
            user_id,
            may_self_approve: false,
        },
        None,
        None,
    )
    .await
    .expect_err("a legacy out-of-range application must not be approvable");
    assert!(matches!(err, AppError::BadRequest(_)), "got {err:?}");

    let staged: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM payroll_entries WHERE employee_id = $1 AND item_type = 'overtime'",
    )
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(staged, 0, "nothing may be staged for a refused approval");

    let status: String =
        sqlx::query_scalar("SELECT status FROM overtime_applications WHERE id = $1")
            .bind(ot_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "pending");
}

// ─── R1-C4: maker-checker ───

#[tokio::test]
async fn approver_cannot_approve_own_claim() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee_with_email(&pool, company_id, None, 500_000).await;
    let user_id = seed_user(&pool, company_id, "hr_manager").await;
    link_user_to_employee(&pool, user_id, employee_id).await;

    let claim_id = seed_claim(
        &pool,
        company_id,
        employee_id,
        15_000,
        NaiveDate::from_ymd_opt(2024, 4, 10).unwrap(),
        "pending",
    )
    .await;

    let err = approval_service::approve_claim(
        &pool,
        &test_config(),
        company_id,
        claim_id,
        Reviewer {
            user_id,
            may_self_approve: false,
        },
        None,
        None,
    )
    .await
    .expect_err("self-approval must be refused");
    assert!(matches!(err, AppError::Forbidden(_)), "got {err:?}");

    // Still pending proves the transaction rolled back, not merely that the
    // HTTP status was wrong.
    assert_eq!(
        claim_state(&pool, claim_id).await,
        ("pending".to_string(), None)
    );
}

/// The link is read from `users`, not from the token. A token minted before the
/// user was linked carries no employee id, and a guard keyed on that would be
/// bypassed by simply not logging in again.
#[tokio::test]
async fn guard_uses_the_database_link_not_the_token() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee_with_email(&pool, company_id, None, 500_000).await;
    // Created unlinked — as it would have been when the token was issued.
    let user_id = seed_user(&pool, company_id, "hr_manager").await;

    let claim_id = seed_claim(
        &pool,
        company_id,
        employee_id,
        15_000,
        NaiveDate::from_ymd_opt(2024, 4, 10).unwrap(),
        "pending",
    )
    .await;

    // Linked only now.
    link_user_to_employee(&pool, user_id, employee_id).await;

    let err = approval_service::approve_claim(
        &pool,
        &test_config(),
        company_id,
        claim_id,
        Reviewer {
            user_id,
            may_self_approve: false,
        },
        None,
        None,
    )
    .await
    .expect_err("the guard must read the current database link");
    assert!(matches!(err, AppError::Forbidden(_)), "got {err:?}");
}

#[tokio::test]
async fn approver_can_approve_a_colleagues_claim() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let approver_employee = seed_employee_with_email(&pool, company_id, None, 500_000).await;
    let colleague = seed_employee_with_email(&pool, company_id, None, 400_000).await;
    let user_id = seed_user(&pool, company_id, "hr_manager").await;
    link_user_to_employee(&pool, user_id, approver_employee).await;

    let claim_id = seed_claim(
        &pool,
        company_id,
        colleague,
        15_000,
        NaiveDate::from_ymd_opt(2024, 4, 10).unwrap(),
        "pending",
    )
    .await;

    let claim = approval_service::approve_claim(
        &pool,
        &test_config(),
        company_id,
        claim_id,
        Reviewer {
            user_id,
            may_self_approve: false,
        },
        None,
        None,
    )
    .await
    .expect("approving a colleague's claim is the non-regression case");
    assert_eq!(claim.status, "approved");
}

/// An admin who is not themselves on the payroll has `users.employee_id = NULL`.
/// `None == None` must not read as a self-approval.
#[tokio::test]
async fn approver_with_no_employee_record_is_unaffected() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee_with_email(&pool, company_id, None, 500_000).await;
    let user_id = seed_user(&pool, company_id, "admin").await;

    let claim_id = seed_claim(
        &pool,
        company_id,
        employee_id,
        15_000,
        NaiveDate::from_ymd_opt(2024, 4, 10).unwrap(),
        "pending",
    )
    .await;

    let claim = approval_service::approve_claim(
        &pool,
        &test_config(),
        company_id,
        claim_id,
        Reviewer {
            user_id,
            may_self_approve: false,
        },
        None,
        None,
    )
    .await
    .expect("an approver with no employee record approves normally");
    assert_eq!(claim.status, "approved");
}

/// `super_admin` is the documented escape hatch for a one-person-HR tenant. It
/// is sound because `ManageUsers` belongs to no role set except SUPER_ADMIN, so
/// the role is not self-grantable — but it must be queryable in the audit trail
/// rather than indistinguishable from an ordinary approval.
#[tokio::test]
async fn super_admin_may_approve_own_claim_and_the_override_is_audited() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee_with_email(&pool, company_id, None, 500_000).await;
    let user_id = seed_user(&pool, company_id, "super_admin").await;
    link_user_to_employee(&pool, user_id, employee_id).await;

    let claim_id = seed_claim(
        &pool,
        company_id,
        employee_id,
        15_000,
        NaiveDate::from_ymd_opt(2024, 4, 10).unwrap(),
        "pending",
    )
    .await;

    let claim = approval_service::approve_claim(
        &pool,
        &test_config(),
        company_id,
        claim_id,
        Reviewer {
            user_id,
            may_self_approve: true,
        },
        None,
        None,
    )
    .await
    .expect("the override path must succeed");
    assert_eq!(claim.status, "approved");

    let overrides: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM audit_logs
           WHERE action = 'self_approval_override' AND entity_id = $1"#,
    )
    .bind(claim_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        overrides, 1,
        "the override must be visible in /api/audit-logs"
    );
}

#[tokio::test]
async fn approver_cannot_approve_own_overtime() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee_with_email(&pool, company_id, None, 500_000).await;
    let user_id = seed_user(&pool, company_id, "hr_manager").await;
    link_user_to_employee(&pool, user_id, employee_id).await;

    let created = approval_service::create_overtime_admin(
        &pool,
        company_id,
        employee_id,
        CreateOvertimeRequest {
            ot_date: NaiveDate::from_ymd_opt(2024, 4, 10).unwrap(),
            start_time: "18:00".into(),
            end_time: "20:00".into(),
            hours: dec!(2),
            ot_type: None,
            reason: None,
        },
        user_id,
        None,
    )
    .await
    .expect("raising your own overtime is legitimate; approving it is not");

    let err = approval_service::approve_overtime(
        &pool,
        company_id,
        created.id,
        Reviewer {
            user_id,
            may_self_approve: false,
        },
        None,
        None,
    )
    .await
    .expect_err("self-approval must be refused");
    assert!(matches!(err, AppError::Forbidden(_)), "got {err:?}");

    let status: String =
        sqlx::query_scalar("SELECT status FROM overtime_applications WHERE id = $1")
            .bind(created.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "pending", "the CAS must have rolled back");

    let staged: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM payroll_entries WHERE employee_id = $1 AND item_type = 'overtime'",
    )
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(staged, 0);
}

#[tokio::test]
async fn approver_cannot_approve_own_leave() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee_with_email(&pool, company_id, None, 500_000).await;
    let user_id = seed_user(&pool, company_id, "hr_manager").await;
    link_user_to_employee(&pool, user_id, employee_id).await;

    let leave_type_id: Uuid = sqlx::query_scalar(
        "INSERT INTO leave_types (company_id, name, default_days) VALUES ($1, $2, 10) RETURNING id",
    )
    .bind(company_id)
    .bind(format!("Annual {}", &Uuid::new_v4().to_string()[..8]))
    .fetch_one(&pool)
    .await
    .expect("create leave type");

    sqlx::query(
        r#"INSERT INTO leave_balances
           (employee_id, leave_type_id, year, entitled_days, taken_days, pending_days)
           VALUES ($1, $2, 2024, 10, 0, 1)"#,
    )
    .bind(employee_id)
    .bind(leave_type_id)
    .execute(&pool)
    .await
    .expect("seed leave balance");

    let request_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO leave_requests
           (employee_id, company_id, leave_type_id, start_date, end_date, days, status)
           VALUES ($1, $2, $3, $4, $4, 1, 'pending') RETURNING id"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(leave_type_id)
    .bind(NaiveDate::from_ymd_opt(2024, 4, 10).unwrap())
    .fetch_one(&pool)
    .await
    .expect("create leave request");

    let err = approval_service::approve_leave(
        &pool,
        &test_config(),
        company_id,
        request_id,
        Reviewer {
            user_id,
            may_self_approve: false,
        },
        None,
        None,
    )
    .await
    .expect_err("self-approval must be refused");
    assert!(matches!(err, AppError::Forbidden(_)), "got {err:?}");

    let status: String = sqlx::query_scalar("SELECT status FROM leave_requests WHERE id = $1")
        .bind(request_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "pending");

    // The rollback that matters: the pending->taken move must not have happened.
    let (taken, pending): (rust_decimal::Decimal, rust_decimal::Decimal) = sqlx::query_as(
        "SELECT taken_days, pending_days FROM leave_balances WHERE employee_id = $1 AND leave_type_id = $2 AND year = 2024",
    )
    .bind(employee_id)
    .bind(leave_type_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(taken, dec!(0));
    assert_eq!(pending, dec!(1));
}
