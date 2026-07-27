use chrono::{Duration, NaiveDate, NaiveTime};
use rust_decimal::Decimal;

use crate::core::error::AppError;
use crate::models::attendance::{
    AttendanceExportQuery, AttendanceSummaryQuery, ManualAttendanceRequest,
    UpdateAttendanceRecordRequest,
};
use crate::models::company_location::{CreateLocationRequest, UpdateLocationRequest};
use crate::models::work_schedule::{CreateWorkScheduleRequest, UpdateWorkScheduleRequest};
use crate::repositories::attendance_records;
use crate::repositories::reads::attendance as attendance_reads;
use crate::services::{attendance_service, geofence_service, work_schedule_service};
use crate::tests::support::{
    seed_company, seed_company_and_employee, seed_employee, seed_user, skip_if_no_db,
};

const KL: &str = "Asia/Kuala_Lumpur";

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

    let status = attendance_service::determine_checkin_status(
        &pool,
        employee_id,
        company_id,
        "Asia/Kuala_Lumpur",
    )
    .await
    .expect("status determination should succeed");

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

    let record = attendance_service::check_out(&pool, employee_id, company_id, None, None, None)
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

    let err = attendance_service::check_out(&pool, employee_id, company_id, None, None, None)
        .await
        .expect_err("check_out must reject when no in-window open record exists");

    // The stale session is reported as such (with the admin-correction hint),
    // not as "no active check-in" — that advice bounced employees between
    // check-in and check-out errors with no way out.
    assert!(
        format!("{err:?}").contains("more than 24 hours old"),
        "expected stale-session error, got: {err:?}"
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

    let err = attendance_service::check_out(&pool, employee_a, company_b, None, None, None)
        .await
        .expect_err("check_out must not cross company boundaries");
    assert!(format!("{err:?}").contains("No active check-in"));
}

// ─── Auto-absent marking ───

/// Make every day a working day for the company so mark_absent tests can pin
/// arbitrary dates without weekday sensitivity.
async fn seed_all_working_days(pool: &sqlx::PgPool, company_id: uuid::Uuid) {
    for dow in 0..7 {
        sqlx::query(
            "INSERT INTO working_day_config (company_id, day_of_week, is_working_day)
             VALUES ($1, $2, TRUE)",
        )
        .bind(company_id)
        .bind(dow as i16)
        .execute(pool)
        .await
        .expect("insert working_day_config");
    }
}

#[tokio::test]
async fn mark_absent_skips_leave_existing_records_and_is_idempotent() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, emp_absent) = seed_company_and_employee(&pool).await;
    let emp_on_leave = seed_employee(&pool, company_id, None, 500_000).await;
    let emp_with_record = seed_employee(&pool, company_id, None, 500_000).await;
    seed_all_working_days(&pool, company_id).await;

    let target = NaiveDate::from_ymd_opt(2026, 6, 3).unwrap();

    // Approved leave covering the target date.
    let leave_type_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO leave_types (company_id, name) VALUES ($1, 'Test Leave') RETURNING id",
    )
    .bind(company_id)
    .fetch_one(&pool)
    .await
    .expect("insert leave type");
    sqlx::query(
        r#"INSERT INTO leave_requests
           (employee_id, company_id, leave_type_id, start_date, end_date, days, status)
           VALUES ($1, $2, $3, $4, $4, 1, 'approved')"#,
    )
    .bind(emp_on_leave)
    .bind(company_id)
    .bind(leave_type_id)
    .bind(target)
    .execute(&pool)
    .await
    .expect("insert leave request");

    // An existing record on the target local date.
    sqlx::query(
        r#"INSERT INTO attendance_records
           (company_id, employee_id, check_in_at, check_out_at, method, status)
           VALUES ($1, $2, ($3::date + TIME '09:00')::timestamp AT TIME ZONE 'Asia/Kuala_Lumpur',
                   ($3::date + TIME '18:00')::timestamp AT TIME ZONE 'Asia/Kuala_Lumpur',
                   'manual', 'present')"#,
    )
    .bind(company_id)
    .bind(emp_with_record)
    .bind(target)
    .execute(&pool)
    .await
    .expect("insert existing record");

    let marked = attendance_service::mark_absent_for_date(&pool, KL, target, Some(company_id))
        .await
        .expect("mark_absent should succeed");
    assert_eq!(
        marked, 1,
        "only the employee without leave or a record is marked (got {marked})"
    );

    let absent_employee: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT employee_id FROM attendance_records
         WHERE company_id = $1 AND status = 'absent' AND created_by IS NULL",
    )
    .bind(company_id)
    .fetch_optional(&pool)
    .await
    .expect("query absent rows");
    assert_eq!(absent_employee, Some(emp_absent));

    // Re-running the same date inserts nothing.
    let again = attendance_service::mark_absent_for_date(&pool, KL, target, Some(company_id))
        .await
        .expect("second run should succeed");
    assert_eq!(again, 0, "mark_absent must be idempotent per date");
}

#[tokio::test]
async fn mark_absent_respects_the_employment_window() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, _employed) = seed_company_and_employee(&pool).await;
    seed_all_working_days(&pool, company_id).await;

    // Backfilling a date before the employee joined must not invent an
    // absence. seed_employee sets date_joined = 2020-01-01, so pick earlier.
    let before_joining = NaiveDate::from_ymd_opt(2019, 6, 5).unwrap();
    let marked =
        attendance_service::mark_absent_for_date(&pool, KL, before_joining, Some(company_id))
            .await
            .expect("mark_absent should succeed");
    assert_eq!(
        marked, 0,
        "an employee cannot be absent before they were hired"
    );

    // ...and not after they resigned.
    let resigned = seed_employee(&pool, company_id, None, 500_000).await;
    sqlx::query("UPDATE employees SET date_resigned = $2 WHERE id = $1")
        .bind(resigned)
        .bind(NaiveDate::from_ymd_opt(2026, 5, 31).unwrap())
        .execute(&pool)
        .await
        .expect("set date_resigned");

    let after_leaving = NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();
    attendance_service::mark_absent_for_date(&pool, KL, after_leaving, Some(company_id))
        .await
        .expect("mark_absent should succeed");

    let leaver_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendance_records WHERE employee_id = $1 AND status = 'absent'",
    )
    .bind(resigned)
    .fetch_one(&pool)
    .await
    .expect("count leaver rows");
    assert_eq!(
        leaver_rows, 0,
        "a resigned employee must not accrue absences after leaving"
    );
}

#[tokio::test]
async fn mark_absent_skips_recurring_holidays_from_prior_years() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, _employee_id) = seed_company_and_employee(&pool).await;
    seed_all_working_days(&pool, company_id).await;

    // Recurring holiday saved in 2024; target is the same month/day in 2026.
    sqlx::query(
        "INSERT INTO holidays (company_id, name, date, is_recurring)
         VALUES ($1, 'Recurring Day', '2024-06-04', TRUE)",
    )
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("insert holiday");

    let target = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
    let marked = attendance_service::mark_absent_for_date(&pool, KL, target, Some(company_id))
        .await
        .expect("mark_absent should succeed");
    assert_eq!(
        marked, 0,
        "a recurring holiday must suppress absent marking"
    );
}

