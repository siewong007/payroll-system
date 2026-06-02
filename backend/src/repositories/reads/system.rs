use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;

/// Check migration status for the health check.
pub async fn get_migration_status(
    executor: impl Executor<'_, Database = Postgres>,
) -> AppResult<(Option<i64>, i64)> {
    let row = sqlx::query_as("SELECT MAX(version), COUNT(*) FROM _sqlx_migrations WHERE success = TRUE")
        .fetch_one(executor)
        .await?;
    Ok(row)
}
