//! Detached execution for work too long for a request.
//!
//! A submission writes a `background_jobs` row and returns it at once (202);
//! a spawned task then does the work and records status, progress and result
//! back on the row for the client to poll. The row — not the task — is the
//! source of truth, so a restart can fail every stale `pending`/`running` job
//! at boot instead of leaving it indeterminate.
//!
//! The executor being in-process is deliberate: the heavy units (payroll run,
//! employee import) are already one transaction each, so there is no queue
//! semantics to preserve across a crash. If execution ever moves out of this
//! binary, this module is the seam — `submit_*` becomes a gRPC call and the
//! progress writer below becomes the stream event.

use std::sync::atomic::{AtomicI32, Ordering};

use chrono::NaiveDate;
use sqlx::PgPool;
use tracing::{error, warn};
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::background_job::{BackgroundJob, job_type};
use crate::models::employee_import::ImportConfirmRequest;
use crate::models::payroll::ProcessPayrollRequest;
use crate::repositories::background_jobs;
use crate::services::audit_service::AuditRequestMeta;
use crate::services::{employee_import_service, payroll_engine};

/// In-flight jobs one company may stack. The route rate limiter caps
/// submissions per session; this caps them per tenant, because each running
/// job holds a pool connection (and during a payroll run, an open write
/// transaction) for its whole duration.
const MAX_ACTIVE_PER_COMPANY: i64 = 4;

/// Throttled `progress_done`/`progress_total` writer that a long-running
/// executor ticks through.
///
/// Writes go through the pool, deliberately NOT through the job's own
/// transaction: a rolled-back payroll run must not take its progress record
/// down with it, and a polled read has to see movement before commit.
/// Progress is best-effort — a missed update only means a stale poll.
pub struct JobProgress {
    pool: PgPool,
    job_id: Uuid,
    /// Update granularity: ~5% of the total once it is known.
    step: AtomicI32,
    total: AtomicI32,
    last_written: AtomicI32,
}

impl JobProgress {
    pub fn new(pool: PgPool, job_id: Uuid) -> Self {
        Self {
            pool,
            job_id,
            step: AtomicI32::new(1),
            total: AtomicI32::new(0),
            last_written: AtomicI32::new(0),
        }
    }

    /// Declare the denominator — or extend it, for multi-phase jobs like the
    /// import (insert phase, then provisioning phase).
    pub async fn set_total(&self, total: i32) {
        self.total.store(total, Ordering::Relaxed);
        self.step.store((total / 20).max(1), Ordering::Relaxed);
        let _ = background_jobs::set_progress(
            &self.pool,
            self.job_id,
            self.last_written.load(Ordering::Relaxed),
            total,
        )
        .await;
    }

    /// Record `done` units. Writes at most once per `step` (and always at the
    /// total), so progress costs ~20 UPDATEs over a whole job, not one per
    /// unit of work.
    pub async fn tick(&self, done: i32) {
        let total = self.total.load(Ordering::Relaxed);
        let step = self.step.load(Ordering::Relaxed).max(1);
        let prev = self.last_written.load(Ordering::Relaxed);
        if done >= total || done - prev >= step {
            self.last_written.store(done, Ordering::Relaxed);
            let _ = background_jobs::set_progress(&self.pool, self.job_id, done, total).await;
        }
    }
}

/// Enqueue + spawn. Shared by every submit path: insert is the dedup point
/// (the payroll partial unique index lands here), spawn is the executor.
async fn enqueue(
    pool: &PgPool,
    company_id: Uuid,
    job_type_str: &str,
    payload: serde_json::Value,
    created_by: Uuid,
) -> AppResult<BackgroundJob> {
    if background_jobs::count_active_for_company(pool, company_id).await? >= MAX_ACTIVE_PER_COMPANY
    {
        return Err(AppError::Conflict(
            "Too many background jobs are already running for this company. Wait for one to finish."
                .into(),
        ));
    }
    background_jobs::insert(
        pool,
        Uuid::now_v7(),
        company_id,
        job_type_str,
        payload,
        created_by,
    )
    .await
}