#[tokio::test]
async fn checkin_supersedes_todays_auto_absent_row() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;

    // Simulate the cron having already marked the employee absent today.
    let today: NaiveDate =
        sqlx::query_scalar("SELECT (NOW() AT TIME ZONE 'Asia/Kuala_Lumpur')::date")
            .fetch_one(&pool)
            .await
            .expect("query today");
    let marked = attendance_service::mark_absent_for_date(&pool, KL, today, Some(company_id))
        .await
        .expect("mark absent today");
    // Weekend runs insert nothing (default Mon-Fri fallback would apply), so
    // force the placeholder row directly if the date wasn't a working day.
    if marked == 0 {
        sqlx::query(
            r#"INSERT INTO attendance_records
               (company_id, employee_id, check_in_at, check_out_at, method, status, notes)
               VALUES ($1, $2, ($3::date)::timestamp AT TIME ZONE 'Asia/Kuala_Lumpur',
                       ($3::date)::timestamp AT TIME ZONE 'Asia/Kuala_Lumpur',
                       'manual', 'absent', 'Auto-marked absent (no check-in recorded)')"#,
        )
        .bind(company_id)
        .bind(employee_id)
        .bind(today)
        .execute(&pool)
        .await
        .expect("insert absent placeholder");
    }

    // A real (service-level) check-in afterwards must remove the placeholder.
    let record =
        attendance_service::check_in_face_id(&pool, employee_id, company_id, None, None, None)
            .await
            .expect("check-in should succeed after auto-absent");
    assert_ne!(record.status, "absent");

    let absent_left: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendance_records
         WHERE employee_id = $1 AND status = 'absent' AND created_by IS NULL",
    )
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("count absent rows");
    assert_eq!(absent_left, 0, "auto-absent placeholder must be superseded");
}

// ─── Summary semantics ───

#[tokio::test]
async fn summary_counts_distinct_days_with_status_precedence() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;

    let day1 = NaiveDate::from_ymd_opt(2026, 6, 8).unwrap();
    let day2 = NaiveDate::from_ymd_opt(2026, 6, 9).unwrap();

    // Day 1: auto-absent placeholder AND a late check-in (the cron-then-arrive case).
    sqlx::query(
        r#"INSERT INTO attendance_records
           (company_id, employee_id, check_in_at, check_out_at, method, status, notes)
           VALUES ($1, $2, ($3::date)::timestamp AT TIME ZONE 'Asia/Kuala_Lumpur',
                   ($3::date)::timestamp AT TIME ZONE 'Asia/Kuala_Lumpur',
                   'manual', 'absent', 'Auto-marked absent (no check-in recorded)')"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .bind(day1)
    .execute(&pool)
    .await
    .expect("insert absent row");
    sqlx::query(
        r#"INSERT INTO attendance_records
           (company_id, employee_id, check_in_at, check_out_at, method, status)
           VALUES ($1, $2, ($3::date + TIME '14:00')::timestamp AT TIME ZONE 'Asia/Kuala_Lumpur',
                   ($3::date + TIME '18:00')::timestamp AT TIME ZONE 'Asia/Kuala_Lumpur',
                   'manual', 'late')"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .bind(day1)
    .execute(&pool)
    .await
    .expect("insert late row");

    // Day 2: a split shift — two present sessions on one local date.
    for (start, end) in [("08:00", "12:00"), ("13:00", "17:00")] {
        sqlx::query(&format!(
            r#"INSERT INTO attendance_records
               (company_id, employee_id, check_in_at, check_out_at, method, status)
               VALUES ($1, $2, ($3::date + TIME '{start}')::timestamp AT TIME ZONE 'Asia/Kuala_Lumpur',
                       ($3::date + TIME '{end}')::timestamp AT TIME ZONE 'Asia/Kuala_Lumpur',
                       'manual', 'present')"#,
        ))
        .bind(company_id)
        .bind(employee_id)
        .bind(day2)
        .execute(&pool)
        .await
        .expect("insert present row");
    }

    let q = AttendanceSummaryQuery {
        date_from: day1,
        date_to: day2,
        employee_id: Some(employee_id),
        department: None,
    };
    let items = attendance_service::get_attendance_summary(&pool, company_id, &q)
        .await
        .expect("summary should succeed");
    let item = items
        .iter()
        .find(|i| i.employee_id == employee_id)
        .expect("employee present in summary");

    assert_eq!(item.late_days, 1, "cron+late day counts once, as late");
    assert_eq!(item.absent_days, 0, "superseded absent must not be counted");
    assert_eq!(item.present_days, 1, "split shift is one present day");
    assert_eq!(item.half_days, 0);
}

// ─── Manual entry tenant scoping ───

#[tokio::test]
async fn manual_attendance_rejects_cross_company_employee() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_a, _employee_a) = seed_company_and_employee(&pool).await;
    let (_company_b, employee_b) = seed_company_and_employee(&pool).await;
    let admin = seed_user(&pool, company_a, "admin").await;

    let req = ManualAttendanceRequest {
        employee_id: employee_b,
        check_in_at: chrono::Utc::now(),
        check_out_at: None,
        status: Some("present".into()),
        notes: None,
    };

    let err = attendance_service::manual_attendance(&pool, company_a, req, admin, None)
        .await
        .expect_err("cross-company manual entry must be rejected");
    assert!(
        matches!(err, AppError::NotFound(_)),
        "expected NotFound, got: {err:?}"
    );
}

// ─── Correction workflow ───

#[tokio::test]
async fn correction_requires_reason_supports_clears_and_writes_scoped_audit() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;
    let admin = seed_user(&pool, company_id, "hr_manager").await;

    let record_id: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO attendance_records
           (company_id, employee_id, check_in_at, check_out_at, method, status, notes, hours_worked)
           VALUES ($1, $2, NOW() - INTERVAL '9 hours', NOW() - INTERVAL '1 hour',
                   'manual', 'present', 'wrong note', 8.0)
           RETURNING id"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("insert record");

    // Missing reason is rejected.
    let no_reason = UpdateAttendanceRecordRequest {
        check_in_at: None,
        check_out_at: None,
        status: None,
        notes: None,
        clear_check_out: None,
        clear_notes: None,
        reason: "   ".into(),
    };
    let err = attendance_service::update_attendance_record(
        &pool, company_id, record_id, &no_reason, admin, None,
    )
    .await
    .expect_err("a blank reason must be rejected");
    assert!(matches!(err, AppError::BadRequest(_)));

    // Clearing check-out reopens the session and clears derived hours.
    let clear = UpdateAttendanceRecordRequest {
        check_in_at: None,
        check_out_at: None,
        status: None,
        notes: None,
        clear_check_out: Some(true),
        clear_notes: Some(true),
        reason: "Recorded against the wrong shift".into(),
    };
    let record = attendance_service::update_attendance_record(
        &pool, company_id, record_id, &clear, admin, None,
    )
    .await
    .expect("correction should succeed");
    assert!(record.check_out_at.is_none(), "check-out must be cleared");
    assert!(record.notes.is_none(), "notes must be cleared");
    assert!(
        record.hours_worked.is_none(),
        "hours must be recomputed away"
    );
    assert!(record.overtime_hours.is_none());

    // The audit row is company-scoped (visible to the audit UI) and carries the reason.
    let (audit_company, description): (Option<uuid::Uuid>, Option<String>) = sqlx::query_as(
        r#"SELECT company_id, description FROM audit_logs
           WHERE entity_type = 'attendance_record' AND action = 'update' AND entity_id = $1
           ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(record_id)
    .fetch_one(&pool)
    .await
    .expect("audit row must exist");
    assert_eq!(audit_company, Some(company_id));
    assert!(
        description
            .unwrap_or_default()
            .contains("Recorded against the wrong shift"),
        "audit description must carry the reason"
    );
}

