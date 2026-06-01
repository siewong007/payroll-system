use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::payroll::{
    CreatePayrollEntryRequest, PayrollEntry, PayrollEntryWithEmployee, UpdatePayrollEntryRequest,
};
use crate::repositories::reads::payroll as payroll_reads;
use crate::repositories::{employees, payroll_entries};
use crate::services::audit_service::{self, AuditRequestMeta};

fn validate_payroll_entry(
    period_year: i32,
    period_month: i32,
    category: &str,
    item_type: &str,
    description: &str,
    amount: i64,
) -> AppResult<()> {
    if !(1..=12).contains(&period_month) {
        return Err(AppError::BadRequest("Payroll month must be 1-12".into()));
    }

    if !(1900..=3000).contains(&period_year) {
        return Err(AppError::BadRequest("Payroll year is invalid".into()));
    }

    if !["earning", "deduction"].contains(&category) {
        return Err(AppError::BadRequest(
            "Category must be earning or deduction".into(),
        ));
    }

    if item_type.trim().is_empty() {
        return Err(AppError::BadRequest("Item type is required".into()));
    }

    if description.trim().is_empty() {
        return Err(AppError::BadRequest("Description is required".into()));
    }

    if amount <= 0 {
        return Err(AppError::BadRequest(
            "Amount must be greater than zero".into(),
        ));
    }

    Ok(())
}

async fn ensure_employee_in_company(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
) -> AppResult<()> {
    if !employees::exists_in_company(pool, employee_id, company_id).await? {
        return Err(AppError::NotFound(
            "Employee not found in the active company".into(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn list_entries(
    pool: &PgPool,
    company_id: Uuid,
    period_year: Option<i32>,
    period_month: Option<i32>,
    employee_id: Option<Uuid>,
    item_type: Option<&str>,
    include_processed: bool,
) -> AppResult<Vec<PayrollEntryWithEmployee>> {
    payroll_reads::entries_with_employee(
        pool,
        company_id,
        period_year,
        period_month,
        employee_id,
        item_type,
        include_processed,
    )
    .await
}

pub async fn create_entry(
    pool: &PgPool,
    company_id: Uuid,
    req: CreatePayrollEntryRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollEntry> {
    validate_payroll_entry(
        req.period_year,
        req.period_month,
        &req.category,
        &req.item_type,
        &req.description,
        req.amount,
    )?;
    ensure_employee_in_company(pool, company_id, req.employee_id).await?;

    let entry = payroll_entries::insert(
        pool,
        req.employee_id,
        company_id,
        req.period_year,
        req.period_month,
        req.category.trim(),
        req.item_type.trim(),
        req.description.trim(),
        req.amount,
        req.quantity,
        req.rate,
        req.is_taxable,
        actor_id,
    )
    .await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "create",
        "payroll_entry",
        Some(entry.id),
        None,
        Some(serde_json::to_value(&entry).unwrap_or_default()),
        Some("Created payroll adjustment entry"),
        audit_meta,
    )
    .await;

    Ok(entry)
}

pub async fn update_entry(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
    req: UpdatePayrollEntryRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollEntry> {
    let current = payroll_entries::get_unprocessed(pool, id, company_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("Payroll entry not found or already processed".into())
        })?;

    let employee_id = req.employee_id.unwrap_or(current.employee_id);
    let period_year = req.period_year.unwrap_or(current.period_year);
    let period_month = req.period_month.unwrap_or(current.period_month);
    let category = req.category.unwrap_or_else(|| current.category.clone());
    let item_type = req.item_type.unwrap_or_else(|| current.item_type.clone());
    let description = req
        .description
        .unwrap_or_else(|| current.description.clone());
    let amount = req.amount.unwrap_or(current.amount);

    validate_payroll_entry(
        period_year,
        period_month,
        &category,
        &item_type,
        &description,
        amount,
    )?;
    ensure_employee_in_company(pool, company_id, employee_id).await?;

    let updated = payroll_entries::update(
        pool,
        id,
        company_id,
        employee_id,
        period_year,
        period_month,
        category.trim(),
        item_type.trim(),
        description.trim(),
        amount,
        req.quantity.or(current.quantity),
        req.rate.or(current.rate),
        req.is_taxable,
        actor_id,
    )
    .await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "update",
        "payroll_entry",
        Some(id),
        Some(serde_json::to_value(&current).unwrap_or_default()),
        Some(serde_json::to_value(&updated).unwrap_or_default()),
        Some("Updated payroll adjustment entry"),
        audit_meta,
    )
    .await;

    Ok(updated)
}

pub async fn delete_entry(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let current = payroll_entries::get_unprocessed(pool, id, company_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("Payroll entry not found or already processed".into())
        })?;

    payroll_entries::delete_unprocessed(pool, id, company_id).await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "delete",
        "payroll_entry",
        Some(id),
        Some(serde_json::to_value(&current).unwrap_or_default()),
        None,
        Some("Deleted payroll adjustment entry"),
        audit_meta,
    )
    .await;

    Ok(())
}