/// The executor lifecycle every job shares: claim (`pending` → `running`),
/// run, then one terminal write. A `mark_running` miss means another executor
/// took the row — the duplicate task drops its work rather than double-run.
///
/// The work future runs in its own `tokio::spawn` so any panic is caught at
/// the `JoinHandle` boundary and transitions the row to `failed` instead of
/// leaving it wedged in `running` until restart.
async fn execute<Fut>(pool: PgPool, job_id: Uuid, work: Fut)
where
    Fut: std::future::Future<Output = AppResult<serde_json::Value>> + Send + 'static,
{
    let mut marked = None;
    for attempt in 0..3 {
        match background_jobs::mark_running(&pool, job_id).await {
            Ok(v) => {
                marked = Some(Ok(v));
                break;
            }
            Err(e) => {
                marked = Some(Err(e));
                if attempt < 2 {
                    tokio::time::sleep(std::time::Duration::from_millis(50 * (attempt + 1))).await;
                }
            }
        }
    }

    match marked.expect("at least one attempt") {
        Ok(true) => {}
        Ok(false) => {
            warn!(%job_id, "job already claimed; dropping duplicate executor");
            return;
        }
        Err(e) => {
            error!(%job_id, error = %e, "could not mark background job running");
            let _ = background_jobs::fail(&pool, job_id, "Internal server error").await;
            return;
        }
    }

    match tokio::spawn(work).await {
        Ok(Ok(result)) => {
            let mut last_err = None;
            for attempt in 0..3 {
                match background_jobs::succeed(&pool, job_id, result.clone()).await {
                    Ok(()) => {
                        last_err = None;
                        break;
                    }
                    Err(e) => {
                        last_err = Some(e);
                        if attempt < 2 {
                            tokio::time::sleep(std::time::Duration::from_millis(
                                50 * (attempt + 1),
                            ))
                            .await;
                        }
                    }
                }
            }
            if let Some(e) = last_err {
                error!(%job_id, error = %e, "job finished but its result was not recorded");
            }
        }
        Ok(Err(e)) => {
            // `client_response` is the sanitized text — the same string the
            // synchronous endpoint would have returned, so the polled error
            // carries no internals a raw `to_string` might.
            let (_, message) = e.client_response();
            warn!(%job_id, error = %e, "background job failed");
            for attempt in 0..3 {
                match background_jobs::fail(&pool, job_id, &message).await {
                    Ok(()) => break,
                    Err(write_err) => {
                        if attempt == 2 {
                            error!(%job_id, error = %write_err, "job failure was not recorded");
                        } else {
                            tokio::time::sleep(std::time::Duration::from_millis(
                                50 * (attempt + 1),
                            ))
                            .await;
                        }
                    }
                }
            }
        }
        Err(join_err) => {
            error!(%job_id, error = %join_err, "background job task panicked or was aborted");
            for attempt in 0..3 {
                match background_jobs::fail(&pool, job_id, "Internal server error").await {
                    Ok(()) => break,
                    Err(write_err) => {
                        if attempt == 2 {
                            error!(%job_id, error = %write_err, "job panic failure was not recorded");
                        } else {
                            tokio::time::sleep(std::time::Duration::from_millis(
                                50 * (attempt + 1),
                            ))
                            .await;
                        }
                    }
                }
            }
        }
    }
}

