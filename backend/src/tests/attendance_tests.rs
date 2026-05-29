use crate::services::attendance_service;
use crate::tests::support::{seed_company_and_employee, skip_if_no_db};

/// Regression test for the `::int16` day-of-week bug: an employee-specific work
/// schedule for *today's* weekday must be matched so a check-in after the start
/// time is flagged "late". The employee-schedule lookup filters on day_of_week,
/// so before the fix (dow always 0/Sunday) this row was missed on weekdays and
/// the status wrongly fell through to "present".
#[tokio::test]
async fn checkin_status_matches_employee_schedule_for_today() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;

    // Schedule for today's KL weekday starting at 00:00:01 with no grace, so any
    // real-time check-in is "late". day_of_week is computed in the same timezone
    // determine_checkin_status uses (no company default row => Asia/Kuala_Lumpur).
    sqlx::query(
        r#"INSERT INTO employee_work_schedules
           (employee_id, company_id, day_of_week, start_time, end_time, grace_minutes, is_active)
           VALUES ($1, $2, EXTRACT(DOW FROM (NOW() AT TIME ZONE 'Asia/Kuala_Lumpur'))::int2,
                   TIME '00:00:01', TIME '23:59:59', 0, TRUE)"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("insert employee work schedule");

    let status = attendance_service::determine_checkin_status(&pool, employee_id, company_id).await;

    assert_eq!(
        status, "late",
        "employee schedule for today's weekday should be matched (requires the ::int2 fix)"
    );
}

/// Helper: insert an open (no check_out_at) attendance record whose
/// `check_in_at` is `hours_ago` hours in the past. Returns the new row's id.
async fn insert_open_record(
    pool: &sqlx::PgPool,
    company_id: uuid::Uuid,
    employee_id: uuid::Uuid,
    hours_ago: i32,
) -> uuid::Uuid {
    let id = uuid::Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO attendance_records
           (id, company_id, employee_id, check_in_at, method, status)
           VALUES ($1, $2, $3, NOW() - make_interval(hours => $4), 'manual', 'present')"#,
    )
    .bind(id)
    .bind(company_id)
    .bind(employee_id)
    .bind(hours_ago)
    .execute(pool)
    .await
    .expect("insert attendance record");
    id
}

#[tokio::test]
async fn check_out_matches_open_record_within_24h() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;

    // Overnight check-in: 20 hours ago, still open.
    let open_id = insert_open_record(&pool, company_id, employee_id, 20).await;

    let record = attendance_service::check_out(&pool, employee_id, company_id, None, None)
        .await
        .expect("check_out should succeed for a record within 24h");

    assert_eq!(record.id, open_id, "should close the open record");
    assert!(record.check_out_at.is_some(), "check_out_at must be set");
}

#[tokio::test]
async fn check_out_ignores_stale_record_older_than_24h() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;

    // Only record is 30h old — outside the 24-hour window.
    insert_open_record(&pool, company_id, employee_id, 30).await;

    let err = attendance_service::check_out(&pool, employee_id, company_id, None, None)
        .await
        .expect_err("check_out must reject when no in-window open record exists");

    assert!(
        format!("{err:?}").contains("No active check-in"),
        "expected 'No active check-in' error, got: {err:?}"
    );
}

#[tokio::test]
async fn check_out_is_scoped_to_company() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_a, employee_a) = seed_company_and_employee(&pool).await;
    let (company_b, _employee_b) = seed_company_and_employee(&pool).await;

    // Employee A has an open record at their real company, but the caller
    // asserts company_id = company_b. That mismatch must not close the record.
    insert_open_record(&pool, company_a, employee_a, 4).await;

    let err = attendance_service::check_out(&pool, employee_a, company_b, None, None)
        .await
        .expect_err("check_out must not cross company boundaries");
    assert!(format!("{err:?}").contains("No active check-in"));
}
