//! Restore-fidelity regression tests for whole-company backup/restore.
//!
//! Three defects live here, and all three are silent in production: a restore
//! that reports success and leaves the tenant wrong.
//!
//! * The archive never captured the attendance configuration, so a restored
//!   Jakarta tenant came back on `provision_company_defaults`' Malaysian
//!   09:00-18:00 with geofencing off — which moves every attendance day
//!   boundary, every lateness decision and the 12:30-local auto-absent window.
//! * Every restored row was stamped with the restore instant, so each
//!   `ORDER BY created_at … LIMIT n` list returned an arbitrary slice of a fully
//!   tied set, and every claim was submitted years before it was created.
//! * An overwrite hard-deletes `employees`, but the composite foreign key only
//!   NULLs `users.employee_id` — so anyone hired after the backup was taken kept
//!   a working login with no employee record behind it.

use chrono::{DateTime, NaiveTime, TimeZone, Utc};
use rust_decimal_macros::dec;
use sqlx::PgPool;
use uuid::Uuid;

use crate::repositories::reads::approvals;
use crate::repositories::{companies, company_locations, company_work_schedules};
use crate::services::backup_service::{export_company, import_company};
use crate::tests::support::{seed_company, seed_employee, seed_user, skip_if_no_db};

/// Instants the timestamp tests back-date rows to: distinct, and far enough in
/// the past that they cannot be confused with the restore instant.
fn archived_created_at() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2021, 3, 4, 5, 6, 7).unwrap()
}

fn archived_updated_at() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2022, 7, 8, 9, 10, 11).unwrap()
}

fn archived_submitted_at() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2021, 1, 2, 3, 4, 5).unwrap()
}

fn unique_email(prefix: &str) -> String {
    format!("{prefix}-{}@example.invalid", Uuid::new_v4())
}

fn restored_name() -> String {
    format!("Restored-{}", Uuid::new_v4())
}

async fn seed_schedule(pool: &PgPool, company_id: Uuid, name: &str, zone: &str) {
    let start = NaiveTime::from_hms_opt(8, 0, 0).unwrap();
    let end = NaiveTime::from_hms_opt(17, 0, 0).unwrap();
    company_work_schedules::upsert_default(pool, company_id, name, start, end, 5, 6.0, zone)
        .await
        .expect("seed the default work schedule");
}

async fn seed_location(pool: &PgPool, company_id: Uuid, name: &str) {
    company_locations::insert(pool, company_id, name, -6.2088, 106.8456, 150)
        .await
        .expect("seed a geofence location");
}

async fn arm_geofence(pool: &PgPool, company_id: Uuid) {
    companies::set_geofence_mode(pool, company_id, "enforce")
        .await
        .expect("arm the geofence");
}

async fn geofence_mode(pool: &PgPool, company_id: Uuid) -> Option<String> {
    companies::get_geofence_mode(pool, company_id)
        .await
        .expect("read the geofence mode")
}

async fn attendance_method(pool: &PgPool, company_id: Uuid) -> Option<String> {
    companies::get_attendance_method(pool, company_id)
        .await
        .expect("read the attendance method")
}

/// Insert a login row directly. The importer's own provisioning path is covered
/// elsewhere; these tests need accounts in states it would never create.
async fn seed_login(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Option<Uuid>,
    email: &str,
    role: &str,
) -> Uuid {
    let user_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO users (id, email, password_hash, full_name, roles, company_id, employee_id)
           VALUES ($1, $2, 'x', 'Test Login', ARRAY[$3]::VARCHAR(50)[], $4, $5)"#,
    )
    .bind(user_id)
    .bind(email)
    .bind(role)
    .bind(company_id)
    .bind(employee_id)
    .execute(pool)
    .await
    .expect("insert a login");
    user_id
}

struct LoginState {
    is_active: bool,
    employee_id: Option<Uuid>,
    is_deleted: bool,
}