#[tokio::test]
async fn correction_rejects_an_overlong_reason_instead_of_failing_the_transaction() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;
    let admin = seed_user(&pool, company_id, "hr_manager").await;

    let record_id: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO attendance_records
           (company_id, employee_id, check_in_at, check_out_at, method, status)
           VALUES ($1, $2, NOW() - INTERVAL '8 hours', NOW(), 'manual', 'present')
           RETURNING id"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("insert record");

    // The reason lands in audit_logs.description (varchar(500)). Without a
    // bound this overflowed and rolled back the correction as an opaque 500.
    let req = UpdateAttendanceRecordRequest {
        check_in_at: None,
        check_out_at: None,
        status: None,
        notes: None,
        clear_check_out: None,
        clear_notes: None,
        reason: "x".repeat(600),
    };

    let err = attendance_service::update_attendance_record(
        &pool, company_id, record_id, &req, admin, None,
    )
    .await
    .expect_err("an over-long reason must be rejected");
    assert!(
        matches!(err, AppError::BadRequest(_)),
        "expected an actionable 400, got: {err:?}"
    );
}

#[tokio::test]
async fn reopening_a_session_conflicts_cleanly_when_another_is_open() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;
    let admin = seed_user(&pool, company_id, "hr_manager").await;

    // A closed record, plus a second session left open for the same employee.
    let closed_id: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO attendance_records
           (company_id, employee_id, check_in_at, check_out_at, method, status)
           VALUES ($1, $2, NOW() - INTERVAL '5 hours', NOW() - INTERVAL '4 hours', 'manual', 'present')
           RETURNING id"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("insert closed record");
    insert_open_record(&pool, company_id, employee_id, 1).await;

    let req = UpdateAttendanceRecordRequest {
        check_in_at: None,
        check_out_at: None,
        status: None,
        notes: None,
        clear_check_out: Some(true),
        clear_notes: None,
        reason: "Undo an accidental check-out".into(),
    };

    // The one-open-session index rejects this; the admin must get an
    // explanation, not a bare database error.
    let err = attendance_service::update_attendance_record(
        &pool, company_id, closed_id, &req, admin, None,
    )
    .await
    .expect_err("reopening must conflict with the existing open session");
    assert!(
        matches!(err, AppError::Conflict(_)),
        "expected Conflict, got: {err:?}"
    );
}

// ─── Derived overtime ceiling ───

/// A session left open overnight closes with truthful hours but no overtime.
///
/// Check-out matches any open record inside 24h (deliberately, for night
/// shifts), and the derived figure was written from wall-clock elapsed time with
/// no ceiling — so a Monday check-in closed on Tuesday produced ~14 h of paid
/// overtime. Above the ceiling the figure is left unrated rather than clamped:
/// clamping would still pay the ceiling and erase the evidence, while NULL means
/// "a human must decide" and is skipped by the payroll SUM.
#[tokio::test]
async fn forgotten_check_out_leaves_overtime_unrated() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;

    insert_open_record(&pool, company_id, employee_id, 23).await;

    let record = attendance_service::check_out(&pool, employee_id, company_id, None, None, None)
        .await
        .expect("check_out must still close the session");

    let hours = record.hours_worked.expect("hours worked are recorded");
    assert!(
        hours >= Decimal::new(2295, 2) && hours <= Decimal::new(2305, 2),
        "hours_worked stays truthful — it is what the clock says: {hours}"
    );
    assert!(
        record.overtime_hours.is_none(),
        "14 h of derived overtime is an anomaly, not a payment: {:?}",
        record.overtime_hours
    );
}

/// The ceiling must not swallow legitimate overtime.
#[tokio::test]
async fn a_normal_overtime_check_out_is_still_rated() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;

    // 11 h against the 9 h default shift → 2 h, comfortably under the ceiling.
    insert_open_record(&pool, company_id, employee_id, 11).await;

    let record = attendance_service::check_out(&pool, employee_id, company_id, None, None, None)
        .await
        .expect("check_out should succeed");

    assert_eq!(
        record.overtime_hours,
        Some(Decimal::new(200, 2)),
        "ordinary overtime must still be rated"
    );
}

/// The ceiling is a company setting, not a constant: a tenant that raises or
/// lowers it changes what check-out will rate.
#[tokio::test]
async fn the_overtime_ceiling_is_read_from_company_settings() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;

    sqlx::query(
        r#"INSERT INTO company_settings (company_id, category, key, value)
           VALUES ($1, 'payroll', 'max_overtime_hours_per_day', '"1"'::jsonb)
           ON CONFLICT (company_id, category, key) DO UPDATE SET value = EXCLUDED.value"#,
    )
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("set the company ceiling");

    // The same 2 h that the previous test rates is now above the ceiling.
    insert_open_record(&pool, company_id, employee_id, 11).await;

    let record = attendance_service::check_out(&pool, employee_id, company_id, None, None, None)
        .await
        .expect("check_out should succeed");

    assert!(
        record.overtime_hours.is_none(),
        "a 1 h ceiling must leave 2 h unrated: {:?}",
        record.overtime_hours
    );
}

/// The HR correction path is the designated remedy for an unrated record, so it
/// is not subject to the per-day overtime ceiling — capping it there would leave
/// an operator with no way to record a genuine long shift. The only bound is the
/// 24-hour session span (see `a_sixty_day_correction_is_refused_before_the_write`),
/// and this 23-hour shift is deliberately just inside it.
#[tokio::test]
async fn an_hr_correction_can_still_record_long_overtime() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;
    let admin = seed_user(&pool, company_id, "hr_manager").await;

    let record_id: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO attendance_records
           (company_id, employee_id, check_in_at, check_out_at, method, status)
           VALUES ($1, $2, NOW() - INTERVAL '23 hours', NOW() - INTERVAL '22 hours',
                   'manual', 'present')
           RETURNING id"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("insert record");

    let req = UpdateAttendanceRecordRequest {
        check_in_at: None,
        check_out_at: Some(chrono::Utc::now()),
        status: None,
        notes: None,
        clear_check_out: None,
        clear_notes: None,
        reason: "Genuine 23-hour incident shift, confirmed with the supervisor".into(),
    };

    let record = attendance_service::update_attendance_record(
        &pool, company_id, record_id, &req, admin, None,
    )
    .await
    .expect("correction should succeed");

    let overtime = record
        .overtime_hours
        .expect("a correction must not be nulled by the check-out ceiling");
    assert!(
        overtime > Decimal::from(4),
        "the corrected figure is recorded as stated: {overtime}"
    );

    let audit_rows: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM audit_logs
           WHERE entity_type = 'attendance_record' AND action = 'update' AND entity_id = $1"#,
    )
    .bind(record_id)
    .fetch_one(&pool)
    .await
    .expect("count audit rows");
    assert_eq!(audit_rows, 1, "the correction must leave an audit trail");
}

