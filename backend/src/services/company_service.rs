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
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("Company name is required".into()));
    }
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
    access: &crate::models::upload::UploadAccess,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Company> {
    if req.name.as_deref().is_some_and(|n| n.trim().is_empty()) {
        return Err(AppError::BadRequest("Company name cannot be empty".into()));
    }
    if req.unpaid_leave_divisor.is_some_and(|divisor| divisor <= 0) {
        return Err(AppError::BadRequest(
            "Unpaid leave divisor must be greater than zero".into(),
        ));
    }

    let logo_url = req.logo_url.as_deref().filter(|s| !s.trim().is_empty());
    crate::core::upload_path::validate_optional_file_url(logo_url)?;
    crate::services::upload_service::ensure_upload_attach_authorized(pool, access, logo_url, false)
        .await?;

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
    let (total_employees, total_departments) =
        employees::count_active_and_departments(pool, company_id).await?;
    Ok(CompanyStats {
        total_employees,
        total_departments,
        total_payroll_groups: payroll_groups::count_active(pool, company_id).await?,
        total_documents: documents::count_active(pool, company_id).await?,
    })
}

/// Hard-delete a company and all of its data, in dependency order, inside one
/// transaction. The multi-table cascade lives in `companies::delete_cascade`.
pub async fn delete_company(pool: &PgPool, company_id: Uuid, actor_id: Uuid) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    let stored_files =
        crate::repositories::uploaded_files::list_stored_names_for_company(&mut *tx, company_id)
            .await?;

    let deleted = companies::delete_cascade_for_actor(&mut tx, company_id, Some(actor_id)).await?;
    if deleted == 0 {
        return Err(AppError::NotFound("Company not found".into()));
    }

    tx.commit().await?;

    for stored_name in stored_files {
        if let Ok(path) = crate::core::upload_path::stored_path(&stored_name) {
            let _ = tokio::fs::remove_file(&path).await;
        }
    }

    Ok(())
}