async fn login_state(pool: &PgPool, user_id: Uuid) -> LoginState {
    let sql = "SELECT is_active, employee_id, deleted_at IS NOT NULL FROM users WHERE id = $1";
    let row: (bool, Option<Uuid>, bool) = sqlx::query_as(sql)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("read the login state");
    LoginState {
        is_active: row.0,
        employee_id: row.1,
        is_deleted: row.2,
    }
}

/// Run a one-parameter scalar query returning a timestamp.
async fn timestamp_of(pool: &PgPool, sql: &str, company_id: Uuid) -> DateTime<Utc> {
    sqlx::query_scalar(sql)
        .bind(company_id)
        .fetch_one(pool)
        .await
        .expect("read a restored timestamp")
}

/// Run a one-parameter scalar query returning a count.
async fn count_of(pool: &PgPool, sql: &str, id: Uuid) -> i64 {
    sqlx::query_scalar(sql)
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("read a count")
}

async fn set_employee_email(pool: &PgPool, employee_id: Uuid, email: &str) {
    sqlx::query("UPDATE employees SET email = $2 WHERE id = $1")
        .bind(employee_id)
        .bind(email)
        .execute(pool)
        .await
        .expect("set the employee email");
}

// ─── R2-M11: the attendance configuration a backup could not carry ───

/// The defect in one assertion. Before the fix this company came back on
/// 09:00-18:00, 15-minute grace, 4.0 half-day hours and Asia/Kuala_Lumpur with
/// geofencing off, whatever the source was configured for.
#[tokio::test]
async fn a_restored_tenant_keeps_its_timezone_schedule_and_geofence() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let source = seed_company(&pool).await;
    let actor = seed_user(&pool, source, "admin").await;

    seed_schedule(&pool, source, "Jakarta HQ", "Asia/Jakarta").await;
    seed_location(&pool, source, "Head office").await;
    arm_geofence(&pool, source).await;
    companies::set_attendance_method(&pool, source, Some("face_id"))
        .await
        .expect("set the source attendance method");

    let mut backup = export_company(&pool, source).await.expect("export");
    backup.company.name = restored_name();

    let result = import_company(&pool, backup, None, actor)
        .await
        .expect("restore the archive as a new company");
    let restored = result.new_company_id;

    let schedule = company_work_schedules::get_default(&pool, restored)
        .await
        .expect("read the restored schedule")
        .expect("the restored company has a default schedule");
    let start = NaiveTime::from_hms_opt(8, 0, 0).unwrap();
    let end = NaiveTime::from_hms_opt(17, 0, 0).unwrap();
    assert_eq!(schedule.name, "Jakarta HQ");
    assert_eq!(schedule.start_time, start);
    assert_eq!(schedule.end_time, end);
    assert_eq!(schedule.grace_minutes, 5);
    assert_eq!(schedule.half_day_hours, dec!(6.0));
    assert_eq!(
        schedule.timezone, "Asia/Jakarta",
        "a restore that silently moves the tenant to MYT re-buckets every day"
    );

    let mode = geofence_mode(&pool, restored).await;
    assert_eq!(mode.as_deref(), Some("enforce"));
    let method = attendance_method(&pool, restored).await;
    assert_eq!(method.as_deref(), Some("face_id"));

    let locations = company_locations::list_for_company(&pool, restored)
        .await
        .expect("read the restored geofence locations");
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].name, "Head office");
    assert_eq!(locations[0].radius_meters, 150);
    assert!((locations[0].latitude + 6.2088).abs() < 1e-9);
    assert!((locations[0].longitude - 106.8456).abs() < 1e-9);

    let counts = &result.records_imported;
    assert_eq!(counts.get("company_work_schedules").copied(), Some(1));
    assert_eq!(counts.get("company_locations").copied(), Some(1));
}

