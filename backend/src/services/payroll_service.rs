//! Read-side payroll-run management (summary, listing, items, groups). Run mutations
//! live in payroll_lifecycle_service (transitions) and the handler (delete / PCB edit,
//! pending migration).

use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::payroll::{PayrollGroup, PayrollItem, PayrollRun, PayrollSummary};
use crate::repositories::reads::payroll as payroll_reads;
use crate::repositories::{payroll_groups, payroll_items, payroll_runs};

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
