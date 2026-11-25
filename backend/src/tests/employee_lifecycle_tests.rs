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

#[tokio::test]
async fn employee_numeric_and_date_invariants_are_validated_on_create_and_update() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let user_id = seed_user(&pool, company_id, "hr_manager").await;

    let mut bad_salary: crate::models::employee::CreateEmployeeRequest =
        serde_json::from_value(json!({
            "employee_number": format!("NEG-{}", Uuid::new_v4()),
            "full_name": "Invalid Salary",
            "date_joined": "2024-01-10",
            "basic_salary": -100
        }))
        .expect("valid CreateEmployeeRequest");

    let err = employee_service::create_employee(&pool, company_id, bad_salary, user_id, None)
        .await
        .expect_err("negative salary must be rejected");
    assert!(format!("{err:?}").contains("Basic salary cannot be negative"));

    bad_salary = serde_json::from_value(json!({
        "employee_number": format!("PROB-{}", Uuid::new_v4()),
        "full_name": "Invalid Probation",
        "date_joined": "2024-01-10",
        "basic_salary": 300000,
        "probation_start": "2024-03-01",
        "probation_end": "2024-02-01"
    }))
    .expect("valid CreateEmployeeRequest");

    let err = employee_service::create_employee(&pool, company_id, bad_salary, user_id, None)
        .await
        .expect_err("probation end before start must be rejected");
    assert!(format!("{err:?}").contains("Probation end date cannot be earlier"));

    let employee_id = seed_employee(&pool, company_id, None, 300_000).await;

    for (payload, needle) in [
        (
            json!({ "hourly_rate": -1 }),
            "Hourly rate cannot be negative",
        ),
        (json!({ "daily_rate": -1 }), "Daily rate cannot be negative"),
        (
            json!({ "num_children": 31 }),
            "Number of children must be between 0 and 30",
        ),
        (
            json!({ "zakat_monthly_amount": -5 }),
            "Monthly Zakat amount cannot be negative",
        ),
        (
            json!({ "ptptn_monthly_amount": -5 }),
            "Monthly PTPTN amount cannot be negative",
        ),
        (
            json!({ "tabung_haji_amount": -5 }),
            "Monthly Tabung Haji amount cannot be negative",
        ),
        (
            json!({ "confirmation_date": "2019-12-31" }),
            "Confirmation date cannot be earlier than join date",
        ),
    ] {
        let err = employee_service::update_employee(
            &pool,
            employee_id,
            company_id,
            update_request(payload),
            user_id,
            None,
        )
        .await
        .expect_err("invalid employee field must be rejected on update");
        assert!(
            format!("{err:?}").contains(needle),
            "expected '{needle}', got {err:?}"
        );
    }
}

/// A staff login linked to an employee record: what an employee promoted to a
/// staff role looks like. Returns the user id.
async fn seed_linked_login(pool: &PgPool, company_id: Uuid, employee_id: Uuid, role: &str) -> Uuid {
    let user_id = seed_user(pool, company_id, role).await;
    crate::repositories::users::link_to_employee(pool, employee_id, company_id, user_id)
        .await
        .expect("link login to employee");
    crate::repositories::user_companies::insert(pool, user_id, company_id)
        .await
        .expect("link company");
    user_id
}

/// A manager or HR user must not be able to cut off an admin or super_admin by
/// deactivating or deleting the employee record their login is linked to.
#[tokio::test]
async fn lower_roles_cannot_revoke_a_privileged_linked_login() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let roles = |r: &[&str]| r.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();

    for (target, refused_for, allowed_for) in [
        (
            "super_admin",
            vec!["admin", "manager", "hr_manager"],
            "super_admin",
        ),
        (
            "admin",
            vec!["manager", "hr_manager", "payroll_admin"],
            "admin",
        ),
    ] {
        let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
        seed_linked_login(&pool, company_id, employee_id, target).await;

        for caller in refused_for {
            let err = employee_service::ensure_may_revoke_linked_logins(
                &pool,
                employee_id,
                company_id,
                &roles(&[caller]),
            )
            .await
            .expect_err("lower role must be refused");
            assert!(
                matches!(err, crate::core::error::AppError::Forbidden(_)),
                "{caller} revoking {target}: {err:?}"
            );
        }
        employee_service::ensure_may_revoke_linked_logins(
            &pool,
            employee_id,
            company_id,
            &roles(&[allowed_for]),
        )
        .await
        .unwrap_or_else(|e| panic!("{allowed_for} may revoke {target}: {e:?}"));
    }

    // A record with no privileged login is unaffected.
    let plain = seed_employee(&pool, company_id, None, 500_000).await;
    employee_service::ensure_may_revoke_linked_logins(
        &pool,
        plain,
        company_id,
        &roles(&["manager"]),
    )
    .await
    .expect("no linked login");
}

/// Deleting an employee in one company removes only that company's access from
/// a linked staff login. Its other memberships survive and the stale employee
/// link is cleared, so it keeps working elsewhere.
#[tokio::test]
async fn deleting_an_employee_keeps_a_staff_logins_other_companies() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_a = seed_company(&pool).await;
    let company_b = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_a, None, 500_000).await;
    let user_id = seed_linked_login(&pool, company_a, employee_id, "admin").await;
    crate::repositories::user_companies::insert(&pool, user_id, company_b)
        .await
        .expect("link second company");
    let actor = seed_user(&pool, company_a, "admin").await;

    employee_service::soft_delete_employee(&pool, employee_id, company_a, actor, None)
        .await
        .expect("delete employee");

    use crate::repositories::user_companies::user_has_company;
    assert!(!user_has_company(&pool, user_id, company_a).await.unwrap());
    assert!(
        user_has_company(&pool, user_id, company_b).await.unwrap(),
        "membership in another company must survive"
    );

    let (employee_link, active_company, deleted): (Option<Uuid>, Option<Uuid>, bool) =
        sqlx::query_as(
            "SELECT employee_id, company_id, deleted_at IS NOT NULL FROM users WHERE id = $1",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("read user");
    assert_eq!(employee_link, None, "stale employee link must be cleared");
    assert_eq!(
        active_company,
        Some(company_b),
        "active company moves off the deleted one"
    );
    assert!(
        !deleted,
        "a staff login is not deleted with the employee record"
    );
}