/// The regression guard for the naive "always delete then insert" fix. A 1.0
/// archive cannot speak to the attendance configuration, so restoring one over a
/// live tenant must leave that tenant's own settings standing — otherwise the
/// fix above becomes a new way to lose the same data.
#[tokio::test]
async fn a_legacy_archive_does_not_overwrite_the_targets_schedule() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let source = seed_company(&pool).await;
    let actor = seed_user(&pool, source, "admin").await;
    let target = seed_company(&pool).await;

    seed_schedule(&pool, target, "Target HQ", "Asia/Jakarta").await;
    seed_location(&pool, target, "Target site").await;
    arm_geofence(&pool, target).await;

    let mut backup = export_company(&pool, source).await.expect("export");
    // Everything a format-1.0 archive is missing, and nothing else.
    backup.metadata.format_version = "1.0".into();
    backup.company_work_schedules.clear();
    backup.company_locations.clear();
    backup.company.timezone = None;
    backup.company.geofence_mode = None;
    backup.company.attendance_method = None;

    let result = import_company(&pool, backup, Some(target), actor)
        .await
        .expect("a 1.0 archive must still restore");

    let schedule = company_work_schedules::get_default(&pool, target)
        .await
        .expect("read the target schedule")
        .expect("the target keeps its default schedule");
    assert_eq!(schedule.name, "Target HQ");
    assert_eq!(schedule.timezone, "Asia/Jakarta");
    let mode = geofence_mode(&pool, target).await;
    assert_eq!(mode.as_deref(), Some("enforce"));

    let locations = company_locations::list_for_company(&pool, target)
        .await
        .expect("read the target locations");
    assert_eq!(locations.len(), 1, "the target keeps its geofence anchor");

    let named = result.warnings.iter().any(|w| w.contains("work schedule"));
    assert!(named, "the gap must be reported: {:?}", result.warnings);
}

/// A hand-edited archive — or an honest one naming a zone tzdata has since
/// retired — used to abort the restore with a bare 500 from migration 1015's
/// trigger and the two CHECK constraints. It has to fall back, and it has to
/// name what it rejected.
#[tokio::test]
async fn an_unusable_timezone_or_geofence_mode_warns_instead_of_failing() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let source = seed_company(&pool).await;
    let actor = seed_user(&pool, source, "admin").await;
    seed_schedule(&pool, source, "Bad zone", "Asia/Jakarta").await;

    let mut backup = export_company(&pool, source).await.expect("export");
    backup.company.name = restored_name();
    backup.company.timezone = Some("Mars/Olympus".into());
    backup.company.geofence_mode = Some("lockdown".into());
    backup.company.attendance_method = Some("telepathy".into());
    backup.company_work_schedules[0].timezone = "Mars/Olympus".into();

    let result = import_company(&pool, backup, None, actor)
        .await
        .expect("an unusable value must warn, not abort the restore");
    let restored = result.new_company_id;

    let sql = "SELECT timezone FROM companies WHERE id = $1";
    let stored: String = sqlx::query_scalar(sql)
        .bind(restored)
        .fetch_one(&pool)
        .await
        .expect("read the restored company timezone");
    assert_eq!(stored, "Asia/Kuala_Lumpur");
    let mode = geofence_mode(&pool, restored).await;
    assert_eq!(mode.as_deref(), Some("none"));
    assert_eq!(attendance_method(&pool, restored).await, None);

    let schedule = company_work_schedules::get_default(&pool, restored)
        .await
        .expect("read the restored schedule")
        .expect("the restored company has a default schedule");
    assert_eq!(schedule.timezone, "Asia/Kuala_Lumpur");

    for rejected in ["Mars/Olympus", "lockdown", "telepathy"] {
        let named = result.warnings.iter().any(|w| w.contains(rejected));
        assert!(named, "no warning named {rejected}: {:?}", result.warnings);
    }
}

/// Geofencing armed with nothing to measure against blocks nobody. That looks
/// like protection and is not, so the restore says so.
#[tokio::test]
async fn geofencing_restored_with_no_locations_warns() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let source = seed_company(&pool).await;
    let actor = seed_user(&pool, source, "admin").await;
    arm_geofence(&pool, source).await;

    let mut backup = export_company(&pool, source).await.expect("export");
    backup.company.name = restored_name();

    let result = import_company(&pool, backup, None, actor)
        .await
        .expect("restore the archive as a new company");

    let warned = result
        .warnings
        .iter()
        .any(|w| w.contains("nothing will be enforced"));
    assert!(warned, "an armed but empty geofence must be reported");
}

