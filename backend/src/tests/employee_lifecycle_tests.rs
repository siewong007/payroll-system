//! Employment lifecycle: recording, clearing and honouring a resignation.
//!
//! Two halves of one defect. A leaver is deactivated the moment HR records the
//! termination, and the payroll population used to select on that flag — so the
//! final, prorated payslip and the statutory contributions on it disappeared
//! with no error and no preview diagnostic. The resignation date itself was also
//! write-once, because `employees::update` folded it through a bare COALESCE.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::employee::UpdateEmployeeRequest;
use crate::services::{employee_service, payroll_engine};
use crate::tests::support::{
    seed_company, seed_employee, seed_payroll_group, seed_user, skip_if_no_db,
};

/// Build an `UpdateEmployeeRequest` from the JSON an API client would send.
/// Every field is optional, so this also pins the wire names the frontend uses.
fn update_request(body: serde_json::Value) -> UpdateEmployeeRequest {
    serde_json::from_value(body).expect("valid UpdateEmployeeRequest")
}

/// Put an employee into a lifecycle state directly, the way a legacy row or an
/// HR action outside this test would leave it.
async fn set_lifecycle(
    pool: &PgPool,
    employee_id: Uuid,
    date_resigned: Option<NaiveDate>,
    is_active: bool,
) {
    sqlx::query("UPDATE employees SET date_resigned = $2, is_active = $3 WHERE id = $1")
        .bind(employee_id)
        .bind(date_resigned)
        .bind(is_active)
        .execute(pool)
        .await
        .expect("update employment lifecycle fields");
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
}

/// A resignation can be recorded and then undone. Regression test for
/// `date_resigned = COALESCE($27, date_resigned)`, under which an employee
/// re-hired or terminated by mistake could never be un-terminated through the
/// API — the date was write-once and kept them out of every later run.
#[tokio::test]
async fn resignation_date_can_be_set_then_cleared() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let user_id = seed_user(&pool, company_id, "hr_manager").await;

    let body = json!({
        "date_resigned": "2024-05-15",
        "resignation_reason": "Resigned to study",
    });
    let terminated = employee_service::update_employee(
        &pool,
        employee_id,
        company_id,
        update_request(body),
        user_id,
        None,
    )
    .await
    .expect("recording a resignation should succeed");

    assert_eq!(terminated.date_resigned, Some(date(2024, 5, 15)));
    let reason = terminated.resignation_reason.as_deref();
    assert_eq!(reason, Some("Resigned to study"));

    // An unrelated edit must not disturb it — absent means keep.
    let untouched = employee_service::update_employee(
        &pool,
        employee_id,
        company_id,
        update_request(json!({ "department": "Finance" })),
        user_id,
        None,
    )
    .await
    .expect("unrelated edit should succeed");
    let kept = untouched.date_resigned;
    assert_eq!(
        kept,
        Some(date(2024, 5, 15)),
        "absent means keep, not clear"
    );

    let reinstated = employee_service::update_employee(
        &pool,
        employee_id,
        company_id,
        update_request(json!({ "clear_date_resigned": true })),
        user_id,
        None,
    )
    .await
    .expect("clearing a resignation should succeed");

    assert_eq!(reinstated.date_resigned, None);
    // The reason is meaningless without a date, so it clears with it.
    let cleared_reason = reinstated.resignation_reason;
    assert_eq!(cleared_reason, None, "the reason clears with the date");
}

/// A resignation before the date joined would make `days_worked` clamp to zero
/// and pay nothing for a month actually worked.
#[tokio::test]
async fn resignation_date_before_date_joined_is_rejected() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    // `seed_employee` joins 2020-01-01.
    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let user_id = seed_user(&pool, company_id, "hr_manager").await;

    let err = employee_service::update_employee(
        &pool,
        employee_id,
        company_id,
        update_request(json!({ "date_resigned": "2019-06-30" })),
        user_id,
        None,
    )
    .await
    .expect_err("a resignation before the date joined must be rejected");

    assert!(
        format!("{err:?}").contains("earlier than the date joined 2020-01-01"),
        "expected a date-order BadRequest naming both dates, got: {err:?}"
    );
}

/// Setting and clearing in one request is ambiguous, so it is refused rather
/// than silently resolved in either direction.
#[tokio::test]
async fn setting_and_clearing_the_resignation_date_together_is_rejected() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let user_id = seed_user(&pool, company_id, "hr_manager").await;

    let body = json!({
        "date_resigned": "2024-05-15",
        "clear_date_resigned": true,
    });
    let err = employee_service::update_employee(
        &pool,
        employee_id,
        company_id,
        update_request(body),
        user_id,
        None,
    )
    .await
    .expect_err("set and clear together must be rejected");

    assert!(
        format!("{err:?}").contains("set and clear"),
        "expected a contradiction BadRequest, got: {err:?}"
    );
}

