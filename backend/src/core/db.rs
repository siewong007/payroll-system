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

    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .expect("Failed to run database migrations");
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