// ─── R2-M10: logins the overwrite restore orphaned ───

/// The overwrite hard-deletes `employees`; `users_employee_tenant_fkey` nulls
/// `users.employee_id` and leaves `is_active` alone. Before the fix an employee
/// hired after the backup was taken kept a working login — and a live session
/// that would refresh forever — with no employee record behind it.
#[tokio::test]
async fn an_overwrite_restore_deactivates_a_login_the_backup_does_not_contain() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company = seed_company(&pool).await;
    let actor = seed_user(&pool, company, "admin").await;

    let kept = seed_employee(&pool, company, None, 500_000).await;
    let kept_email = unique_email("kept");
    set_employee_email(&pool, kept, &kept_email).await;
    let kept_login = seed_login(&pool, company, Some(kept), &kept_email, "employee").await;

    let backup = export_company(&pool, company).await.expect("export");

    // Hired after the backup was taken, so the archive knows nothing of them.
    let later = seed_employee(&pool, company, None, 400_000).await;
    let later_email = unique_email("later");
    let orphan = seed_login(&pool, company, Some(later), &later_email, "employee").await;
    let session_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO user_sessions (id, user_id, expires_at)
           VALUES ($1, $2, NOW() + INTERVAL '7 days')"#,
    )
    .bind(session_id)
    .bind(orphan)
    .execute(&pool)
    .await
    .expect("insert a live session");
    sqlx::query(
        r#"INSERT INTO refresh_tokens (user_id, token_hash, expires_at, session_id)
           VALUES ($1, $2, NOW() + INTERVAL '7 days', $3)"#,
    )
    .bind(orphan)
    .bind(Uuid::new_v4().to_string())
    .bind(session_id)
    .execute(&pool)
    .await
    .expect("insert a live refresh token");

    let result = import_company(&pool, backup, Some(company), actor)
        .await
        .expect("overwrite the company from its own backup");

    let state = login_state(&pool, orphan).await;
    assert!(!state.is_active, "an orphaned login must not still sign in");
    assert_eq!(state.employee_id, None);
    assert!(
        !state.is_deleted,
        "deactivation, not a tombstone: a tombstone would block a later restore"
    );

    let sessions = "SELECT count(*) FROM user_sessions WHERE user_id = $1 AND revoked_at IS NULL";
    assert_eq!(count_of(&pool, sessions, orphan).await, 0);
    let tokens = "SELECT count(*) FROM refresh_tokens WHERE user_id = $1 AND revoked = FALSE";
    assert_eq!(count_of(&pool, tokens, orphan).await, 0);

    let survivor = login_state(&pool, kept_login).await;
    assert!(survivor.is_active, "a re-linked login survives the sweep");
    assert!(
        survivor.employee_id.is_some(),
        "the archive's own employee was re-inserted, so their login relinks"
    );
    assert_ne!(
        survivor.employee_id,
        Some(kept),
        "the restore mints fresh ids, so the link points at the new row"
    );

    let counts = &result.records_imported;
    assert_eq!(counts.get("employee_logins_deactivated").copied(), Some(1));
    let reported = result
        .warnings
        .iter()
        .any(|w| w.contains("Deactivated 1 employee login"));
    assert!(
        reported,
        "the count must reach the admin: {:?}",
        result.warnings
    );
}

