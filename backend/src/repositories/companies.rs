//! Data access for the `companies` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::company::{Company, CreateCompanyRequest, UpdateCompanyRequest};

pub async fn get(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Option<Company>> {
    let company = sqlx::query_as!(
        Company,
        "SELECT id, name, registration_number, tax_number, epf_number, socso_code, eis_code, hrdf_number, address_line1, address_line2, city, state, postcode, country, phone, email, logo_url, hrdf_enabled, unpaid_leave_divisor, is_active, created_at, updated_at, created_by, updated_by FROM companies WHERE id = $1",
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(company)
}

pub async fn list(executor: impl Executor<'_, Database = Postgres>) -> AppResult<Vec<Company>> {
    let companies = sqlx::query_as!(
        Company,
        "SELECT id, name, registration_number, tax_number, epf_number, socso_code, eis_code, hrdf_number, address_line1, address_line2, city, state, postcode, country, phone, email, logo_url, hrdf_enabled, unpaid_leave_divisor, is_active, created_at, updated_at, created_by, updated_by FROM companies ORDER BY name ASC"
    )
    .fetch_all(executor)
    .await?;
    Ok(companies)
}

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    req: &CreateCompanyRequest,
    created_by: Uuid,
) -> AppResult<Company> {
    let company = sqlx::query_as!(
        Company,
        r#"INSERT INTO companies (name, registration_number, tax_number, email, phone, created_by, updated_by)
        VALUES ($1, $2, $3, $4, $5, $6, $6)
        RETURNING id, name, registration_number, tax_number, epf_number, socso_code, eis_code, hrdf_number, address_line1, address_line2, city, state, postcode, country, phone, email, logo_url, hrdf_enabled, unpaid_leave_divisor, is_active, created_at, updated_at, created_by, updated_by"#,
        req.name,
        req.registration_number,
        req.tax_number,
        req.email,
        req.phone,
        created_by,
    )
    .fetch_one(executor)
    .await?;
    Ok(company)
}

pub async fn update(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    req: &UpdateCompanyRequest,
    updated_by: Uuid,
) -> AppResult<Option<Company>> {
    let company = sqlx::query_as!(
        Company,
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
        RETURNING id, name, registration_number, tax_number, epf_number, socso_code, eis_code, hrdf_number, address_line1, address_line2, city, state, postcode, country, phone, email, logo_url, hrdf_enabled, unpaid_leave_divisor, is_active, created_at, updated_at, created_by, updated_by"#,
        company_id,
        req.name,
        req.registration_number,
        req.tax_number,
        req.epf_number,
        req.socso_code,
        req.eis_code,
        req.hrdf_number,
        req.address_line1,
        req.address_line2,
        req.city,
        req.state,
        req.postcode,
        req.country,
        req.phone,
        req.email,
        req.logo_url,
        req.hrdf_enabled,
        req.unpaid_leave_divisor,
        updated_by,
    )
    .fetch_optional(executor)
    .await?;
    Ok(company)
}

/// The company's attendance-method override, if any (flattened from the nullable column).
pub async fn get_attendance_method(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Option<String>> {
    let method = sqlx::query_scalar!(
        "SELECT attendance_method FROM companies WHERE id = $1",
        company_id,
    )
    .fetch_optional(executor)
    .await?
    .flatten();
    Ok(method)
}

pub async fn set_attendance_method(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    method: Option<&str>,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE companies SET attendance_method = $1 WHERE id = $2",
        method,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}