// ─── Per-tenant timezones ───

/// Give the company a default work schedule on `tz` (times keep their defaults).
async fn seed_default_schedule(pool: &sqlx::PgPool, company_id: uuid::Uuid, tz: &str) {
    sqlx::query(
        "INSERT INTO company_work_schedules (company_id, name, timezone, is_default)
         VALUES ($1, 'Default', $2, TRUE)",
    )
    .bind(company_id)
    .bind(tz)
    .execute(pool)
    .await
    .expect("insert default work schedule");
}

/// `(local date, local time-of-day)` in `tz` per the database clock.
async fn local_now(pool: &sqlx::PgPool, tz: &str) -> (NaiveDate, NaiveTime) {
    sqlx::query_as("SELECT (NOW() AT TIME ZONE $1)::date, (NOW() AT TIME ZONE $1)::time")
        .bind(tz)
        .fetch_one(pool)
        .await
        .expect("read the database clock")
}

/// Mirrors the service's cutoff rule: today is only owed once 12:30 local has
/// passed, otherwise the last owed date is yesterday.
fn expected_last_due(today: NaiveDate, now_local: NaiveTime) -> NaiveDate {
    if now_local >= NaiveTime::from_hms_opt(12, 30, 0).expect("valid cutoff") {
        today
    } else {
        today - Duration::days(1)
    }
}

async fn set_absent_bookmark(pool: &sqlx::PgPool, company_id: uuid::Uuid, date: NaiveDate) {
    sqlx::query("UPDATE companies SET auto_absent_last_run_date = $2 WHERE id = $1")
        .bind(company_id)
        .bind(date)
        .execute(pool)
        .await
        .expect("set the auto-absent bookmark");
}

async fn absent_bookmark(pool: &sqlx::PgPool, company_id: uuid::Uuid) -> Option<NaiveDate> {
    sqlx::query_scalar("SELECT auto_absent_last_run_date FROM companies WHERE id = $1")
        .bind(company_id)
        .fetch_one(pool)
        .await
        .expect("read the auto-absent bookmark")
}

/// Reproduce a row written before migration 1015 existed. The trigger is the
/// point of that migration, so planting a value it rejects means stepping
/// around it deliberately — re-enabled even if the write fails, or every later
/// test would run without the guard.
async fn plant_corrupt_timezone(pool: &sqlx::PgPool, company_id: uuid::Uuid, bad: &str) {
    sqlx::query(
        "ALTER TABLE company_work_schedules \
         DISABLE TRIGGER company_work_schedules_timezone_valid",
    )
    .execute(pool)
    .await
    .expect("disable the timezone trigger");

    let planted = sqlx::query(
        "UPDATE company_work_schedules SET timezone = $2
         WHERE company_id = $1 AND is_default = TRUE",
    )
    .bind(company_id)
    .bind(bad)
    .execute(pool)
    .await;

    sqlx::query(
        "ALTER TABLE company_work_schedules \
         ENABLE TRIGGER company_work_schedules_timezone_valid",
    )
    .execute(pool)
    .await
    .expect("re-enable the timezone trigger");

    planted.expect("plant the corrupt timezone");
}

/// The stored zone is interpolated into `AT TIME ZONE` by every attendance
/// read and write, so an unrecognised one is not a cosmetic setting error —
/// it is a 500 on every check-in for that tenant. Reject it at the service,
/// where the other schedule fields are already validated.
#[tokio::test]
async fn work_schedule_writes_reject_an_unrecognised_timezone() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let admin = seed_user(&pool, company_id, "hr_manager").await;

    let create = |tz: &str| CreateWorkScheduleRequest {
        name: None,
        start_time: "09:00".into(),
        end_time: "18:00".into(),
        grace_minutes: None,
        half_day_hours: None,
        timezone: Some(tz.to_string()),
    };

    let err = work_schedule_service::upsert_default_schedule(
        &pool,
        company_id,
        &create("Asia/Kuala_Lumpr"),
        admin,
        None,
    )
    .await
    .expect_err("a typo'd zone must not reach the column");
    assert!(
        matches!(err, AppError::BadRequest(_)),
        "expected an actionable 400, got: {err:?}"
    );
    assert!(
        work_schedule_service::get_default_schedule(&pool, company_id)
            .await
            .expect("read schedule")
            .is_none(),
        "a rejected request must leave the tenant unconfigured, not half-configured"
    );

    // A non-MYT zone is perfectly legitimate and round-trips.
    let created = work_schedule_service::upsert_default_schedule(
        &pool,
        company_id,
        &create("Asia/Jakarta"),
        admin,
        None,
    )
    .await
    .expect("a real IANA zone must be accepted");
    assert_eq!(created.timezone, "Asia/Jakarta");

    let bad_update = UpdateWorkScheduleRequest {
        name: None,
        start_time: None,
        end_time: None,
        grace_minutes: None,
        half_day_hours: None,
        timezone: Some("Asia/Kuala_Lumpr".into()),
    };
    let err = work_schedule_service::update_schedule(
        &pool,
        company_id,
        created.id,
        &bad_update,
        admin,
        None,
    )
    .await
    .expect_err("the update path must be guarded too");
    assert!(matches!(err, AppError::BadRequest(_)), "got: {err:?}");

    let stored = work_schedule_service::get_default_schedule(&pool, company_id)
        .await
        .expect("read schedule")
        .expect("schedule still exists");
    assert_eq!(
        stored.timezone, "Asia/Jakarta",
        "a rejected update must not disturb the stored zone"
    );
}

