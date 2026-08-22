use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::payroll::PayrollRun;
use crate::repositories::payroll_runs;
use crate::services::audit_service::AuditRequestMeta;

async fn load_run(pool: &PgPool, company_id: Uuid, id: Uuid) -> AppResult<PayrollRun> {
    payroll_runs::get_for_company(pool, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))
}

#[allow(clippy::too_many_arguments)]
async fn audit_transition(
    pool: &PgPool,
    company_id: Uuid,
    actor_user_id: Uuid,
    action: &str,
    old_run: &PayrollRun,
    run: &PayrollRun,
    new_values: serde_json::Value,
    description: String,
    audit_meta: Option<&AuditRequestMeta>,
) {
    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_user_id),
        action,
        "payroll_run",
        Some(run.id),
        Some(serde_json::to_value(old_run).unwrap_or_default()),
        Some(new_values),
        Some(&description),
        audit_meta,
    )
    .await;
}

pub async fn submit_for_approval(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    actor_user_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollRun> {
    let old_run = load_run(pool, company_id, run_id).await?;
    if old_run.status != "processed" {
        return Err(AppError::BadRequest(
            "Only processed payroll runs can be submitted for approval".into(),
        ));
    }

    let run = payroll_runs::set_pending_approval(pool, run_id, company_id, actor_user_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("Payroll run could not be submitted".into()))?;

    audit_transition(
        pool,
        company_id,
        actor_user_id,
        "submit_approval",
        &old_run,
        &run,
        serde_json::to_value(&run).unwrap_or_default(),
        format!(
            "Submitted payroll run for approval for {:02}/{}",
            run.period_month, run.period_year
        ),
        audit_meta,
    )
    .await;

    Ok(run)
}

pub async fn approve(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    actor_user_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollRun> {
    let old_run = load_run(pool, company_id, run_id).await?;
    if old_run.status != "pending_approval" {
        return Err(AppError::BadRequest(
            "Only submitted payroll runs can be approved".into(),
        ));
    }

    let run = payroll_runs::set_approved(pool, run_id, company_id, actor_user_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("Payroll run could not be approved".into()))?;

    audit_transition(
        pool,
        company_id,
        actor_user_id,
        "approve",
        &old_run,
        &run,
        serde_json::to_value(&run).unwrap_or_default(),
        format!(
            "Approved payroll run for {:02}/{}",
            run.period_month, run.period_year
        ),
        audit_meta,
    )
    .await;

    Ok(run)
}

pub async fn return_for_changes(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    actor_user_id: Uuid,
    reason: Option<String>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollRun> {
    let old_run = load_run(pool, company_id, run_id).await?;
    if old_run.status != "pending_approval" {
        return Err(AppError::BadRequest(
            "Only submitted payroll runs can be returned for changes".into(),
        ));
    }

    let reason = reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(500).collect::<String>());

    let run = payroll_runs::set_returned(pool, run_id, company_id, actor_user_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("Payroll run could not be returned".into()))?;

    audit_transition(
        pool,
        company_id,
        actor_user_id,
        "return_changes",
        &old_run,
        &run,
        serde_json::json!({
            "payroll_run": run,
            "reason": reason,
        }),
        format!(
            "Returned payroll run for changes for {:02}/{}",
            old_run.period_month, old_run.period_year
        ),
        audit_meta,
    )
    .await;

    Ok(run)
}

pub async fn lock_as_paid(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    actor_user_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollRun> {
    let old_run = load_run(pool, company_id, run_id).await?;
    if old_run.status != "approved" {
        return Err(AppError::BadRequest(
            "Only approved payroll runs can be marked paid and locked".into(),
        ));
    }

    let run = payroll_runs::set_paid(pool, run_id, company_id, actor_user_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("Payroll run could not be locked".into()))?;

    audit_transition(
        pool,
        company_id,
        actor_user_id,
        "lock",
        &old_run,
        &run,
        serde_json::to_value(&run).unwrap_or_default(),
        format!(
            "Marked payroll run as paid and locked for {:02}/{}",
            run.period_month, run.period_year
        ),
        audit_meta,
    )
    .await;

    Ok(run)
}

/// Cancel a not-yet-paid run (plan item 11): the run becomes `cancelled`,
/// its staged entries and reimbursed claims return to their pre-run state,
/// and the period is free to re-run. The payslips stay on record so the
/// audit trail can show what was voided and why.
///
/// A PAID run cannot be cancelled here: money has left the building, and
/// pretending otherwise would let a click undo a bank file. Use
/// [`reverse_paid_run`].
pub async fn cancel_run(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    actor_user_id: Uuid,
    reason: Option<String>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollRun> {
    let old_run = load_run(pool, company_id, run_id).await?;
    if old_run.status == "paid" {
        return Err(AppError::BadRequest(
            "A paid payroll run cannot be cancelled; use the reverse endpoint, which also releases the claims and entries it consumed".into(),
        ));
    }
    if old_run.status == "cancelled" {
        return Err(AppError::BadRequest(
            "This payroll run is already cancelled".into(),
        ));
    }

    let reason = normalise_reason(reason);

    let mut tx = pool.begin().await?;
    let run = payroll_runs::set_cancelled(&mut *tx, run_id, company_id, actor_user_id)
        .await?
        .ok_or_else(|| AppError::Conflict("This run changed state concurrently".into()))?;
    // Release exactly what this run consumed — by run id, which is precise.
    crate::repositories::payroll_entries::revert_for_run(
        &mut *tx,
        run_id,
        company_id,
        actor_user_id,
    )
    .await?;
    crate::repositories::claims::revert_for_run(&mut *tx, run_id, company_id).await?;
    tx.commit().await?;

    audit_transition(
        pool,
        company_id,
        actor_user_id,
        "cancel",
        &old_run,
        &run,
        serde_json::json!({
            "payroll_run": run,
            "reason": reason,
        }),
        format!(
            "Cancelled {} payroll run for {:02}/{}",
            old_run.status, old_run.period_month, old_run.period_year
        ),
        audit_meta,
    )
    .await;

    Ok(run)
}

/// Reverse a PAID run (plan item 11): status becomes `cancelled`, consumed
/// claims and entries are released in the same transaction as the status
/// change, and the payslip history remains for auditors. The bank transfer
/// itself is the operator's to unwind — this endpoint makes the ledger tell
/// the truth about it, nothing more.
pub async fn reverse_paid_run(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    actor_user_id: Uuid,
    reason: Option<String>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollRun> {
    let old_run = load_run(pool, company_id, run_id).await?;
    if old_run.status != "paid" {
        return Err(AppError::BadRequest(format!(
            "Only paid payroll runs can be reversed; this one is '{}'. Cancel instead.",
            old_run.status
        )));
    }

    let reason = normalise_reason(reason);

    let mut tx = pool.begin().await?;
    let run = payroll_runs::set_reversed(&mut *tx, run_id, company_id, actor_user_id)
        .await?
        .ok_or_else(|| AppError::Conflict("This run changed state concurrently".into()))?;
    crate::repositories::payroll_entries::revert_for_run(
        &mut *tx,
        run_id,
        company_id,
        actor_user_id,
    )
    .await?;
    crate::repositories::claims::revert_for_run(&mut *tx, run_id, company_id).await?;
    tx.commit().await?;

    audit_transition(
        pool,
        company_id,
        actor_user_id,
        "reverse",
        &old_run,
        &run,
        serde_json::json!({
            "payroll_run": run,
            "reason": reason,
        }),
        format!(
            "Reversed paid payroll run for {:02}/{}",
            old_run.period_month, old_run.period_year
        ),
        audit_meta,
    )
    .await;

    Ok(run)
}

fn normalise_reason(reason: Option<String>) -> Option<String> {
    reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(500).collect::<String>())
}
