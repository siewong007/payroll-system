use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::company::{Company, CreateCompanyRequest, UpdateCompanyRequest};

pub async fn get_company(pool: &PgPool, company_id: Uuid) -> AppResult<Company> {
    sqlx::query_as::<_, Company>("SELECT * FROM companies WHERE id = $1")
        .bind(company_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Company not found".into()))
}

pub async fn create_company(
    pool: &PgPool,
    req: CreateCompanyRequest,
    created_by: Uuid,
) -> AppResult<Company> {
    let company = sqlx::query_as::<_, Company>(
        r#"INSERT INTO companies (name, registration_number, tax_number, email, phone, created_by, updated_by)
        VALUES ($1, $2, $3, $4, $5, $6, $6)
        RETURNING *"#,
    )
    .bind(&req.name)
    .bind(&req.registration_number)
    .bind(&req.tax_number)
    .bind(&req.email)
    .bind(&req.phone)
    .bind(created_by)
    .fetch_one(pool)
    .await?;
    Ok(company)
}

pub async fn list_companies(pool: &PgPool) -> AppResult<Vec<Company>> {
    let companies = sqlx::query_as::<_, Company>(
        "SELECT * FROM companies ORDER BY name ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(companies)
}

pub async fn update_company(
    pool: &PgPool,
    company_id: Uuid,
    req: UpdateCompanyRequest,
    updated_by: Uuid,
) -> AppResult<Company> {
    let company = sqlx::query_as::<_, Company>(
        r#"UPDATE companies SET
            name = COALESCE($2, name),
            registration_number = COALESCE($3, registration_number),
            tax_number = COALESCE($4, tax_number),
            epf_number = COALESCE($5, epf_number),
            socso_code = COALESCE($6, socso_code),
            eis_code = COALESCE($7, eis_code),
            hrdf_number = COALESCE($8, hrdf_number),
            address_line1 = COALESCE($9, address_line1),
            address_line2 = COALESCE($10, address_line2),
            city = COALESCE($11, city),
            state = COALESCE($12, state),
            postcode = COALESCE($13, postcode),
            country = COALESCE($14, country),
            phone = COALESCE($15, phone),
            email = COALESCE($16, email),
            logo_url = COALESCE($17, logo_url),
            hrdf_enabled = COALESCE($18, hrdf_enabled),
            unpaid_leave_divisor = COALESCE($19, unpaid_leave_divisor),
            updated_by = $20,
            updated_at = NOW()
        WHERE id = $1
        RETURNING *"#,
    )
    .bind(company_id)
    .bind(&req.name)
    .bind(&req.registration_number)
    .bind(&req.tax_number)
    .bind(&req.epf_number)
    .bind(&req.socso_code)
    .bind(&req.eis_code)
    .bind(&req.hrdf_number)
    .bind(&req.address_line1)
    .bind(&req.address_line2)
    .bind(&req.city)
    .bind(&req.state)
    .bind(&req.postcode)
    .bind(&req.country)
    .bind(&req.phone)
    .bind(&req.email)
    .bind(&req.logo_url)
    .bind(req.hrdf_enabled)
    .bind(req.unpaid_leave_divisor)
    .bind(updated_by)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Company not found".into()))?;

    Ok(company)
}

pub async fn get_company_stats(
    pool: &PgPool,
    company_id: Uuid,
) -> AppResult<CompanyStats> {
    let total_employees: Option<i64> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM employees WHERE company_id = $1 AND is_active = TRUE",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await?;

    let total_departments: Option<i64> = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT department) FROM employees WHERE company_id = $1 AND is_active = TRUE AND department IS NOT NULL",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await?;

    let total_payroll_groups: Option<i64> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM payroll_groups WHERE company_id = $1 AND is_active = TRUE",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await?;

    let total_documents: Option<i64> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM documents WHERE company_id = $1 AND deleted_at IS NULL",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await?;

    Ok(CompanyStats {
        total_employees: total_employees.unwrap_or(0),
        total_departments: total_departments.unwrap_or(0),
        total_payroll_groups: total_payroll_groups.unwrap_or(0),
        total_documents: total_documents.unwrap_or(0),
    })
}

#[derive(Debug, serde::Serialize)]
pub struct CompanyStats {
    pub total_employees: i64,
    pub total_departments: i64,
    pub total_payroll_groups: i64,
    pub total_documents: i64,
}