/// The platform-wide wedge: one tenant with an unusable stored zone used to
/// abort the whole catch-up before the shared bookmark was written, so every
/// other tenant's absences stopped being marked until an operator fixed the
/// row by hand — and the owed dates aged out of the backfill window meanwhile.
#[tokio::test]
async fn a_wedged_tenant_no_longer_halts_the_absent_run_and_heals_itself() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let (wedged, _wedged_employee) = seed_company_and_employee(&pool).await;
    let (healthy, _healthy_employee) = seed_company_and_employee(&pool).await;
    for company_id in [wedged, healthy] {
        seed_all_working_days(&pool, company_id).await;
        seed_default_schedule(&pool, company_id, KL).await;
    }

    // Defence in depth: any write path that skips the service validator is
    // still refused by the migration-1015 trigger.
    let rejected = sqlx::query(
        "UPDATE company_work_schedules SET timezone = 'Asia/Kuala_Lumpr'
         WHERE company_id = $1 AND is_default = TRUE",
    )
    .bind(wedged)
    .execute(&pool)
    .await;
    assert!(
        rejected.is_err(),
        "the database guard must refuse an unknown zone"
    );

    plant_corrupt_timezone(&pool, wedged, "Asia/Kuala_Lumpr").await;

    let (today, now_local) = local_now(&pool, KL).await;
    let last_due = expected_last_due(today, now_local);
    let seeded_bookmark = last_due - Duration::days(3);
    set_absent_bookmark(&pool, wedged, seeded_bookmark).await;
    set_absent_bookmark(&pool, healthy, seeded_bookmark).await;

    attendance_service::run_auto_absent_catchup(&pool)
        .await
        .expect("one bad tenant must not fail the run");

    assert_eq!(
        absent_bookmark(&pool, healthy).await,
        Some(last_due),
        "the healthy tenant must be marked and bookmarked as usual"
    );
    assert_eq!(
        absent_bookmark(&pool, wedged).await,
        Some(seeded_bookmark),
        "the skipped tenant's bookmark must not advance over dates it never ran"
    );

    let healthy_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendance_records
         WHERE company_id = $1 AND status = 'absent' AND created_by IS NULL",
    )
    .bind(healthy)
    .fetch_one(&pool)
    .await
    .expect("count absent rows");
    assert!(
        healthy_rows > 0,
        "the healthy tenant's owed dates must actually have been marked"
    );

    // Recovery needs no operator SQL: correcting the zone through the ordinary
    // settings endpoint is enough, and the next tick backfills what was owed.
    let schedule = work_schedule_service::get_default_schedule(&pool, wedged)
        .await
        .expect("read schedule")
        .expect("schedule exists");
    work_schedule_service::update_schedule(
        &pool,
        wedged,
        schedule.id,
        &UpdateWorkScheduleRequest {
            name: None,
            start_time: None,
            end_time: None,
            grace_minutes: None,
            half_day_hours: None,
            timezone: Some(KL.to_string()),
        },
        seed_user(&pool, wedged, "hr_manager").await,
        None,
    )
    .await
    .expect("an admin can repair the zone");

    attendance_service::run_auto_absent_catchup(&pool)
        .await
        .expect("catch-up should succeed");

    assert_eq!(
        absent_bookmark(&pool, wedged).await,
        Some(last_due),
        "the repaired tenant must catch up on its own, from its own bookmark"
    );
}

/// R1-H13: the daily tick fires once, on one UTC instant. Deciding "today"
/// from a single shared zone marked a tenant west of MYT absent for a date
/// that had not started there yet — a row `mark_absent_for_company_date`
/// refuses to create, and one the dashboard, summary and export all show.
#[tokio::test]
async fn auto_absent_uses_each_tenant_own_calendar_and_bookmark() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    const HNL: &str = "Pacific/Honolulu"; // UTC-10: 18 hours behind MYT.

    let (west, _west_employee) = seed_company_and_employee(&pool).await;
    let (east, _east_employee) = seed_company_and_employee(&pool).await;
    seed_all_working_days(&pool, west).await;
    seed_all_working_days(&pool, east).await;
    seed_default_schedule(&pool, west, HNL).await;
    seed_default_schedule(&pool, east, KL).await;

    for (company_id, tz) in [(west, HNL), (east, KL)] {
        let (today, now_local) = local_now(&pool, tz).await;
        let bookmark = expected_last_due(today, now_local) - Duration::days(2);
        set_absent_bookmark(&pool, company_id, bookmark).await;
    }

    attendance_service::run_auto_absent_catchup(&pool)
        .await
        .expect("catch-up should succeed");

    // The defect itself: a placeholder dated after the tenant's own today.
    let future_dated: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendance_records
         WHERE company_id = $1
           AND (check_in_at AT TIME ZONE $2)::date > (NOW() AT TIME ZONE $2)::date",
    )
    .bind(west)
    .bind(HNL)
    .fetch_one(&pool)
    .await
    .expect("count future-dated rows");
    assert_eq!(
        future_dated, 0,
        "no tenant may be marked absent for a date that has not started there"
    );

    // Each bookmark tracks its own calendar, so the two may legitimately sit a
    // day apart. (Read after the run: the two clock reads straddle the same
    // instant, so a local 12:30 or midnight crossing in between would show up
    // here as an off-by-one.)
    for (company_id, tz) in [(west, HNL), (east, KL)] {
        let (today, now_local) = local_now(&pool, tz).await;
        assert_eq!(
            absent_bookmark(&pool, company_id).await,
            Some(expected_last_due(today, now_local)),
            "{tz} must be bookmarked on its own local due date"
        );
    }

    // Re-running owes nothing: the bookmarks stay put and no rows are added.
    let before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendance_records
         WHERE company_id = ANY($1) AND status = 'absent'",
    )
    .bind(vec![west, east])
    .fetch_one(&pool)
    .await
    .expect("count absent rows");
    let west_bookmark = absent_bookmark(&pool, west).await;
    let east_bookmark = absent_bookmark(&pool, east).await;

    attendance_service::run_auto_absent_catchup(&pool)
        .await
        .expect("catch-up should succeed");

    let after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendance_records
         WHERE company_id = ANY($1) AND status = 'absent'",
    )
    .bind(vec![west, east])
    .fetch_one(&pool)
    .await
    .expect("count absent rows");
    assert_eq!(after, before, "a second run must be a no-op");
    assert_eq!(absent_bookmark(&pool, west).await, west_bookmark);
    assert_eq!(absent_bookmark(&pool, east).await, east_bookmark);
}

/// Without an explicit order the tenant sequence varied run to run, so which
/// companies had been reached when a run aborted was nondeterministic — and
/// the log stream could not be compared between ticks.
#[tokio::test]
async fn auto_absent_targets_are_ordered_by_company_id() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let mut seeded = Vec::new();
    for _ in 0..3 {
        seeded.push(seed_company(&pool).await);
    }
    seeded.sort();

    // Twice: the order must be a property of the query, not of whatever the
    // planner happened to do. (Other tests seed companies concurrently, so the
    // two result sets need not be identical — only individually ordered.)
    for _ in 0..2 {
        let targets = attendance_reads::auto_absent_targets(&pool)
            .await
            .expect("read auto-absent targets");
        let ids: Vec<uuid::Uuid> = targets.iter().map(|t| t.company_id).collect();

        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "targets must come back ordered by company id");

        let mine: Vec<uuid::Uuid> = ids.into_iter().filter(|id| seeded.contains(id)).collect();
        assert_eq!(
            mine, seeded,
            "every seeded company must appear, in id order"
        );
    }
}

// ─── Auto-absent placeholder uniqueness (migration 1019) ───

/// Insert one auto-absent-shaped row directly, bypassing `mark_absent`, so the
/// index predicate itself can be probed. `notes` is the discriminator: only the
/// exact marker string is covered by the partial unique index.
async fn insert_placeholder_row(
    pool: &sqlx::PgPool,
    company_id: uuid::Uuid,
    employee_id: uuid::Uuid,
    at: chrono::DateTime<chrono::Utc>,
    notes: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO attendance_records
           (company_id, employee_id, check_in_at, check_out_at, method, status, notes)
           VALUES ($1, $2, $3, $3, 'manual', 'absent', $4)"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .bind(at)
    .bind(notes)
    .execute(pool)
    .await
    .map(|_| ())
}

