//! Read-side payroll-run management (summary, listing, items, groups). Run mutations
//! live in payroll_lifecycle_service (transitions) and the handler (delete / PCB edit,
//! pending migration).

use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::payroll::{PayrollGroup, PayrollItem, PayrollRun, PayrollSummary};
use crate::repositories::reads::payroll as payroll_reads;
use crate::repositories::{
    claims, payroll_entries, payroll_groups, payroll_item_details, payroll_items, payroll_runs,
};
use crate::services::audit_service::{self, AuditRequestMeta};

/// Run header + per-employee payslip summaries.
pub async fn get_summary(pool: &PgPool, company_id: Uuid, id: Uuid) -> AppResult<PayrollSummary> {
    let run = payroll_runs::get_for_company(pool, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))?;
    let items = payroll_reads::item_summaries_for_run(pool, id).await?;
    Ok(PayrollSummary {
        payroll_run: run,
        items,
    })
}

pub async fn list_runs(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<PayrollRun>> {
    payroll_runs::list_for_company(pool, company_id).await
}

pub async fn list_groups(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<PayrollGroup>> {
    payroll_groups::list_active(pool, company_id).await
}

pub async fn list_items(pool: &PgPool, run_id: Uuid) -> AppResult<Vec<PayrollItem>> {
    payroll_items::list_for_run(pool, run_id).await
}

/// Hard-delete a non-locked run and revert its staged entries/claims, in one transaction.
pub async fn delete_run(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let run = payroll_runs::get_for_company(pool, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))?;

    if run.status == "processing" {
        return Err(AppError::BadRequest(
            "Payroll run is currently processing and cannot be deleted".into(),
        ));
    }

    if matches!(
        run.status.as_str(),
        "pending_approval" | "approved" | "paid"
    ) || run.locked_at.is_some()
    {
        return Err(AppError::BadRequest(
            "Submitted, approved, or paid payroll runs are locked and cannot be deleted".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    payroll_entries::revert_for_run(&mut *tx, id, company_id, actor_id).await?;
    claims::revert_processed_for_period(&mut *tx, company_id, run.period_start, run.period_end)
        .await?;
    payroll_item_details::delete_for_run(&mut *tx, id).await?;
    payroll_items::delete_for_run(&mut *tx, id).await?;
    payroll_runs::delete(&mut *tx, id, company_id).await?;
    tx.commit().await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "delete",
        "payroll_run",
        Some(id),
        Some(serde_json::to_value(&run).unwrap_or_default()),
        None,
        Some(&format!(
            "Deleted payroll run {} for {:02}/{}",
            id, run.period_month, run.period_year
        )),
        audit_meta,
    )
    .await;

    Ok(())
}

/// Edit a single payslip's PCB while the run is still `processed`, recomputing the
/// item's deductions/net and the run totals in one locked transaction.
pub async fn update_item_pcb(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    employee_id: Uuid,
    pcb_amount: i64,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollSummary> {
    if pcb_amount < 0 {
        return Err(AppError::BadRequest("PCB amount cannot be negative".into()));
    }

    let mut tx = pool.begin().await?;

    let run_row = payroll_runs::get_status_locked(&mut *tx, run_id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))?;

    if run_row.status != "processed" {
        return Err(AppError::BadRequest(
            "PCB can only be edited while the payroll run is processed and not yet approved".into(),
        ));
    }

    let has_later_run = payroll_reads::employee_has_later_run(
        &mut *tx,
        employee_id,
        company_id,
        run_row.period_year,
        run_row.period_month,
    )
    .await?;

    if has_later_run {
        return Err(AppError::BadRequest(
            "PCB cannot be edited because a later payroll run already exists for this employee"
                .into(),
        ));
    }

    let current = payroll_items::get_pcb_fields_locked(&mut *tx, run_id, employee_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Payroll item not found".into()))?;

    let old_pcb = current.pcb_amount;
    let delta = pcb_amount - old_pcb;
    let new_total_deductions = current.total_deductions + delta;
    let new_net_salary = current.net_salary - delta;
    let new_ytd_pcb = current.ytd_pcb + delta;

    payroll_items::update_pcb(
        &mut *tx,
        run_id,
        employee_id,
        pcb_amount,
        new_total_deductions,
        new_net_salary,
        new_ytd_pcb,
    )
    .await?;

    payroll_runs::bump_pcb_totals(&mut *tx, run_id, company_id, delta, actor_id).await?;

    tx.commit().await?;

    let summary = get_summary(pool, company_id, run_id).await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "update",
        "payroll_item",
        None,
        Some(serde_json::json!({
            "payroll_run_id": run_id,
            "employee_id": employee_id,
            "pcb_amount": old_pcb
        })),
        Some(serde_json::json!({
            "payroll_run_id": run_id,
            "employee_id": employee_id,
            "pcb_amount": pcb_amount
        })),
        Some("Updated payroll item PCB amount"),
        audit_meta,
    )
    .await;

    Ok(summary)
}
