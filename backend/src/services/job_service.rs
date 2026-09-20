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
        let prev = self.last_written.swap(done, Ordering::Relaxed);
        if done >= total || done - prev >= step {
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
async fn execute<Fut>(pool: PgPool, job_id: Uuid, work: Fut)
where
    Fut: std::future::Future<Output = AppResult<serde_json::Value>>,
{
    match background_jobs::mark_running(&pool, job_id).await {
        Ok(true) => {}
        Ok(false) => {
            warn!(%job_id, "job already claimed; dropping duplicate executor");
            return;
        }
        Err(e) => {
            // The row stays `pending` and the boot-time recovery will mark it
            // failed — noisy but never silently lost.
            error!(%job_id, error = %e, "could not mark background job running");
            return;
        }
    }

    match work.await {
        Ok(result) => {
            if let Err(e) = background_jobs::succeed(&pool, job_id, result).await {
                error!(%job_id, error = %e, "job finished but its result was not recorded");
            }
        }
        Err(e) => {
            // `client_response` is the sanitized text — the same string the
            // synchronous endpoint would have returned, so the polled error
            // carries no internals a raw `to_string` might.
            let (_, message) = e.client_response();
            warn!(%job_id, error = %e, "background job failed");
            if let Err(write_err) = background_jobs::fail(&pool, job_id, &message).await {
                error!(%job_id, error = %write_err, "job failure was not recorded");
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