/// The defect: two simultaneous runs both passed the `NOT EXISTS` under READ
/// COMMITTED and both inserted a full set of placeholders. The daily timer
/// racing the startup pass, or a double-clicked admin backfill, is enough.
#[tokio::test]
async fn concurrent_absent_runs_insert_one_placeholder_not_two() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;
    seed_all_working_days(&pool, company_id).await;

    let target = NaiveDate::from_ymd_opt(2026, 6, 17).unwrap();
    let (first, second) = tokio::join!(
        attendance_records::mark_absent(&pool, KL, target, Some(company_id)),
        attendance_records::mark_absent(&pool, KL, target, Some(company_id)),
    );
    let inserted = first.expect("first run") + second.expect("second run");

    let rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendance_records WHERE employee_id = $1 AND status = 'absent'",
    )
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("count placeholders");

    assert_eq!(rows, 1, "the day must carry exactly one placeholder");
    assert_eq!(
        inserted, 1,
        "the reported count must be genuine inserts, not double-counted"
    );
}

/// The placeholder `mark_absent` writes must carry the marker note the index
/// predicate (and `delete_auto_absent_today`) match on. If the Rust constant
/// and the SQL literal drift, nothing errors — the writes simply stop matching
/// and duplicates come back silently.
#[tokio::test]
async fn a_written_placeholder_carries_the_marker_note() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;
    seed_all_working_days(&pool, company_id).await;

    let target = NaiveDate::from_ymd_opt(2026, 6, 18).unwrap();
    attendance_records::mark_absent(&pool, KL, target, Some(company_id))
        .await
        .expect("mark absent");

    let notes: Option<String> = sqlx::query_scalar(
        "SELECT notes FROM attendance_records WHERE employee_id = $1 AND status = 'absent'",
    )
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("read the placeholder note");
    let marker = attendance_records::AUTO_ABSENT_NOTE;
    assert_eq!(notes.as_deref(), Some(marker));
}

/// The other half of the same coupling: 1019's index predicate has to spell the
/// marker note out as a literal (index predicates cannot be parameterised), so
/// the only thing keeping it in step with the Rust constant is this reflection.
#[tokio::test]
async fn the_auto_absent_index_predicate_matches_the_marker_note() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let indexdef: String = sqlx::query_scalar(
        "SELECT indexdef FROM pg_indexes
         WHERE schemaname = 'public'
           AND indexname = 'attendance_auto_absent_one_per_employee_day'",
    )
    .fetch_one(&pool)
    .await
    .expect("migration 1019 must have created the index");

    assert!(
        indexdef.contains(attendance_records::AUTO_ABSENT_NOTE),
        "the index predicate no longer matches AUTO_ABSENT_NOTE: {indexdef}"
    );
    assert!(
        indexdef.contains("UNIQUE"),
        "the index must be unique to stop the race: {indexdef}"
    );
}

/// The index is deliberately narrow: it constrains only cron-written rows. An
/// HR-edited placeholder falls outside the predicate, so it neither collides
/// with nor is deleted by anything the cron does. Pinning this makes the
/// narrowness a decision rather than an accident.
#[tokio::test]
async fn the_placeholder_index_constrains_only_cron_written_rows() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;
    let at = chrono::Utc::now() - Duration::days(400);
    let marker = attendance_records::AUTO_ABSENT_NOTE;

    insert_placeholder_row(&pool, company_id, employee_id, at, marker)
        .await
        .expect("the first cron-shaped placeholder inserts");

    let duplicate = insert_placeholder_row(&pool, company_id, employee_id, at, marker)
        .await
        .expect_err("a second cron-shaped placeholder must violate the index");
    assert!(
        matches!(&duplicate, sqlx::Error::Database(e) if e.code().as_deref() == Some("23505")),
        "expected a unique violation, got: {duplicate:?}"
    );

    // Same (employee, instant), HR-edited note → outside the predicate, allowed.
    let hr_note = "Reviewed with the supervisor; absence confirmed";
    insert_placeholder_row(&pool, company_id, employee_id, at, hr_note)
        .await
        .expect("an HR-edited row must not be constrained by the cron index");
}

// ─── Correction span (24 h) ───

/// The reported reproduction: correcting an open record with a check-out 60 days
/// later drove `hours_worked` past `numeric(5,2)`, which surfaced as a bare 500
/// and rolled the whole correction back. It must be a 400, before anything is
/// written.
#[tokio::test]
async fn a_sixty_day_correction_is_refused_before_the_write() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;
    let admin = seed_user(&pool, company_id, "hr_manager").await;

    let record_id = insert_open_record(&pool, company_id, employee_id, 2).await;

    let req = UpdateAttendanceRecordRequest {
        check_in_at: None,
        check_out_at: Some(chrono::Utc::now() + Duration::days(60)),
        status: None,
        notes: None,
        clear_check_out: None,
        clear_notes: None,
        reason: "Closing a very stale session".into(),
    };
    let err = attendance_service::update_attendance_record(
        &pool, company_id, record_id, &req, admin, None,
    )
    .await
    .expect_err("a 60-day session must not be writable");

    match &err {
        AppError::BadRequest(msg) => assert!(
            msg.contains("24 hours"),
            "the message must name the bound: {msg}"
        ),
        other => panic!("expected BadRequest, got: {other:?}"),
    }

    // Nothing may have been written: the transaction must never have opened.
    let check_out: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT check_out_at FROM attendance_records WHERE id = $1")
            .bind(record_id)
            .fetch_one(&pool)
            .await
            .expect("read the record back");
    assert!(check_out.is_none(), "the record is untouched");

    let audit_rows: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM audit_logs
           WHERE entity_type = 'attendance_record' AND entity_id = $1"#,
    )
    .bind(record_id)
    .fetch_one(&pool)
    .await
    .expect("count audit rows");
    assert_eq!(audit_rows, 0, "no audit row for a refusal");
}

/// The intentional behaviour change, pinned so it stays deliberate: closing a
/// days-old open session now requires correcting the check-in too. Recording
/// ~72 payable hours for it was never right, and the error says what to do.
#[tokio::test]
async fn closing_a_stale_session_requires_correcting_both_timestamps() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;
    let admin = seed_user(&pool, company_id, "hr_manager").await;

    let record_id = insert_open_record(&pool, company_id, employee_id, 72).await;
    let now = chrono::Utc::now();

    let stale = UpdateAttendanceRecordRequest {
        check_in_at: None,
        check_out_at: Some(now),
        status: None,
        notes: None,
        clear_check_out: None,
        clear_notes: None,
        reason: "Employee forgot to check out".into(),
    };
    let err = attendance_service::update_attendance_record(
        &pool, company_id, record_id, &stale, admin, None,
    )
    .await
    .expect_err("a 3-day span is past the cap");
    assert!(
        matches!(&err, AppError::BadRequest(msg) if msg.contains("Correct the check-in time")),
        "the message must name the remedy: {err:?}"
    );

    // The workflow is still completable — correct both ends.
    let both = UpdateAttendanceRecordRequest {
        check_in_at: Some(now - Duration::hours(8)),
        check_out_at: Some(now),
        status: None,
        notes: None,
        clear_check_out: None,
        clear_notes: None,
        reason: "Employee forgot to check out; times confirmed with the supervisor".into(),
    };
    let record = attendance_service::update_attendance_record(
        &pool, company_id, record_id, &both, admin, None,
    )
    .await
    .expect("an 8-hour correction must succeed");

    assert_eq!(
        record.hours_worked,
        Some(Decimal::new(800, 2)),
        "hours are recomputed from the corrected pair"
    );

    let audit_rows: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM audit_logs
           WHERE entity_type = 'attendance_record' AND entity_id = $1"#,
    )
    .bind(record_id)
    .fetch_one(&pool)
    .await
    .expect("count audit rows");
    assert_eq!(audit_rows, 1, "the completed correction is audited");
}