/// The bound on the sweep. It may only ever name accounts that belonged to this
/// tenant, held nothing but the employee role, and were linked to an employee
/// when the restore began. `update_user` can legitimately demote an
/// administrator to `roles = ['employee']` with no employee link at all, and a
/// blanket "deactivate every unlinked employee account" would lock them out.
#[tokio::test]
async fn a_privileged_login_and_an_already_unlinked_login_survive_the_sweep() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company = seed_company(&pool).await;
    let actor = seed_user(&pool, company, "admin").await;

    let managed = seed_employee(&pool, company, None, 500_000).await;
    let manager_email = unique_email("manager");
    let manager = seed_login(&pool, company, Some(managed), &manager_email, "hr_manager").await;

    let demoted_email = unique_email("demoted");
    let demoted = seed_login(&pool, company, None, &demoted_email, "employee").await;

    let backup = export_company(&pool, company).await.expect("export");
    let result = import_company(&pool, backup, Some(company), actor)
        .await
        .expect("overwrite the company from its own backup");

    assert!(
        login_state(&pool, manager).await.is_active,
        "an administrative account is never retired by employee lifecycle"
    );
    assert!(
        login_state(&pool, demoted).await.is_active,
        "an account already unlinked before the restore was never orphaned by it"
    );
    let counts = &result.records_imported;
    assert_eq!(counts.get("employee_logins_deactivated").copied(), Some(0));
}

// ─── R2-M12: the archive's own timestamps ───

/// Every restored row used to carry `Utc::now()`. That made each restored batch
/// one fully tied set for every `ORDER BY created_at` list, and left every claim
/// claiming to have been submitted years before it was created.
#[tokio::test]
async fn a_restore_preserves_the_archives_timestamps() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company = seed_company(&pool).await;
    let actor = seed_user(&pool, company, "admin").await;
    let employee = seed_employee(&pool, company, None, 500_000).await;
    let created_at = archived_created_at();
    let updated_at = archived_updated_at();

    sqlx::query("UPDATE employees SET created_at = $2, updated_at = $3 WHERE id = $1")
        .bind(employee)
        .bind(created_at)
        .bind(updated_at)
        .execute(&pool)
        .await
        .expect("back-date the employee");
    sqlx::query(
        r#"INSERT INTO claims
              (employee_id, company_id, title, amount, expense_date, status,
               submitted_at, created_at, updated_at)
           VALUES ($1, $2, 'Taxi', 1500, DATE '2021-01-01', 'pending', $3, $4, $5)"#,
    )
    .bind(employee)
    .bind(company)
    .bind(archived_submitted_at())
    .bind(created_at)
    .bind(updated_at)
    .execute(&pool)
    .await
    .expect("seed a back-dated claim");

    let team_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO teams (id, company_id, name, tag, created_at, updated_at)
           VALUES ($1, $2, 'Ops', 'general', $3, $4)"#,
    )
    .bind(team_id)
    .bind(company)
    .bind(created_at)
    .bind(updated_at)
    .execute(&pool)
    .await
    .expect("seed a back-dated team");
    sqlx::query(
        r#"INSERT INTO team_members (team_id, employee_id, role, joined_at)
           VALUES ($1, $2, 'member', $3)"#,
    )
    .bind(team_id)
    .bind(employee)
    .bind(created_at)
    .execute(&pool)
    .await
    .expect("seed a back-dated team member");

    let mut backup = export_company(&pool, company).await.expect("export");
    backup.company.name = restored_name();
    let result = import_company(&pool, backup, None, actor)
        .await
        .expect("restore the archive as a new company");
    let restored = result.new_company_id;

    let sql = "SELECT created_at FROM employees WHERE company_id = $1";
    assert_eq!(timestamp_of(&pool, sql, restored).await, created_at);
    let sql = "SELECT updated_at FROM employees WHERE company_id = $1";
    assert_eq!(timestamp_of(&pool, sql, restored).await, updated_at);

    let sql = "SELECT created_at FROM claims WHERE company_id = $1";
    let claim_created = timestamp_of(&pool, sql, restored).await;
    assert_eq!(claim_created, created_at);
    let sql = "SELECT updated_at FROM claims WHERE company_id = $1";
    assert_eq!(timestamp_of(&pool, sql, restored).await, updated_at);
    let sql = "SELECT submitted_at FROM claims WHERE company_id = $1";
    let submitted = timestamp_of(&pool, sql, restored).await;
    assert_eq!(submitted, archived_submitted_at());
    assert!(
        submitted <= claim_created,
        "a restored claim must not be submitted before it was created"
    );

    let sql = r#"SELECT tm.joined_at FROM team_members tm
                 JOIN teams t ON tm.team_id = t.id
                 WHERE t.company_id = $1"#;
    assert_eq!(timestamp_of(&pool, sql, restored).await, created_at);
}