/// The core defect: a leaver deactivated on the day their resignation was
/// recorded is still owed a prorated final payslip and the statutory
/// contributions on it. Under the old `is_active = TRUE` population predicate
/// this run failed with "No active employees found" and the payslip — and every
/// contribution derived from it — was never produced.
#[tokio::test]
async fn deactivated_leaver_with_a_resignation_date_is_still_paid_for_the_month() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    set_lifecycle(&pool, employee_id, Some(date(2024, 5, 15)), false).await;

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        5,
        date(2024, 6, 5),
        user_id,
        None,
        None,
    )
    .await
    .expect("a leaver with a resignation date must be paid for the month they left");

    assert_eq!(run.employee_count, 1);

    let (basic, days_worked, is_prorated): (i64, Option<Decimal>, bool) = sqlx::query_as(
        r#"SELECT basic_salary, days_worked, is_prorated
           FROM payroll_items
           WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("the leaver should have a payslip");

    // Employment Act 1955 s.18B prorates by calendar days: 500_000 x 15/31.
    assert_eq!(basic, 241_935, "prorated to the 15 days actually worked");
    assert_eq!(days_worked, Some(Decimal::from(15)));
    assert!(is_prorated);

    // The statutory half of the defect: these went unremitted entirely.
    let (epf_ee, socso_ee, eis_ee): (i64, i64, i64) = sqlx::query_as(
        r#"SELECT epf_employee, socso_employee, eis_employee
           FROM payroll_items
           WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("the leaver should have a payslip");

    assert!(epf_ee > 0, "EPF must still be deducted on a final payslip");
    assert!(socso_ee > 0, "SOCSO must still be deducted");
    assert!(eis_ee > 0, "EIS must still be deducted");
}

/// An inactive employee with no resignation date is ambiguous — nothing on the
/// row says whether a final payslip is owed — so the run refuses instead of
/// guessing, and the preview names them.
#[tokio::test]
async fn inactive_employee_with_no_resignation_date_blocks_the_run() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let payable_id = seed_employee(&pool, company_id, Some(group_id), 300_000).await;
    let ambiguous_id = seed_employee(&pool, company_id, Some(group_id), 400_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    set_lifecycle(&pool, ambiguous_id, None, false).await;

    let preview =
        payroll_engine::preview_payroll(&pool, company_id, group_id, 2024, 6, date(2024, 7, 5))
            .await
            .expect("preview itself should succeed and report the problem");

    assert!(!preview.can_process, "an ambiguous row must block the run");
    let diagnostic = preview
        .blocking
        .iter()
        .find(|d| d.code == "inactive_without_resignation_date")
        .expect("preview should name the ambiguous employee");
    assert_eq!(diagnostic.employee_id, Some(ambiguous_id));

    // The payable colleague is still projected, so the operator sees what the
    // run would pay once the ambiguity is resolved.
    let previewed = preview
        .employees
        .iter()
        .any(|e| e.employee_id == payable_id);
    assert!(previewed, "the rest of the group must still be previewed");

    let err = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        6,
        date(2024, 7, 5),
        user_id,
        None,
        None,
    )
    .await
    .expect_err("processing must refuse while the row is ambiguous");
    assert!(
        format!("{err:?}").contains("inactive with no resignation date"),
        "the error should say why, got: {err:?}"
    );

    // The run row inserted before the check must roll back with the transaction.
    let runs: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM payroll_runs
           WHERE company_id = $1 AND payroll_group_id = $2
             AND period_year = 2024 AND period_month = 6"#,
    )
    .bind(company_id)
    .bind(group_id)
    .fetch_one(&pool)
    .await
    .expect("count runs");
    assert_eq!(runs, 0, "a refused run must leave nothing behind");
}

/// The population predicate admits a leaver only while their employment window
/// overlaps the period. Guards against the new predicate paying them forever.
#[tokio::test]
async fn leaver_is_not_paid_in_periods_after_they_left() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    set_lifecycle(&pool, employee_id, Some(date(2024, 4, 30)), false).await;

    let err = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        5,
        date(2024, 6, 5),
        user_id,
        None,
        None,
    )
    .await
    .expect_err("someone who left in April must not be paid for May");

    assert!(
        format!("{err:?}").contains("No active employees"),
        "expected an empty-population error, got: {err:?}"
    );
}