// ─── Geofence ───

async fn set_geofence_mode_directly(pool: &sqlx::PgPool, company_id: uuid::Uuid, mode: &str) {
    // Straight to the column: `set_geofence_mode` now refuses to arm an empty
    // location list, and these tests need to *start* from the state a tenant
    // configured before that guard existed.
    sqlx::query("UPDATE companies SET geofence_mode = $2 WHERE id = $1")
        .bind(company_id)
        .bind(mode)
        .execute(pool)
        .await
        .expect("set geofence mode");
}

async fn add_location(
    pool: &sqlx::PgPool,
    company_id: uuid::Uuid,
    actor: uuid::Uuid,
) -> uuid::Uuid {
    let req = CreateLocationRequest {
        name: "HQ".into(),
        latitude: 3.157_64,
        longitude: 101.711_86,
        radius_meters: Some(200),
    };
    geofence_service::create_location(pool, company_id, &req, actor, None)
        .await
        .expect("create location")
        .id
}

/// The fail-open half of the defect. With `enforce` and no active locations,
/// coordinates from anywhere on earth were accepted *and recorded as inside the
/// fence*, while an on-site employee whose browser denied GPS was rejected by
/// the same handler.
#[tokio::test]
async fn an_enforced_geofence_with_no_locations_refuses_and_names_the_configuration() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    set_geofence_mode_directly(&pool, company_id, "enforce").await;

    // Reykjavik: nowhere near any Malaysian office.
    let err = geofence_service::validate_geofence(&pool, company_id, Some(64.14), Some(-21.94))
        .await
        .expect_err("an unevaluable enforced fence must fail closed");

    match &err {
        AppError::BadRequest(msg) => {
            assert!(
                msg.contains("no approved office locations are configured"),
                "the message must name the missing configuration: {msg}"
            );
            assert!(
                !msg.contains("You are outside"),
                "it must not send the employee hunting for an office: {msg}"
            );
        }
        other => panic!("expected BadRequest, got: {other:?}"),
    }
}

#[tokio::test]
async fn a_warning_geofence_with_no_locations_flags_instead_of_recording_inside() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    set_geofence_mode_directly(&pool, company_id, "warn").await;

    let flagged = geofence_service::validate_geofence(&pool, company_id, Some(64.14), Some(-21.94))
        .await
        .expect("warn mode never rejects");
    assert!(
        flagged,
        "a fence that was never evaluated must not be recorded as 'inside'"
    );
}

/// Regression guard for the majority of tenants: a company that never turns
/// geofencing on must be completely unaffected by the fail-closed change.
#[tokio::test]
async fn a_geofence_set_to_none_is_untouched_by_the_fail_closed_change() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;

    let flagged = geofence_service::validate_geofence(&pool, company_id, Some(64.14), Some(-21.94))
        .await
        .expect("mode 'none' never rejects");
    assert!(!flagged, "mode 'none' must not flag anything");
}

/// The CLAUDE.md invariant: check-out never fails on geofence. Failing it here
/// would strand the employee in an open session only an admin could close —
/// which is exactly what fail-closed must not cause.
#[tokio::test]
async fn check_out_still_never_fails_on_an_unconfigured_geofence() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    set_geofence_mode_directly(&pool, company_id, "enforce").await;

    let flagged =
        geofence_service::flag_geofence_for_checkout(&pool, company_id, Some(64.14), Some(-21.94))
            .await
            .expect("check-out must never be refused on geofence");
    assert!(flagged, "it is flagged for admin review instead");
}

/// The write-side guards are what keep the fail-closed rejection unreachable in
/// normal operation. Note the asymmetry: getting back *out* (mode → 'none') is
/// never blocked.
#[tokio::test]
async fn the_last_active_location_cannot_be_removed_while_the_geofence_is_armed() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let actor = seed_user(&pool, company_id, "admin").await;
    let location_id = add_location(&pool, company_id, actor).await;
    set_geofence_mode_directly(&pool, company_id, "enforce").await;

    let err = geofence_service::delete_location(&pool, company_id, location_id, actor, None)
        .await
        .expect_err("removing the last location while enforcing must conflict");
    assert!(
        matches!(&err, AppError::Conflict(msg) if msg.contains("geofence mode to 'none'")),
        "409 must name the fix: {err:?}"
    );

    geofence_service::set_geofence_mode(&pool, company_id, "none", actor, None)
        .await
        .expect("switching the geofence off is never blocked");
    geofence_service::delete_location(&pool, company_id, location_id, actor, None)
        .await
        .expect("with the geofence off the same delete succeeds");
}

#[tokio::test]
async fn deactivating_the_last_location_conflicts_but_renaming_an_inactive_one_does_not() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let actor = seed_user(&pool, company_id, "admin").await;
    let location_id = add_location(&pool, company_id, actor).await;
    let spare = add_location(&pool, company_id, actor).await;
    set_geofence_mode_directly(&pool, company_id, "enforce").await;

    let off = UpdateLocationRequest {
        name: None,
        latitude: None,
        longitude: None,
        radius_meters: None,
        is_active: Some(false),
    };

    // Two active locations: deactivating one is fine.
    geofence_service::update_location(&pool, company_id, spare, &off, actor, None)
        .await
        .expect("one of two may be deactivated");

    // Now it is the last one.
    let err = geofence_service::update_location(&pool, company_id, location_id, &off, actor, None)
        .await
        .expect_err("deactivating the last active location must conflict");
    assert!(matches!(err, AppError::Conflict(_)), "got {err:?}");

    // Renaming the already-inactive row is not a deactivation and must pass.
    let rename = UpdateLocationRequest {
        name: Some("Old Warehouse".into()),
        latitude: None,
        longitude: None,
        radius_meters: None,
        is_active: Some(false),
    };
    let row = geofence_service::update_location(&pool, company_id, spare, &rename, actor, None)
        .await
        .expect("renaming an inactive location must not trip the guard");
    assert_eq!(row.name, "Old Warehouse");
}

/// This is the entry that used to let an admin arm enforcement against an empty
/// list in a single click.
#[tokio::test]
async fn the_geofence_cannot_be_armed_without_an_active_location() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let actor = seed_user(&pool, company_id, "admin").await;

    for mode in ["warn", "enforce"] {
        let err = geofence_service::set_geofence_mode(&pool, company_id, mode, actor, None)
            .await
            .expect_err("arming an empty fence must conflict");
        assert!(
            matches!(&err, AppError::Conflict(msg) if msg.contains(mode)),
            "409 must name the mode: {err:?}"
        );
    }

    add_location(&pool, company_id, actor).await;
    geofence_service::set_geofence_mode(&pool, company_id, "enforce", actor, None)
        .await
        .expect("with a location present, arming succeeds");
}

// ─── CSV export ───

