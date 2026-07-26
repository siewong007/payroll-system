use chrono::NaiveDate;

use crate::services::payroll_engine;
use crate::tests::support::{
    seed_company, seed_employee, seed_payroll_group, seed_user, skip_if_no_db,
};

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

    // The claim row must be marked processed so it isn't picked up next run.
    let claim_status: String = sqlx::query_scalar("SELECT status FROM claims WHERE id = $1")
        .bind(claim_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(claim_status, "processed");
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

/// Approving a claim leaves BOTH an `approved` claims row and a staged
/// `payroll_entries` (earning/claim_reimbursement) row. Reimbursements are not
/// wages: the staged row must stay out of gross (or it becomes EPF/SOCSO/EIS/PCB
/// liable) and must not be added to net a second time.
#[tokio::test]
async fn approved_claim_with_staged_entry_is_not_double_paid() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    sqlx::query(
        r#"INSERT INTO claims
           (employee_id, company_id, title, amount, expense_date, status)
           VALUES ($1, $2, 'Taxi fare', 15000, $3, 'approved')"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(NaiveDate::from_ymd_opt(2024, 4, 10).unwrap())
    .execute(&pool)
    .await
    .expect("insert claim");

    // What approve_claim also writes.
    sqlx::query(
        r#"INSERT INTO payroll_entries
           (employee_id, company_id, period_year, period_month, category, item_type,
            description, amount, created_by)
           VALUES ($1, $2, 2024, 4, 'earning', 'claim_reimbursement', 'Taxi fare', 15000, $3)"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("stage claim entry");

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

    assert_eq!(total_claims, 15_000, "claim counted once");
    assert_eq!(
        gross_salary, 500_000,
        "staged reimbursement must not enter gross"
    );
    assert_eq!(
        net_salary,
        gross_salary - total_deductions + total_claims,
        "reimbursement added to net exactly once"
    );
}
