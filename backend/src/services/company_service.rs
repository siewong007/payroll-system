use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::company::{Company, CreateCompanyRequest, UpdateCompanyRequest};
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
/// transaction.
///
/// NOTE: this cascade spans ~25 tables owned by domains that are not yet on the
/// repositories layer, and uses a deliberate runtime-query loop for the uniform
/// `WHERE company_id = $1` deletes. It is intentionally left inline until those
/// domains have repos; the final cleanup commit will recompose it from per-table
/// `delete_by_company` calls. See docs/refactor-repositories-layer.md.
pub async fn delete_company(pool: &PgPool, company_id: Uuid) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    // Delete in dependency order (children before parents)

    // 1. Team members (via teams)
    sqlx::query!(
        "DELETE FROM team_members WHERE team_id IN (SELECT id FROM teams WHERE company_id = $1)",
        company_id,
    )
    .execute(&mut *tx)
    .await?;

    // 2. Leave balances (via employees)
    sqlx::query!("DELETE FROM leave_balances WHERE employee_id IN (SELECT id FROM employees WHERE company_id = $1)", company_id)
        .execute(&mut *tx).await?;

    // 3. Payroll items (via payroll_runs)
    sqlx::query!("DELETE FROM payroll_items WHERE payroll_run_id IN (SELECT id FROM payroll_runs WHERE company_id = $1)", company_id)
        .execute(&mut *tx).await?;

    // 4. Salary history & employee allowances (via employees)
    sqlx::query!("DELETE FROM salary_history WHERE employee_id IN (SELECT id FROM employees WHERE company_id = $1)", company_id)
        .execute(&mut *tx).await?;
    sqlx::query!("DELETE FROM employee_allowances WHERE employee_id IN (SELECT id FROM employees WHERE company_id = $1)", company_id)
        .execute(&mut *tx).await?;

    // 5. Tables with direct company_id FK
    let tables = [
        "overtime_applications",
        "claims",
        "leave_requests",
        "leave_types",
        "notifications",
        "email_logs",
        "email_templates",
        "bulk_import_sessions",
        "documents",
        "document_categories",
        "company_settings",
        "working_day_config",
        "holidays",
        "teams",
        "payroll_entries",
        "payroll_runs",
        "payroll_groups",
        "employees",
        "user_companies",
    ];

    for table in tables {
        let query = format!("DELETE FROM {} WHERE company_id = $1", table);
        sqlx::query(&query)
            .bind(company_id)
            .execute(&mut *tx)
            .await?;
    }

    // 6. Clear company_id on users (nullable FK)
    sqlx::query!(
        "UPDATE users SET company_id = NULL WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *tx)
    .await?;

    // 7. Delete the company itself
    let result = sqlx::query!("DELETE FROM companies WHERE id = $1", company_id)
        .execute(&mut *tx)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Company not found".into()));
    }

    tx.commit().await?;
    Ok(())
}

#[derive(Debug, serde::Serialize)]
pub struct CompanyStats {
    pub total_employees: i64,
    pub total_departments: i64,
    pub total_payroll_groups: i64,
    pub total_documents: i64,
}
