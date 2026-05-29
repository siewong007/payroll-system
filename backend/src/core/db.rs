use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

pub async fn create_pool(database_url: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
        .expect("Failed to create database pool")
}

pub async fn run_migrations(pool: &PgPool) {
    close_legacy_absent_records(pool).await;

    // Schema migrations are tracked in `_sqlx_migrations`; seed data is applied
    // separately (idempotently) so it isn't re-injected on every boot.
    sqlx::migrate!("./migrations/schema")
        .run(pool)
        .await
        .expect("Failed to run database migrations");

    seed_if_needed(pool).await;
}

/// Inject the seed script once. Statutory rate tables (e.g. `epf_rates`) are
/// the first thing the seed populates, so their presence is a reliable marker
/// that the seed has already been loaded — if so we skip re-injecting it.
async fn seed_if_needed(pool: &PgPool) {
    let already_seeded: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM epf_rates)")
        .fetch_one(pool)
        .await
        .expect("Failed to check whether seed data is present");

    if already_seeded {
        tracing::debug!("Seed data already present; skipping seed injection");
        return;
    }

    tracing::info!("Seeding database with initial data");
    let seed_sql = include_str!("../../migrations/seed/001_seed.sql");
    sqlx::raw_sql(seed_sql)
        .execute(pool)
        .await
        .expect("Failed to inject seed data");
}

async fn close_legacy_absent_records(pool: &PgPool) {
    let attendance_table_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public.attendance_records') IS NOT NULL")
            .fetch_one(pool)
            .await
            .expect("Failed to check attendance_records table");

    if !attendance_table_exists {
        return;
    }

    let result = sqlx::query(
        r#"UPDATE attendance_records
           SET check_out_at = check_in_at,
               updated_at = NOW()
           WHERE status = 'absent'
             AND check_out_at IS NULL"#,
    )
    .execute(pool)
    .await
    .expect("Failed to close legacy absent attendance records");

    if result.rows_affected() > 0 {
        tracing::warn!(
            "Closed {} legacy absent attendance records before running migrations",
            result.rows_affected()
        );
    }
}
