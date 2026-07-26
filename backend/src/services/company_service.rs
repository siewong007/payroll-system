use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::company::{Company, CompanyStats, CreateCompanyRequest, UpdateCompanyRequest};
use crate::repositories::{companies, documents, employees, payroll_groups};
use crate::services::audit_service::{self, AuditRequestMeta};

pub async fn get_company(pool: &PgPool, company_id: Uuid) -> AppResult<Company> {
    companies::get(pool, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Company not found".into()))
}

pub async fn create_company(
    pool: &PgPool,
    req: CreateCompanyRequest,
    created_by: Uuid,
) -> AppResult<Company> {
    let mut tx = pool.begin().await?;
    let company = companies::insert(&mut *tx, &req, created_by).await?;
    companies::provision_defaults(&mut *tx, company.id, Some(created_by)).await?;
    tx.commit().await?;
    Ok(company)
}

pub async fn list_companies(pool: &PgPool) -> AppResult<Vec<Company>> {
    companies::list(pool).await
}

pub async fn update_company(
    pool: &PgPool,
    company_id: Uuid,
    req: UpdateCompanyRequest,
    updated_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Company> {
    if req.unpaid_leave_divisor.is_some_and(|divisor| divisor <= 0) {
        return Err(AppError::BadRequest(
            "Unpaid leave divisor must be greater than zero".into(),
        ));
    }

    // The company profile carries the EPF/SOCSO/EIS employer numbers and the
    // unpaid-leave divisor that every payroll run reads, so a change here moves
    // money. Capture the prior state for the before/after pair.
    let existing = companies::get(pool, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Company not found".into()))?;

    let company = companies::update(pool, company_id, &req, updated_by)
        .await?
        .ok_or_else(|| AppError::NotFound("Company not found".into()))?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(updated_by),
        "update",
        "company",
        Some(company_id),
        Some(serde_json::to_value(&existing).unwrap_or_default()),
        Some(serde_json::to_value(&company).unwrap_or_default()),
        Some("Company profile updated"),
        audit_meta,
    )
    .await;

    Ok(company)
}

pub async fn get_company_stats(pool: &PgPool, company_id: Uuid) -> AppResult<CompanyStats> {
    Ok(CompanyStats {
        total_employees: employees::count_active(pool, company_id).await?,
        total_departments: employees::count_distinct_departments(pool, company_id).await?,
        total_payroll_groups: payroll_groups::count_active(pool, company_id).await?,
        total_documents: documents::count_active(pool, company_id).await?,
    })
}

/// Hard-delete a company and all of its data, in dependency order, inside one
/// transaction. The multi-table cascade lives in `companies::delete_cascade`.
pub async fn delete_company(pool: &PgPool, company_id: Uuid) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    let deleted = companies::delete_cascade(&mut tx, company_id).await?;
    if deleted == 0 {
        return Err(AppError::NotFound("Company not found".into()));
    }

    tx.commit().await?;
    Ok(())
}
