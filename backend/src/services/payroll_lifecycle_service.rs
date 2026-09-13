use sqlx::PgPool;
use uuid::Uuid;

use crate::core::auth::{AuthUser, Permission};
use crate::core::error::{AppError, AppResult};
use crate::models::payroll::PayrollRun;
use crate::repositories::reads::payroll as payroll_reads;
use crate::repositories::{claims, payroll_entries, payroll_runs};
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

/// Cancel a run before money moves, keeping the row as the audit record.
///
/// Cancellation is the mid-lifecycle exit the schema always had a status value
/// for but no transition could reach: a processed run that should not be
/// submitted, or an approved run discovered wrong before payment. `paid` stays
/// terminal — the money already moved, so recovery there is a corrective run,
/// not a status change. The row and its payslip items are kept (cancelled runs
/// are excluded from YTD, portal and report reads), while staged entries and
/// claims are released so a re-run can pick them up.
///
/// The required permission follows the state being cancelled: the preparer
/// (`ManagePayrollDraft`) can withdraw their own draft/processed run, but
/// cancelling what was submitted or approved belongs to the approver
/// (`ApprovePayroll`) — the same four-eyes boundary as approve/return.
pub async fn cancel(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    auth: &AuthUser,
    reason: Option<String>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollRun> {
    let reason = reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(500).collect::<String>())
        .ok_or_else(|| {
            AppError::BadRequest("A reason is required to cancel a payroll run".into())
        })?;

    let old_run = load_run(pool, company_id, run_id).await?;
    match old_run.status.as_str() {
        "draft" | "processed" => auth.require_permission(Permission::ManagePayrollDraft)?,
        "pending_approval" | "approved" => auth.require_permission(Permission::ApprovePayroll)?,
        "processing" => {
            return Err(AppError::BadRequest(
                "Payroll run is currently processing and cannot be cancelled".into(),
            ));
        }
        "paid" => {
            return Err(AppError::BadRequest(
                "Paid payroll runs cannot be cancelled — payment has already been recorded. Use the reverse endpoint for a paid run.".into(),
            ));
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "Payroll run is already {other}"
            )));
        }
    }

    // Same corruption rule as `delete_run`: a later committed run's frozen YTD
    // and PCB annualisation were computed from this run's figures.
    if payroll_reads::run_has_later_committed_run(pool, company_id, run_id).await? {
        return Err(AppError::BadRequest(
            "A later payroll run already includes these employees; cancel that run first".into(),
        ));
    }

    let actor_user_id = auth.0.sub;
    let mut tx = pool.begin().await?;
    payroll_entries::revert_for_run(&mut *tx, run_id, company_id, actor_user_id).await?;
    claims::revert_for_run(&mut *tx, run_id, company_id).await?;
    // Re-checked inside the transaction: the status predicates in the UPDATE
    // are the atomic guard against a concurrent submit/approve/pay.
    let run = payroll_runs::set_cancelled(&mut *tx, run_id, company_id, actor_user_id, &reason)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("Payroll run changed state while being cancelled".into())
        })?;
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
            "Cancelled payroll run for {:02}/{}",
            run.period_month, run.period_year
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

/// Reverse a PAID run (plan item 11): status becomes `cancelled`, consumed
/// claims and staged entries are released in the same transaction, and the
/// payslip history remains for auditors. The bank transfer itself is the
/// operator's to unwind — this endpoint makes the payroll ledger tell the
/// truth about it, nothing more.
///
/// Reversal is the only exit from `paid`, so it sits behind
/// `MarkPayrollPaid` — the same role trusted to mark money as moved decides
/// when it must be unwound. Reason is mandatory, same evidence bar as
/// [`cancel`].
pub async fn reverse_paid_run(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    auth: &AuthUser,
    reason: Option<String>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollRun> {
    auth.require_permission(Permission::MarkPayrollPaid)?;

    let reason = reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(500).collect::<String>())
        .ok_or_else(|| {
            AppError::BadRequest("A reason is required to reverse a paid payroll run".into())
        })?;

    let old_run = load_run(pool, company_id, run_id).await?;
    if old_run.status != "paid" {
        return Err(AppError::BadRequest(format!(
            "Only paid payroll runs can be reversed; this one is '{}'. Cancel instead.",
            old_run.status
        )));
    }

    if payroll_reads::run_has_later_committed_run(pool, company_id, run_id).await? {
        return Err(AppError::BadRequest(
            "A later payroll run already includes these employees; reverse that run first".into(),
        ));
    }

    let actor_user_id = auth.0.sub;
    let mut tx = pool.begin().await?;
    payroll_entries::revert_for_run(&mut *tx, run_id, company_id, actor_user_id).await?;
    claims::revert_for_run(&mut *tx, run_id, company_id).await?;
    let run = payroll_runs::set_reversed(&mut *tx, run_id, company_id, actor_user_id, &reason)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("Payroll run changed state while being reversed".into())
        })?;
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
            run.period_month, run.period_year
        ),
        audit_meta,
    )
    .await;

    Ok(run)
}