/// Insert one attendance row at a wall-clock local time in `tz`.
async fn insert_record_at_local(
    pool: &sqlx::PgPool,
    company_id: uuid::Uuid,
    employee_id: uuid::Uuid,
    local: &str,
    tz: &str,
) {
    sqlx::query(
        r#"INSERT INTO attendance_records
           (company_id, employee_id, check_in_at, check_out_at, method, status)
           VALUES ($1, $2, ($3::text)::timestamp AT TIME ZONE $4,
                   (($3::text)::timestamp + INTERVAL '30 minutes') AT TIME ZONE $4,
                   'manual', 'present')"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .bind(local)
    .bind(tz)
    .execute(pool)
    .await
    .expect("insert attendance record at a local time");
}

fn export_range(from: &str, to: &str) -> AttendanceExportQuery {
    AttendanceExportQuery {
        date_from: from.parse().ok(),
        date_to: to.parse().ok(),
        employee_id: None,
        status: None,
        method: None,
    }
}

/// R1-M16 end to end: the SQL bounds were already threaded with the tenant's
/// zone while the rendering used a hardcoded UTC+8, so a Jakarta tenant got rows
/// selected on their calendar but printed on Malaysia's — some of them dated
/// outside the range they asked for.
#[tokio::test]
async fn the_export_renders_dates_on_the_tenant_own_calendar() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;
    seed_default_schedule(&pool, company_id, "Asia/Jakarta").await;

    // 23:30 on the 31st in Jakarta is 00:30 on the 1st in Kuala Lumpur.
    let at = "2026-07-31 23:30";
    insert_record_at_local(&pool, company_id, employee_id, at, "Asia/Jakarta").await;

    let range = export_range("2026-07-31", "2026-07-31");
    let csv = attendance_service::export_attendance_csv(&pool, company_id, &range)
        .await
        .expect("export should succeed");

    assert!(
        csv.contains("2026-07-31,") && csv.contains("23:30:00"),
        "the row must print on the day the filter selected it for:\n{csv}"
    );
    assert!(
        !csv.contains("2026-08-01"),
        "no exported row may fall outside the requested range:\n{csv}"
    );
}

/// R1-M17(a): one supplied bound used to skip the month default entirely, and
/// the reads layer then emitted a single open-ended bound — the tenant's whole
/// history, scanned and buffered.
#[tokio::test]
async fn each_export_date_bound_defaults_independently() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;

    // Anchored on the database clock: the lone-`date_from` case runs to *today*,
    // so fixed calendar dates would drift out of the span cap over time.
    let (today, _) = local_now(&pool, KL).await;
    let recent = today - Duration::days(5);
    let mid = today - Duration::days(40);
    let old = today - Duration::days(200);
    for day in [recent, mid, old] {
        let at = format!("{day} 09:00");
        insert_record_at_local(&pool, company_id, employee_id, &at, KL).await;
    }

    let only_to = AttendanceExportQuery {
        date_from: None,
        date_to: Some(recent),
        employee_id: None,
        status: None,
        method: None,
    };
    let csv = attendance_service::export_attendance_csv(&pool, company_id, &only_to)
        .await
        .expect("a lone date_to must resolve to the month it names");
    assert!(
        csv.contains(&recent.to_string()),
        "the row on the requested day is in range:\n{csv}"
    );
    assert!(
        !csv.contains(&mid.to_string()),
        "40 days back is a previous month, before the resolved start:\n{csv}"
    );

    let only_from = AttendanceExportQuery {
        date_from: Some(mid),
        date_to: None,
        employee_id: None,
        status: None,
        method: None,
    };
    let csv = attendance_service::export_attendance_csv(&pool, company_id, &only_from)
        .await
        .expect("a lone date_from must run to today");
    assert!(
        csv.contains(&mid.to_string()),
        "the start day itself is included:\n{csv}"
    );
    assert!(
        csv.contains(&recent.to_string()),
        "the range runs forward to today:\n{csv}"
    );
    assert!(
        !csv.contains(&old.to_string()),
        "the supplied start bound is still honoured:\n{csv}"
    );
}

#[tokio::test]
async fn the_export_rejects_an_inverted_or_over_long_range() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;

    let inverted = export_range("2026-05-01", "2026-04-01");
    let err = attendance_service::export_attendance_csv(&pool, company_id, &inverted)
        .await
        .expect_err("an inverted range must be a 400, not an empty 200");
    assert!(matches!(err, AppError::BadRequest(_)), "got {err:?}");

    let cap = attendance_service::MAX_EXPORT_RANGE_DAYS;
    let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();

    let at_cap = AttendanceExportQuery {
        date_from: Some(start),
        date_to: Some(start + Duration::days(cap)),
        employee_id: None,
        status: None,
        method: None,
    };
    attendance_service::export_attendance_csv(&pool, company_id, &at_cap)
        .await
        .expect("exactly the cap must be allowed — a statutory year is a real need");

    let past_cap = AttendanceExportQuery {
        date_to: Some(start + Duration::days(cap + 1)),
        ..at_cap
    };
    let err = attendance_service::export_attendance_csv(&pool, company_id, &past_cap)
        .await
        .expect_err("one day past the cap must be refused");
    assert!(
        matches!(&err, AppError::BadRequest(msg) if msg.contains(&cap.to_string())),
        "the message must name the cap: {err:?}"
    );
}

/// R1-M17(b): the row ceiling. Failing is deliberate — a silently truncated CSV
/// is indistinguishable from a complete one, and an admin cannot tell.
#[tokio::test]
async fn the_export_fails_loudly_rather_than_truncating() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;

    for local in ["2026-02-02 09:00", "2026-02-03 09:00", "2026-02-04 09:00"] {
        insert_record_at_local(&pool, company_id, employee_id, local, KL).await;
    }
    let range = export_range("2026-02-01", "2026-02-28");

    let err = attendance_service::export_attendance_csv_bounded(&pool, company_id, &range, 2)
        .await
        .expect_err("three rows against a ceiling of two must fail");
    assert!(
        matches!(&err, AppError::PayloadTooLarge(msg) if msg.contains("Narrow the date range")),
        "413 must tell the admin what to do: {err:?}"
    );

    let csv = attendance_service::export_attendance_csv_bounded(&pool, company_id, &range, 5)
        .await
        .expect("under the ceiling the export completes");
    assert_eq!(csv.lines().count(), 4, "header plus three rows:\n{csv}");
}

/// `check_in_at` alone is not unique, so without the `, ar.id` tiebreak two
/// identical exports could order (and, at the ceiling, drop) different rows.
#[tokio::test]
async fn identical_exports_are_byte_identical() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let (company_id, employee_id) = seed_company_and_employee(&pool).await;
    let second = seed_employee(&pool, company_id, None, 500_000).await;
    let third = seed_employee(&pool, company_id, None, 500_000).await;

    // Same instant for all three — the case an unstable sort reorders.
    for emp in [employee_id, second, third] {
        insert_record_at_local(&pool, company_id, emp, "2026-01-15 09:00", KL).await;
    }

    let range = export_range("2026-01-01", "2026-01-31");
    let first_run = attendance_service::export_attendance_csv(&pool, company_id, &range)
        .await
        .expect("export");
    let second_run = attendance_service::export_attendance_csv(&pool, company_id, &range)
        .await
        .expect("export");
    assert_eq!(first_run, second_run, "the row order must be deterministic");
}
