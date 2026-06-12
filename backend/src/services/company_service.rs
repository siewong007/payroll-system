use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::company::{Company, CompanyStats, CreateCompanyRequest, UpdateCompanyRequest};
use crate::repositories::{companies, documents, employees, payroll_groups};

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
    companies::insert(pool, &req, created_by).await
}

pub async fn list_companies(pool: &PgPool) -> AppResult<Vec<Company>> {
    companies::list(pool).await
}

pub async fn update_company(
    pool: &PgPool,
    company_id: Uuid,
    req: UpdateCompanyRequest,
    updated_by: Uuid,
) -> AppResult<Company> {
    companies::update(pool, company_id, &req, updated_by)
        .await?
        .ok_or_else(|| AppError::NotFound("Company not found".into()))
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
