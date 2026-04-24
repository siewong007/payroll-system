use crate::services::attendance_service;
use crate::tests::support::{seed_company_and_employee, skip_if_no_db};

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
