//! Data access for the `companies` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::company::{Company, CreateCompanyRequest, UpdateCompanyRequest};

/// Create the idempotent payroll, leave, schedule, and settings baseline that
/// makes a new tenant usable without demo seed data.
pub async fn provision_defaults(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    actor_id: Option<Uuid>,
) -> AppResult<()> {
    sqlx::query("SELECT public.provision_company_defaults($1, $2)")
        .bind(company_id)
        .bind(actor_id)
        .execute(executor)
        .await?;
    Ok(())
}

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

/// Wipe every company-scoped row *except* the `companies` row itself and the
/// user linkage, children before parents.
///
/// This is the single wipe order for the platform. It previously existed twice
/// — here and in `repositories/backup.rs` — as two hand-maintained lists of the
/// same graph, and neither was right: this copy omitted `payroll_item_details`
/// and `tp3_records`, so deleting any tenant that had ever run payroll raised
/// 23503; the other omitted `email_logs`, which has NO ACTION foreign keys into
/// both `email_templates` and `employees`, so an overwrite restore failed on
/// its second statement for any tenant that had sent a templated letter.
///
/// Tables whose foreign key to `companies(id)` is ON DELETE CASCADE are
/// deliberately absent — `attendance_records`, `attendance_qr_tokens`,
/// `attendance_kiosk_credentials`, `company_locations`,
/// `company_work_schedules`, `employee_work_schedules`, `user_groups`,
/// `user_companies`, and `audit_logs` since migration 1008. They follow the
/// `companies` row. Everything listed here is a NO ACTION foreign key that has
/// to be removed by hand, and 1010's `claims_run_tenant_fkey` is NO ACTION on
/// purpose, so `claims` must precede `payroll_runs`.
///
/// Two consequences worth knowing before calling it. `DELETE FROM employees`
/// cascades `attendance_records` and `employee_work_schedules`, so an overwrite
/// restore destroys the target company's attendance history — pre-existing
/// behaviour, stated here now that it is shared. And `email_logs` are destroyed
/// and not restored: they are not part of `CompanyBackup`, and they reference
/// the templates and employees being replaced wholesale.
///
/// Runs many statements, so it takes the caller's transaction connection. Never
/// call it outside a transaction: a mis-ordered statement must roll the whole
/// wipe back rather than half-delete a tenant.
pub async fn delete_company_data(conn: &mut sqlx::PgConnection, company_id: Uuid) -> AppResult<()> {
    // Payroll money rows. `payroll_item_details` carries no company_id of its
    // own (1009 explains why it was left out of the tenant anchoring), so it is
    // reached through its parent.
    sqlx::query!(
        "DELETE FROM payroll_item_details WHERE payroll_item_id IN (SELECT pi.id FROM payroll_items pi JOIN payroll_runs pr ON pi.payroll_run_id = pr.id WHERE pr.company_id = $1)",
        company_id,
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "DELETE FROM payroll_items WHERE payroll_run_id IN (SELECT id FROM payroll_runs WHERE company_id = $1)",
        company_id,
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "DELETE FROM payroll_entries WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *conn)
    .await?;

    // Must precede both `email_templates` and `employees`.
    sqlx::query!("DELETE FROM email_logs WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "DELETE FROM notifications WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "DELETE FROM bulk_import_sessions WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *conn)
    .await?;

    sqlx::query!(
        "DELETE FROM team_members WHERE team_id IN (SELECT id FROM teams WHERE company_id = $1)",
        company_id,
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM teams WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;

    sqlx::query!(
        "DELETE FROM leave_balances WHERE employee_id IN (SELECT id FROM employees WHERE company_id = $1)",
        company_id,
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "DELETE FROM leave_requests WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM leave_types WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;

    // 1010 left `claims_run_tenant_fkey` NO ACTION on purpose so a forgotten
    // revert fails loudly; that makes this line ordering-critical, not tidy.
    sqlx::query!("DELETE FROM claims WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "DELETE FROM overtime_applications WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *conn)
    .await?;

    sqlx::query!("DELETE FROM documents WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "DELETE FROM document_categories WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *conn)
    .await?;

    // Anchored to the tenant by 1009, each with its own index, so these are
    // three indexed deletes rather than three subquery scans over `employees`.
    sqlx::query!("DELETE FROM tp3_records WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "DELETE FROM salary_history WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "DELETE FROM employee_allowances WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *conn)
    .await?;

    sqlx::query!(
        "DELETE FROM email_templates WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "DELETE FROM company_settings WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "DELETE FROM working_day_config WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM holidays WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;

    sqlx::query!("DELETE FROM employees WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;

    sqlx::query!("DELETE FROM payroll_runs WHERE company_id = $1", company_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "DELETE FROM payroll_groups WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Hard-delete a company and everything under it. Runs many statements, so it
/// takes the caller's transaction connection. Returns the number of company
/// rows removed (0 = the company did not exist).
///
/// The order lives in [`delete_company_data`]; this adds only what a tenant
/// teardown does beyond a data wipe. It used to end in a runtime
/// `format!("DELETE FROM {table} …")` loop — the one piece of dynamic SQL in the
/// repository layer, and precisely what let two tables go missing from the list
/// without the compiler noticing.
pub async fn delete_cascade(conn: &mut sqlx::PgConnection, company_id: Uuid) -> AppResult<u64> {
    delete_company_data(&mut *conn, company_id).await?;

    // Redundant — the FK is ON DELETE CASCADE — but kept because it states that
    // membership is company-scoped data rather than something the wipe forgot.
    sqlx::query!(
        "DELETE FROM user_companies WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *conn)
    .await?;

    // Clear company_id on users (nullable FK).
    sqlx::query!(
        "UPDATE users SET company_id = NULL WHERE company_id = $1",
        company_id,
    )
    .execute(&mut *conn)
    .await?;

    let result = sqlx::query!("DELETE FROM companies WHERE id = $1", company_id)
        .execute(&mut *conn)
        .await?;

    Ok(result.rows_affected())
}

/// Advance this company's auto-absent bookmark to `date`.
///
/// Per company, not platform-wide: one shared key could not express "A is done
/// through the 9th, B through the 10th", so a tenant on another calendar — or
/// one whose run failed — dragged every other tenant's backfill window with it.
pub async fn set_auto_absent_last_run_date(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    date: chrono::NaiveDate,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE companies SET auto_absent_last_run_date = $2 WHERE id = $1",
        company_id,
        date,
    )
    .execute(executor)
    .await?;
    Ok(())
}