/// The other half of the same defect. Restored rows share whatever instant they
/// were written with, and `now()` is transaction-start time, so ties are the
/// normal case rather than an edge case. Without a tiebreak on the primary key a
/// tied `ORDER BY … LIMIT 100` returns whichever hundred the planner reached
/// first, which is not required to be the same hundred twice.
#[tokio::test]
async fn tied_timestamps_still_order_deterministically() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company = seed_company(&pool).await;
    let employee = seed_employee(&pool, company, None, 500_000).await;

    let leave_type = Uuid::new_v4();
    sqlx::query("INSERT INTO leave_types (id, company_id, name) VALUES ($1, $2, $3)")
        .bind(leave_type)
        .bind(company)
        .bind(format!("Annual-{}", &leave_type.to_string()[..8]))
        .execute(&pool)
        .await
        .expect("seed a leave type");

    // 120 rows sharing one instant: more than the endpoint's LIMIT of 100, so
    // the tiebreak is what decides which twenty are dropped.
    sqlx::query(
        r#"INSERT INTO leave_requests
              (employee_id, company_id, leave_type_id, start_date, end_date, days,
               status, created_at, updated_at)
           SELECT $1, $2, $3, DATE '2024-01-01' + g, DATE '2024-01-01' + g, 1,
                  'pending', $4, $4
           FROM generate_series(0, 119) AS g"#,
    )
    .bind(employee)
    .bind(company)
    .bind(leave_type)
    .bind(archived_created_at())
    .execute(&pool)
    .await
    .expect("seed 120 tied leave requests");

    let first = approvals::list_pending_leave(&pool, company, None, 100, 0)
        .await
        .expect("first read of the approvals inbox");
    let second = approvals::list_pending_leave(&pool, company, None, 100, 0)
        .await
        .expect("second read of the approvals inbox");

    let first_ids: Vec<Uuid> = first.iter().map(|r| r.id).collect();
    let second_ids: Vec<Uuid> = second.iter().map(|r| r.id).collect();
    assert_eq!(first_ids.len(), 100);
    assert_eq!(first_ids, second_ids, "repeated reads must agree");

    let sql = "SELECT id FROM leave_requests WHERE company_id = $1 ORDER BY id DESC";
    let mut all_ids: Vec<Uuid> = sqlx::query_scalar(sql)
        .bind(company)
        .fetch_all(&pool)
        .await
        .expect("read every seeded leave request");
    all_ids.truncate(100);
    assert_eq!(
        first_ids, all_ids,
        "with created_at tied the inbox must fall back to the primary key"
    );
}

// ─── Archive compatibility ───

/// A 1.1 archive is what the exporter now writes; a 1.0 archive is what every
/// already-downloaded backup is. Both must restore, and nothing else may.
#[tokio::test]
async fn both_archive_format_versions_restore_and_nothing_else_does() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let source = seed_company(&pool).await;
    let actor = seed_user(&pool, source, "admin").await;

    let current = export_company(&pool, source).await.expect("export");
    assert_eq!(
        current.metadata.format_version, "1.1",
        "the exporter writes the version that carries the attendance config"
    );

    let mut legacy = export_company(&pool, source).await.expect("export");
    legacy.metadata.format_version = "1.0".into();
    legacy.company.name = restored_name();
    import_company(&pool, legacy, None, actor)
        .await
        .expect("a 1.0 archive must still restore");

    let mut unknown = export_company(&pool, source).await.expect("export");
    unknown.metadata.format_version = "2.0".into();
    unknown.company.name = restored_name();
    let error = import_company(&pool, unknown, None, actor)
        .await
        .expect_err("an unknown format version must be refused");
    assert!(
        format!("{error:?}").contains("2.0"),
        "the rejection must name the version it was handed: {error:?}"
    );
}
