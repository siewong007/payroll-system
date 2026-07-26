use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::core::error::AppError;
use crate::models::attendance::{
    AttendanceSummaryQuery, ManualAttendanceRequest, UpdateAttendanceRecordRequest,
};
use crate::services::attendance_service;
use crate::tests::support::{seed_company_and_employee, seed_employee, seed_user, skip_if_no_db};

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
    let record = attendance_service::check_in_face_id(&pool, employee_id, company_id, None, None, None)
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

    let record = attendance_service::check_out(&pool, employee_id, company_id, None, None)
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

    let record = attendance_service::check_out(&pool, employee_id, company_id, None, None)
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

    let record = attendance_service::check_out(&pool, employee_id, company_id, None, None)
        .await
        .expect("check_out should succeed");

    assert!(
        record.overtime_hours.is_none(),
        "a 1 h ceiling must leave 2 h unrated: {:?}",
        record.overtime_hours
    );
}

/// The HR correction path is the designated remedy for an unrated record, so it
/// is deliberately uncapped — capping it would leave an operator with no way to
/// record a genuine long shift.
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
