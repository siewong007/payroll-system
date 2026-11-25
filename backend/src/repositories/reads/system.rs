use serde::Serialize;
use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;

#[derive(Debug, Serialize)]
pub struct OperationalQueueCounts {
    pub email_outbox_pending: i64,
    pub email_outbox_failed_24h: i64,
    pub push_outbox_pending: i64,
    pub push_outbox_failed_24h: i64,
    pub background_jobs_failed_24h: i64,
}

/// Check migration status for the health check.
pub async fn get_migration_status(
    executor: impl Executor<'_, Database = Postgres>,
) -> AppResult<(Option<i64>, i64)> {
    let row =
        sqlx::query_as("SELECT MAX(version), COUNT(*) FROM _sqlx_migrations WHERE success = TRUE")
            .fetch_one(executor)
            .await?;
    Ok(row)
}

/// Aggregate operational queue counts for `/api/admin/platform/health`.
pub async fn operational_queue_counts(
    executor: impl Executor<'_, Database = Postgres>,
) -> AppResult<OperationalQueueCounts> {
    let row = sqlx::query!(
        r#"SELECT
               (SELECT COUNT(*) FROM email_logs
                WHERE status = 'pending' AND next_retry_at IS NOT NULL) AS "email_outbox_pending!",
               (SELECT COUNT(*) FROM email_logs
                WHERE status = 'failed' AND created_at >= NOW() - INTERVAL '24 hours') AS "email_outbox_failed_24h!",
               (SELECT COUNT(*) FROM push_outbox
                WHERE status = 'pending') AS "push_outbox_pending!",
               (SELECT COUNT(*) FROM push_outbox
                WHERE status = 'failed' AND created_at >= NOW() - INTERVAL '24 hours') AS "push_outbox_failed_24h!",
               (SELECT COUNT(*) FROM background_jobs
                WHERE status = 'failed' AND created_at >= NOW() - INTERVAL '24 hours') AS "background_jobs_failed_24h!""#
    )
    .fetch_one(executor)
    .await?;
    Ok(OperationalQueueCounts {
        email_outbox_pending: row.email_outbox_pending,
        email_outbox_failed_24h: row.email_outbox_failed_24h,
        push_outbox_pending: row.push_outbox_pending,
        push_outbox_failed_24h: row.push_outbox_failed_24h,
        background_jobs_failed_24h: row.background_jobs_failed_24h,
    })
}
