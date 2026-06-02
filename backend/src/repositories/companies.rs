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

/// The company's geofence mode (`none`/`warn`/`enforce`); `None` if the company is absent.
pub async fn get_geofence_mode(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Option<String>> {
    let mode = sqlx::query_scalar!(
        "SELECT geofence_mode FROM companies WHERE id = $1",
        company_id
    )
    .fetch_optional(executor)
    .await?;
    Ok(mode)
}

pub async fn set_geofence_mode(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    mode: &str,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE companies SET geofence_mode = $1 WHERE id = $2",
        mode,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Hard-delete a company and all of its data, in dependency order. Runs many
/// statements (including a runtime-query loop over the company-scoped tables), so
/// it takes the caller's transaction connection. Returns the number of company
/// rows removed (0 = the company did not exist).
pub async fn delete_cascade(conn: &mut sqlx::PgConnection, company_id: Uuid) -> AppResult<u64> {
    // Delete in dependency order (children before parents)

    // 1. Team members (via teams)
    sqlx::query!(
        "DELETE FROM team_members WHERE team_id IN (SELECT id FROM teams WHERE company_id = $1)",
        company_id,
    )
    .execute(&mut *conn)
    .await?;

    // 2. Leave balances (via employees)
    sqlx::query!("DELETE FROM leave_balances WHERE employee_id IN (SELECT id FROM employees WHERE company_id = $1)", company_id)
        .execute(&mut *conn).await?;

    // 3. Payroll items (via payroll_runs)
    sqlx::query!("DELETE FROM payroll_items WHERE payroll_run_id IN (SELECT id FROM payroll_runs WHERE company_id = $1)", company_id)
        .execute(&mut *conn).await?;

    // 4. Salary history & employee allowances (via employees)
    sqlx::query!("DELETE FROM salary_history WHERE employee_id IN (SELECT id FROM employees WHERE company_id = $1)", company_id)
        .execute(&mut *conn).await?;
    sqlx::query!("DELETE FROM employee_allowances WHERE employee_id IN (SELECT id FROM employees WHERE company_id = $1)", company_id)
        .execute(&mut *conn).await?;

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
            .execute(&mut *conn)
            .await?;
    }

    // 6. Clear company_id on users (nullable FK)
    sqlx::query!(
        "UPDATE users SET company_id = NULL WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *conn)
    .await?;

    // 7. Delete the company itself
    let result = sqlx::query!("DELETE FROM companies WHERE id = $1", company_id)
        .execute(&mut *conn)
        .await?;

    Ok(result.rows_affected())
}
