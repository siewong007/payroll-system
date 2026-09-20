//! Data access for the `background_jobs` table — the durable record behind
//! work that runs detached from its submitting request.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::background_job::BackgroundJob;

/// Enqueue a job. The `background_jobs_one_active_payroll_run` partial unique
/// index makes a duplicate payroll-run submission fail here as a constraint —
/// the service maps it to Conflict.
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    job_type: &str,
    payload: serde_json::Value,
    created_by: Uuid,
) -> AppResult<BackgroundJob> {
    let row = sqlx::query_as!(
        BackgroundJob,
        r#"INSERT INTO background_jobs (id, company_id, job_type, payload, created_by)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, company_id, job_type, status, payload, progress_done,
                  progress_total, result, error, created_by, created_at,
                  started_at, finished_at"#,
        id,
        company_id,
        job_type,
        payload,
        created_by,
    )
    .fetch_one(executor)
    .await?;
    Ok(row)
}

/// Read one job, scoped to the caller's company. Absence maps to NotFound in
/// the service — never a bare row leak across tenants.
pub async fn get(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    id: Uuid,
) -> AppResult<Option<BackgroundJob>> {
    let row = sqlx::query_as!(
        BackgroundJob,
        r#"SELECT id, company_id, job_type, status, payload, progress_done,
                  progress_total, result, error, created_by, created_at,
                  started_at, finished_at
        FROM background_jobs
        WHERE id = $1 AND company_id = $2"#,
        id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

/// Flip `pending` → `running`. Returns false when another executor already
/// claimed the row, so a duplicated spawn cannot run the same job twice.
pub async fn mark_running(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<bool> {
    let done = sqlx::query!(
        r#"UPDATE background_jobs
        SET status = 'running', started_at = now()
        WHERE id = $1 AND status = 'pending'"#,
        id,
    )
    .execute(executor)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Progress heartbeat from the executor. Updates are unconditional — the row
/// is already `running`, and a lost update only means a slightly stale poll.
pub async fn set_progress(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    done: i32,
    total: i32,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE background_jobs
        SET progress_done = $2, progress_total = $3
        WHERE id = $1"#,
        id,
        done,
        total,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Terminal success write: status, result payload and timestamp in one
/// statement so a polled read can never see a result on a `running` row.
pub async fn succeed(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    result: serde_json::Value,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE background_jobs
        SET status = 'succeeded', result = $2, finished_at = now()
        WHERE id = $1"#,
        id,
        result,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Terminal failure write. The message is the client-facing `AppError` text,
/// stored verbatim — it is already sanitized for the caller.
pub async fn fail(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    error: &str,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE background_jobs
        SET status = 'failed', error = $2, finished_at = now()
        WHERE id = $1"#,
        id,
        error,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Active job count for a company — the submit-time throttle that stops a
/// rate-limit burst from stacking unbounded detached tasks on one tenant.
pub async fn count_active_for_company(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM background_jobs
        WHERE company_id = $1 AND status IN ('pending', 'running')"#,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}

/// Startup recovery: every job left `pending`/`running` died with the last
/// process — the executor is in-memory, so nothing will ever finish them.
/// Marking them failed (rather than re-queuing) keeps submission semantics:
/// a payroll run that was mid-transaction rolled back cleanly, and re-running
/// is a deliberate operator act, not an automatic retry.
pub async fn fail_stale(executor: impl Executor<'_, Database = Postgres>) -> AppResult<u64> {
    let done = sqlx::query!(
        r#"UPDATE background_jobs
        SET status = 'failed', error = 'The server restarted before this job finished.', finished_at = now()
        WHERE status IN ('pending', 'running')"#,
    )
    .execute(executor)
    .await?;
    Ok(done.rows_affected())
}