/// Submit a payroll run for detached processing.
///
/// The engine call itself is unchanged — still one transaction, still
/// all-or-nothing. What changed is only *when* the answer arrives: the job
/// row ends with `{run_id}` rather than the run riding back on the request.
pub async fn submit_payroll_run(
    pool: &PgPool,
    company_id: Uuid,
    user_id: Uuid,
    req: ProcessPayrollRequest,
    pay_date: NaiveDate,
    audit_meta: AuditRequestMeta,
) -> AppResult<BackgroundJob> {
    let ProcessPayrollRequest {
        payroll_group_id,
        period_year,
        period_month,
        notes,
        ..
    } = req;

    let payload = serde_json::json!({
        "payroll_group_id": payroll_group_id,
        "period_year": period_year,
        "period_month": period_month,
        "pay_date": pay_date,
        "notes": notes,
    });
    let job = enqueue(pool, company_id, job_type::PAYROLL_RUN, payload, user_id).await?;

    let job_id = job.id;
    let task_pool = pool.clone();
    tokio::spawn(async move {
        execute(task_pool.clone(), job_id, async move {
            let progress = JobProgress::new(task_pool.clone(), job_id);
            let run = payroll_engine::process_payroll_inner(
                &task_pool,
                company_id,
                payroll_group_id,
                period_year,
                period_month,
                pay_date,
                user_id,
                notes,
                Some(&audit_meta),
                Some(&progress),
            )
            .await?;
            Ok(serde_json::json!({ "run_id": run.id }))
        })
        .await;
    });

    Ok(job)
}

/// Submit a confirmed employee-import session for detached processing.
///
/// The confirm writes its row errors and provisioning counts into the job's
/// `result` — the same body the synchronous endpoint used to return.
pub async fn submit_employee_import(
    pool: &PgPool,
    mailer: &crate::services::email_service::SharedMailer,
    config: &crate::core::config::AppConfig,
    company_id: Uuid,
    user_id: Uuid,
    req: ImportConfirmRequest,
    audit_meta: AuditRequestMeta,
) -> AppResult<BackgroundJob> {
    let payload = serde_json::json!({
        "session_id": req.session_id,
        "skip_invalid": req.skip_invalid,
    });
    let job = enqueue(
        pool,
        company_id,
        job_type::EMPLOYEE_IMPORT,
        payload,
        user_id,
    )
    .await?;

    let job_id = job.id;
    let task_pool = pool.clone();
    let task_mailer = mailer.clone();
    let task_config = config.clone();
    tokio::spawn(async move {
        execute(task_pool.clone(), job_id, async move {
            let progress = JobProgress::new(task_pool.clone(), job_id);
            let response = employee_import_service::confirm_import_inner(
                &task_pool,
                company_id,
                user_id,
                req,
                Some(&audit_meta),
                Some(&progress),
                Some((&task_mailer, &task_config)),
            )
            .await?;
            serde_json::to_value(&response)
                .map_err(|e| AppError::Internal(format!("Failed to encode job result: {}", e)))
        })
        .await;
    });

    Ok(job)
}

/// Read a job for the status endpoint, scoped to the caller's company.
pub async fn get(pool: &PgPool, company_id: Uuid, job_id: Uuid) -> AppResult<BackgroundJob> {
    background_jobs::get(pool, company_id, job_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Job not found".into()))
}

/// Boot-time recovery: fail every job the last process left in-flight. Not
/// re-queued — a mid-transaction payroll run already rolled back, and a
/// re-run is a deliberate operator act, not an automatic retry.
pub async fn recover_stale(pool: &PgPool) {
    match background_jobs::fail_stale(pool).await {
        Ok(0) => {}
        Ok(n) => warn!(
            stale = n,
            "marked stale background jobs failed after restart"
        ),
        Err(e) => error!(error = %e, "stale background job recovery failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::support::{seed_company, seed_user, skip_if_no_db};

    #[tokio::test]
    async fn panicking_background_job_marks_row_failed() {
        let Some(pool) = skip_if_no_db().await else {
            return;
        };
        let company_id = seed_company(&pool).await;
        let user_id = seed_user(&pool, company_id, "payroll_admin").await;
        let job = enqueue(
            &pool,
            company_id,
            job_type::EMPLOYEE_IMPORT,
            serde_json::json!({"test": true}),
            user_id,
        )
        .await
        .expect("enqueue job");

        execute(pool.clone(), job.id, async move {
            panic!("simulated unexpected job panic");
        })
        .await;

        let stored = get(&pool, company_id, job.id)
            .await
            .expect("load job after panic");
        assert_eq!(stored.status, "failed");
        assert_eq!(stored.error.as_deref(), Some("Internal server error"));
        assert!(stored.finished_at.is_some());
    }
}
