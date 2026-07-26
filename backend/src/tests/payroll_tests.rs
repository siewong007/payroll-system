use chrono::NaiveDate;

use crate::services::payroll_engine;
use crate::tests::support::{
    seed_company, seed_employee, seed_payroll_group, seed_user, skip_if_no_db,
};

/// End-to-end test: seed one employee on RM5,000 basic, process payroll for
/// January 2024, verify the resulting `PayrollRun` + `PayrollItem` match the
/// values derived from the prototype statutory tables in `1001_data.sql`.
#[tokio::test]
async fn process_payroll_single_employee_rm5000() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        1,
        NaiveDate::from_ymd_opt(2024, 2, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll should succeed");

    assert_eq!(run.period_year, 2024);
    assert_eq!(run.period_month, 1);
    assert_eq!(run.employee_count, 1);
    assert_eq!(run.status, "processed");

    // One payroll_item row for the one employee.
    let (
        basic,
        gross,
        net,
        epf_ee,
        epf_er,
        socso_ee,
        socso_er,
        eis_ee,
        eis_er,
        pcb,
        total_deductions,
        employer_cost,
    ): (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"SELECT basic_salary, gross_salary, net_salary,
                  epf_employee, epf_employer,
                  socso_employee, socso_employer,
                  eis_employee, eis_employer,
                  pcb_amount, total_deductions, employer_cost
           FROM payroll_items
           WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("item should exist");

    // Basic/gross come straight from the employee row (no allowances, no OT).
    assert_eq!(basic, 500_000);
    assert_eq!(gross, 500_000);

    // Prototype reference values: wage 500_000 hits the top of each bracket.
    assert_eq!(
        epf_ee, 53_000,
        "EPF employee from seed bracket (480001, 500000)"
    );
    assert_eq!(epf_er, 58_000);
    assert_eq!(socso_ee, 1_825, "SOCSO employee (490001, 500000) first cat");
    assert_eq!(socso_er, 3_335);
    assert_eq!(eis_ee, 990, "EIS employee at ceiling");
    assert_eq!(eis_er, 990);

    // Accounting identities must hold.
    assert!(pcb >= 0, "PCB must not be negative");
    assert_eq!(
        total_deductions,
        epf_ee + socso_ee + eis_ee + pcb,
        "total_deductions = statutory contributions + PCB (no zakat/ptptn/haji/custom)"
    );
    assert_eq!(
        net,
        gross - total_deductions,
        "net = gross - deductions (no reimbursable claims in this scenario)"
    );
    assert_eq!(employer_cost, gross + epf_er + socso_er + eis_er);

    // Run totals equal item totals (one employee).
    assert_eq!(run.total_gross, gross);
    assert_eq!(run.total_net, net);
    assert_eq!(run.total_epf_employee, epf_ee);
    assert_eq!(run.total_socso_employee, socso_ee);
    assert_eq!(run.total_eis_employee, eis_ee);
    assert_eq!(run.total_pcb, pcb);
    assert_eq!(run.total_employer_cost, employer_cost);
}

/// The engine rejects a second run for the same (company, group, period)
/// unless the prior run was removed or is a retained cancelled legacy row. Protects against accidentally running
/// payroll twice.
#[tokio::test]
async fn process_payroll_rejects_duplicate_period() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let _emp = seed_employee(&pool, company_id, Some(group_id), 300_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let first = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        2,
        NaiveDate::from_ymd_opt(2024, 3, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await;
    assert!(first.is_ok(), "first run should succeed: {first:?}");

    let second = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        2,
        NaiveDate::from_ymd_opt(2024, 3, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await;

    let err = second.expect_err("second run for same period must fail");
    assert!(
        format!("{err:?}").contains("already exists"),
        "expected Conflict, got: {err:?}"
    );
}

/// Empty payroll group → BadRequest. Protects the "no employees found" branch.
#[tokio::test]
async fn process_payroll_rejects_empty_group() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let err = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        3,
        NaiveDate::from_ymd_opt(2024, 4, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect_err("empty group should fail");

    assert!(
        format!("{err:?}").contains("No active employees"),
        "expected 'No active employees' error, got: {err:?}"
    );
}

/// An employee who joins mid-period is paid only for the days they were employed.
/// Employment Act 1955 s.18B prorates an incomplete month by CALENDAR days:
/// monthly wages ÷ days in the month × days eligible.
#[tokio::test]
async fn mid_period_joiner_basic_salary_is_prorated() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    // Joined on the last day of a 31-day month → 1 of 31 days.
    sqlx::query("UPDATE employees SET date_joined = $2 WHERE id = $1")
        .bind(employee_id)
        .bind(NaiveDate::from_ymd_opt(2024, 5, 31).unwrap())
        .execute(&pool)
        .await
        .expect("set date_joined");

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

    let (basic_salary, working_days, days_worked, is_prorated): (
        i64,
        Option<i32>,
        Option<rust_decimal::Decimal>,
        Option<bool>,
    ) = sqlx::query_as(
        r#"SELECT basic_salary, working_days, days_worked, is_prorated
           FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 500_000 × 1 / 31 = 16_129.03 → 16_129 sen.
    assert_eq!(basic_salary, 16_129, "one day of a 31-day month");
    assert_eq!(working_days, Some(31), "calendar days in the period");
    assert_eq!(days_worked, Some(rust_decimal::Decimal::from(1)));
    assert_eq!(is_prorated, Some(true));
}

/// A full-month employee is not prorated — guards the proration branch itself.
#[tokio::test]
async fn full_month_employee_is_not_prorated() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

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

    let (basic_salary, is_prorated): (i64, Option<bool>) = sqlx::query_as(
        r#"SELECT basic_salary, is_prorated
           FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(basic_salary, 500_000, "full month pays the full basic");
    assert_eq!(is_prorated, Some(false));
}
