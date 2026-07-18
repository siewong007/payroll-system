use sqlx::PgPool;
use std::env;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;
use uuid::Uuid;

static MIGRATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Try to connect to the test database and ensure migrations are applied.
///
/// Returns `None` when `DATABASE_URL` is unset or the pool cannot be built,
/// which lets tests skip cleanly in environments without Postgres (e.g.
/// running `cargo test` on a laptop with no docker-compose running).
///
/// The first caller runs `sqlx::migrate!` to bring the schema up to date;
/// subsequent callers reuse the already-migrated database. A mutex serialises
/// the first-migration path so parallel tests don't race on the
/// `_sqlx_migrations` advisory lock.
pub async fn test_pool() -> Option<PgPool> {
    let url = env::var("DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url).await.ok()?;

    let lock = MIGRATE_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().await;
    crate::core::db::run_migrations(&pool).await;

    // The repository deliberately ships only immutable, unverified academic
    // statutory fixtures. Test builds attach those rows to distinct test-only
    // verified datasets; production cannot promote a `legacy-prototype-*` key.
    sqlx::query(
        r#"
        UPDATE statutory_rule_sets
        SET status = 'prototype', verified_at = NULL, updated_at = NOW()
        WHERE dataset_key LIKE 'legacy-prototype-%'
        "#,
    )
    .execute(&pool)
    .await
    .expect("keep legacy statutory fixtures unverified");

    sqlx::query(
        r#"
        INSERT INTO statutory_rule_sets (
            dataset_key, rule_code, effective_from, effective_to, status,
            source_url, source_version, source_sha256,
            verification_notes, verified_at
        ) VALUES
            ('test-fixture-epf-2024', 'epf', '2024-01-01', '2024-12-31', 'verified',
             'https://example.invalid/test-statutory-fixture', 'test-only', repeat('0', 64),
             'Enabled only by the Rust test harness', NOW()),
            ('test-fixture-socso-2024', 'socso', '2024-01-01', '2024-12-31', 'verified',
             'https://example.invalid/test-statutory-fixture', 'test-only', repeat('0', 64),
             'Enabled only by the Rust test harness', NOW()),
            ('test-fixture-eis-2024', 'eis', '2024-01-01', '2024-12-31', 'verified',
             'https://example.invalid/test-statutory-fixture', 'test-only', repeat('0', 64),
             'Enabled only by the Rust test harness', NOW()),
            ('test-fixture-pcb-2024', 'pcb', '2024-01-01', '2024-12-31', 'verified',
             'https://example.invalid/test-statutory-fixture', 'test-only', repeat('0', 64),
             'Enabled only by the Rust test harness', NOW())
        ON CONFLICT (dataset_key) DO UPDATE SET
            status = EXCLUDED.status,
            source_url = EXCLUDED.source_url,
            source_version = EXCLUDED.source_version,
            source_sha256 = EXCLUDED.source_sha256,
            verification_notes = EXCLUDED.verification_notes,
            verified_at = EXCLUDED.verified_at,
            updated_at = NOW()
        "#,
    )
    .execute(&pool)
    .await
    .expect("create test-only statutory rule sets");

    for (table, dataset_key) in [
        ("epf_rates", "test-fixture-epf-2024"),
        ("socso_rates", "test-fixture-socso-2024"),
        ("eis_rates", "test-fixture-eis-2024"),
        ("pcb_brackets", "test-fixture-pcb-2024"),
        ("pcb_reliefs", "test-fixture-pcb-2024"),
    ] {
        let statement = format!(
            "UPDATE {table} SET rule_set_id = (SELECT id FROM statutory_rule_sets WHERE dataset_key = $1)"
        );
        sqlx::query(&statement)
            .bind(dataset_key)
            .execute(&pool)
            .await
            .expect("attach statutory rows to a test-only rule set");
    }

    Some(pool)
}

/// Skip the current test if no database is reachable.
///
/// Use as:
/// ```ignore
/// let Some(pool) = skip_if_no_db().await else { return };
/// ```
pub async fn skip_if_no_db() -> Option<PgPool> {
    match test_pool().await {
        Some(p) => Some(p),
        None => {
            eprintln!("SKIP: DATABASE_URL not set or PostgreSQL is unreachable");
            None
        }
    }
}

/// Monotonic per-process counter used to give each seeded row a unique
/// `employee_number` / company name suffix. Tests share a Postgres instance
/// and can run in any order, so suffixes combine a UUID with this counter to
/// avoid UNIQUE collisions without needing `TRUNCATE` between tests.
static SEED_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_seed_id() -> u64 {
    SEED_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// A fresh company + employee with sensible defaults for tests that only care
/// about the employee existing (e.g. attendance, payroll_engine). Returns
/// `(company_id, employee_id)`.
///
/// Rows persist across test runs — every call uses fresh UUIDs and a unique
/// `employee_number`, so tests can't collide on the `(company_id, employee_number)`
/// unique index.
pub async fn seed_company_and_employee(pool: &PgPool) -> (Uuid, Uuid) {
    let company_id = seed_company(pool).await;
    let employee_id = seed_employee(pool, company_id, None, 500_000).await;
    (company_id, employee_id)
}

pub async fn seed_company(pool: &PgPool) -> Uuid {
    let seq = next_seed_id();
    let company_id = Uuid::new_v4();
    let company_name = format!("TestCo-{}-{}", seq, &company_id.to_string()[..8]);
    sqlx::query("INSERT INTO companies (id, name) VALUES ($1, $2)")
        .bind(company_id)
        .bind(&company_name)
        .execute(pool)
        .await
        .expect("insert company");
    company_id
}

pub async fn seed_payroll_group(pool: &PgPool, company_id: Uuid) -> Uuid {
    let seq = next_seed_id();
    let group_id = Uuid::new_v4();
    sqlx::query("INSERT INTO payroll_groups (id, company_id, name) VALUES ($1, $2, $3)")
        .bind(group_id)
        .bind(company_id)
        .bind(format!("Test Group {seq}"))
        .execute(pool)
        .await
        .expect("insert payroll_group");
    group_id
}

/// Insert an employee with the given basic salary (in sen). `group_id` is
/// optional — set it when the test will run the payroll engine.
pub async fn seed_employee(
    pool: &PgPool,
    company_id: Uuid,
    group_id: Option<Uuid>,
    basic_salary_sen: i64,
) -> Uuid {
    let seq = next_seed_id();
    let employee_id = Uuid::new_v4();
    let employee_number = format!("E-{}-{}", seq, &employee_id.to_string()[..8]);
    sqlx::query(
        r#"INSERT INTO employees
           (id, company_id, employee_number, full_name, date_joined,
            basic_salary, payroll_group_id, date_of_birth, epf_category)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'A')"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(&employee_number)
    .bind("Test Employee")
    .bind(chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap())
    .bind(basic_salary_sen)
    .bind(group_id)
    .bind(chrono::NaiveDate::from_ymd_opt(1990, 1, 1).unwrap())
    .execute(pool)
    .await
    .expect("insert employee");
    employee_id
}

/// Insert a user (used as `processed_by` for payroll runs).
pub async fn seed_user(pool: &PgPool, company_id: Uuid, role: &str) -> Uuid {
    let seq = next_seed_id();
    let user_id = Uuid::new_v4();
    let email = format!("test-{seq}-{}@example.invalid", &user_id.to_string()[..8]);
    sqlx::query(
        r#"INSERT INTO users (id, email, password_hash, full_name, roles, company_id)
           VALUES ($1, $2, 'x', 'Test User', ARRAY[$3]::VARCHAR(50)[], $4)"#,
    )
    .bind(user_id)
    .bind(&email)
    .bind(role)
    .bind(company_id)
    .execute(pool)
    .await
    .expect("insert user");
    user_id
}
