//! Data access for the `employees` table.
//!
//! NOTE: the 60-column `Employee` projection (with `gender::text AS "gender?"`-style
//! enum casts) is repeated across `list`/`get`/`insert`/`update` because sqlx 0.8
//! requires a string literal in `query_as!` — a shared `const`/`concat!` will not
//! compile. Keep the four projections in sync. The follow-up "enum types" refactor
//! removes the casts and the duplication; see docs/refactor-repositories-layer.md §10.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::employee::{CreateEmployeeRequest, Employee, UpdateEmployeeRequest};

pub async fn count(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    search: Option<&str>,
    department: Option<&str>,
    is_active: Option<bool>,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM employees
        WHERE company_id = $1 AND deleted_at IS NULL
        AND ($2::bool IS NULL OR is_active = $2)
        AND ($3::text IS NULL OR full_name ILIKE '%' || $3 || '%' OR employee_number ILIKE '%' || $3 || '%')
        AND ($4::text IS NULL OR department = $4)"#,
        company_id,
        is_active,
        search,
        department,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}

pub async fn list(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    search: Option<&str>,
    department: Option<&str>,
    is_active: Option<bool>,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<Employee>> {
    let employees = sqlx::query_as!(
        Employee,
        r#"SELECT id, company_id, employee_number, full_name, ic_number, passport_number,
            date_of_birth, gender::text AS "gender?", nationality, race::text AS "race?", residency_status::text AS "residency_status!",
            marital_status::text AS "marital_status?", email, phone, address_line1, address_line2, city, state, postcode,
            department, designation, cost_centre, branch, employment_type::text AS "employment_type!",
            date_joined, probation_start, probation_end, confirmation_date,
            date_resigned, resignation_reason, basic_salary, hourly_rate, daily_rate,
            bank_name, bank_account_number, bank_account_type,
            tax_identification_number, epf_number, socso_number, eis_number,
            working_spouse, num_children, epf_category, is_muslim, zakat_eligible,
            zakat_monthly_amount, ptptn_monthly_amount, tabung_haji_amount,
            hrdf_contribution, payroll_group_id, salary_group, is_active,
            deleted_at, created_at, updated_at, created_by, updated_by
        FROM employees
        WHERE company_id = $1 AND deleted_at IS NULL
        AND ($2::bool IS NULL OR is_active = $2)
        AND ($3::text IS NULL OR full_name ILIKE '%' || $3 || '%' OR employee_number ILIKE '%' || $3 || '%')
        AND ($4::text IS NULL OR department = $4)
        ORDER BY employee_number ASC
        LIMIT $5 OFFSET $6"#,
        company_id,
        is_active,
        search,
        department,
        limit,
        offset,
    )
    .fetch_all(executor)
    .await?;
    Ok(employees)
}

pub async fn get(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<Employee>> {
    let employee = sqlx::query_as!(
        Employee,
        r#"SELECT id, company_id, employee_number, full_name, ic_number, passport_number,
            date_of_birth, gender::text AS "gender?", nationality, race::text AS "race?", residency_status::text AS "residency_status!",
            marital_status::text AS "marital_status?", email, phone, address_line1, address_line2, city, state, postcode,
            department, designation, cost_centre, branch, employment_type::text AS "employment_type!",
            date_joined, probation_start, probation_end, confirmation_date,
            date_resigned, resignation_reason, basic_salary, hourly_rate, daily_rate,
            bank_name, bank_account_number, bank_account_type,
            tax_identification_number, epf_number, socso_number, eis_number,
            working_spouse, num_children, epf_category, is_muslim, zakat_eligible,
            zakat_monthly_amount, ptptn_monthly_amount, tabung_haji_amount,
            hrdf_contribution, payroll_group_id, salary_group, is_active,
            deleted_at, created_at, updated_at, created_by, updated_by
        FROM employees WHERE id = $1 AND company_id = $2 AND deleted_at IS NULL"#,
        id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(employee)
}

/// An employee's own profile by id (no company filter), for the self-service portal.
pub async fn get_profile(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<Option<Employee>> {
    let employee = sqlx::query_as!(
        Employee,
        r#"SELECT id, company_id, employee_number, full_name, ic_number, passport_number,
            date_of_birth, gender::text AS "gender?", nationality, race::text AS "race?", residency_status::text AS "residency_status!",
            marital_status::text AS "marital_status?", email, phone, address_line1, address_line2, city, state, postcode,
            department, designation, cost_centre, branch, employment_type::text AS "employment_type!",
            date_joined, probation_start, probation_end, confirmation_date, date_resigned,
            resignation_reason, basic_salary, hourly_rate, daily_rate, bank_name,
            bank_account_number, bank_account_type, tax_identification_number, epf_number,
            socso_number, eis_number, working_spouse, num_children, epf_category, is_muslim,
            zakat_eligible, zakat_monthly_amount, ptptn_monthly_amount, tabung_haji_amount,
            hrdf_contribution, payroll_group_id, salary_group, is_active, deleted_at,
            created_at, updated_at, created_by, updated_by
        FROM employees WHERE id = $1 AND deleted_at IS NULL"#,
        id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(employee)
}

/// An employee's full name by id, if present (for notification messages).
pub async fn full_name(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<Option<String>> {
    let name = sqlx::query_scalar!("SELECT full_name FROM employees WHERE id = $1", id)
        .fetch_optional(executor)
        .await?;
    Ok(name)
}

/// Active flag for an employee, used by auth to reject logins for deleted staff.
/// `None` means the row is absent (also treated as inactive by callers).
pub async fn get_active_status(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<Option<bool>> {
    let status = sqlx::query_scalar!(
        r#"SELECT is_active AS "is_active!" FROM employees WHERE id = $1"#,
        id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(status)
}

/// Count of active employees for a company.
pub async fn count_active(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM employees WHERE company_id = $1 AND is_active = TRUE"#,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}

/// Count of distinct non-null departments among active employees for a company.
pub async fn count_distinct_departments(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(DISTINCT department) AS "count!" FROM employees WHERE company_id = $1 AND is_active = TRUE AND department IS NOT NULL"#,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}

/// Active employees in a payroll group who were employed during the period.
pub async fn list_for_payroll_run(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    payroll_group_id: Uuid,
    period_end: NaiveDate,
    period_start: NaiveDate,
) -> AppResult<Vec<Employee>> {
    let employees = sqlx::query_as!(
        Employee,
        r#"SELECT id, company_id, employee_number, full_name, ic_number, passport_number,
            date_of_birth, gender::text AS "gender?", nationality, race::text AS "race?",
            residency_status::text AS "residency_status!", marital_status::text AS "marital_status?",
            email, phone, address_line1, address_line2, city, state, postcode,
            department, designation, cost_centre, branch, employment_type::text AS "employment_type!",
            date_joined, probation_start, probation_end, confirmation_date,
            date_resigned, resignation_reason, basic_salary, hourly_rate, daily_rate,
            bank_name, bank_account_number, bank_account_type,
            tax_identification_number, epf_number, socso_number, eis_number,
            working_spouse, num_children, epf_category, is_muslim, zakat_eligible,
            zakat_monthly_amount, ptptn_monthly_amount, tabung_haji_amount,
            hrdf_contribution, payroll_group_id, salary_group, is_active,
            deleted_at, created_at, updated_at, created_by, updated_by
        FROM employees
        WHERE company_id = $1 AND payroll_group_id = $2
        AND is_active = TRUE AND deleted_at IS NULL
        AND date_joined <= $3
        AND (date_resigned IS NULL OR date_resigned >= $4)"#,
        company_id,
        payroll_group_id,
        period_end,
        period_start,
    )
    .fetch_all(executor)
    .await?;
    Ok(employees)
}

pub async fn exists_by_number(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_number: &str,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM employees WHERE company_id = $1 AND employee_number = $2 AND deleted_at IS NULL) AS "exists!""#,
        company_id,
        employee_number,
    )
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// Whether an active (non-deleted) employee with this id exists in the company.
pub async fn exists_in_company(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    company_id: Uuid,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(
            SELECT 1 FROM employees
            WHERE id = $1 AND company_id = $2 AND deleted_at IS NULL
        ) AS "exists!""#,
        employee_id,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// An employee's basic salary (sen) and owning company, by id. Used by leave
/// approval to compute the unpaid-leave daily-rate deduction.
pub async fn basic_salary_and_company(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<Option<(i64, Uuid)>> {
    let row = sqlx::query!(
        "SELECT basic_salary, company_id FROM employees WHERE id = $1",
        id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|r| (r.basic_salary, r.company_id)))
}

/// An employee's optional hourly rate and basic salary (sen), by id. Used by OT
/// approval to derive the overtime hourly rate.
pub async fn overtime_rate_basis(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<Option<(Option<i64>, i64)>> {
    let row = sqlx::query!(
        "SELECT hourly_rate, basic_salary FROM employees WHERE id = $1",
        id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|r| (r.hourly_rate, r.basic_salary)))
}

/// Existing `(employee_number, ic_number)` pairs for a company's non-deleted
/// employees, used to detect duplicates during bulk import.
pub async fn existing_numbers_and_ics(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<(String, Option<String>)>> {
    let rows = sqlx::query!(
        "SELECT employee_number, ic_number FROM employees WHERE company_id = $1 AND deleted_at IS NULL",
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.employee_number, r.ic_number))
        .collect())
}

/// Insert one employee from a bulk-import row (no RETURNING; the caller handles
/// per-row success/failure). Enum columns are cast from text; missing
/// residency/employment default to citizen/permanent.
//
// NOTE: indentation matches the byte-exact SQL in the offline `.sqlx` cache
// (this INSERT was nested inside the per-row import loop).
pub async fn insert_bulk_import(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    req: &CreateEmployeeRequest,
    created_by: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO employees (
                id, company_id, employee_number, full_name, ic_number, passport_number,
                date_of_birth, gender, nationality, race, residency_status, marital_status,
                email, phone, address_line1, address_line2, city, state, postcode,
                department, designation, cost_centre, branch,
                employment_type, date_joined, probation_start, probation_end,
                basic_salary, hourly_rate, daily_rate,
                bank_name, bank_account_number, bank_account_type,
                tax_identification_number, epf_number, socso_number, eis_number,
                working_spouse, num_children, epf_category,
                is_muslim, zakat_eligible, zakat_monthly_amount,
                ptptn_monthly_amount, tabung_haji_amount,
                payroll_group_id, salary_group,
                created_by
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8::text::gender_type, $9, $10::text::race_type,
                $11::text::residency_status, $12::text::marital_status,
                $13, $14, $15, $16, $17, $18, $19,
                $20, $21, $22, $23, $24::text::employment_type, $25, $26, $27,
                $28, $29, $30, $31, $32, $33, $34, $35, $36, $37,
                $38, $39, $40, $41, $42, $43, $44, $45, $46, $47, $48
            )"#,
        id,
        company_id,
        req.employee_number,
        req.full_name,
        req.ic_number,
        req.passport_number,
        req.date_of_birth,
        req.gender,
        req.nationality,
        req.race,
        req.residency_status.as_deref().unwrap_or("citizen"),
        req.marital_status,
        req.email,
        req.phone,
        req.address_line1,
        req.address_line2,
        req.city,
        req.state,
        req.postcode,
        req.department,
        req.designation,
        req.cost_centre,
        req.branch,
        req.employment_type.as_deref().unwrap_or("permanent"),
        req.date_joined,
        req.probation_start,
        req.probation_end,
        req.basic_salary,
        req.hourly_rate,
        req.daily_rate,
        req.bank_name,
        req.bank_account_number,
        req.bank_account_type,
        req.tax_identification_number,
        req.epf_number,
        req.socso_number,
        req.eis_number,
        req.working_spouse,
        req.num_children,
        req.epf_category,
        req.is_muslim,
        req.zakat_eligible,
        req.zakat_monthly_amount,
        req.ptptn_monthly_amount,
        req.tabung_haji_amount,
        req.payroll_group_id,
        req.salary_group,
        created_by,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    req: &CreateEmployeeRequest,
    created_by: Uuid,
) -> AppResult<Employee> {
    let emp = sqlx::query_as!(
        Employee,
        r#"INSERT INTO employees (
            id, company_id, employee_number, full_name, ic_number, passport_number,
            date_of_birth, gender, nationality, race, residency_status, marital_status,
            email, phone, address_line1, address_line2, city, state, postcode,
            department, designation, cost_centre, branch,
            employment_type, date_joined, probation_start, probation_end,
            basic_salary, hourly_rate, daily_rate,
            bank_name, bank_account_number, bank_account_type,
            tax_identification_number, epf_number, socso_number, eis_number,
            working_spouse, num_children, epf_category,
            is_muslim, zakat_eligible, zakat_monthly_amount,
            ptptn_monthly_amount, tabung_haji_amount,
            payroll_group_id, salary_group,
            created_by
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8::text::gender_type, $9, $10::text::race_type, $11::text::residency_status, $12::text::marital_status,
            $13, $14, $15, $16, $17, $18, $19,
            $20, $21, $22, $23, $24::text::employment_type, $25, $26, $27,
            $28, $29, $30, $31, $32, $33, $34, $35, $36, $37,
            $38, $39, $40, $41, $42, $43, $44, $45, $46, $47, $48
        ) RETURNING id, company_id, employee_number, full_name, ic_number, passport_number,
            date_of_birth, gender::text AS "gender?", nationality, race::text AS "race?", residency_status::text AS "residency_status!",
            marital_status::text AS "marital_status?", email, phone, address_line1, address_line2, city, state, postcode,
            department, designation, cost_centre, branch, employment_type::text AS "employment_type!",
            date_joined, probation_start, probation_end, confirmation_date,
            date_resigned, resignation_reason, basic_salary, hourly_rate, daily_rate,
            bank_name, bank_account_number, bank_account_type,
            tax_identification_number, epf_number, socso_number, eis_number,
            working_spouse, num_children, epf_category, is_muslim, zakat_eligible,
            zakat_monthly_amount, ptptn_monthly_amount, tabung_haji_amount,
            hrdf_contribution, payroll_group_id, salary_group, is_active,
            deleted_at, created_at, updated_at, created_by, updated_by"#,
        id,
        company_id,
        req.employee_number,
        req.full_name,
        req.ic_number,
        req.passport_number,
        req.date_of_birth,
        req.gender,
        req.nationality,
        req.race,
        req.residency_status.as_deref().unwrap_or("citizen"),
        req.marital_status,
        req.email,
        req.phone,
        req.address_line1,
        req.address_line2,
        req.city,
        req.state,
        req.postcode,
        req.department,
        req.designation,
        req.cost_centre,
        req.branch,
        req.employment_type.as_deref().unwrap_or("permanent"),
        req.date_joined,
        req.probation_start,
        req.probation_end,
        req.basic_salary,
        req.hourly_rate,
        req.daily_rate,
        req.bank_name,
        req.bank_account_number,
        req.bank_account_type,
        req.tax_identification_number,
        req.epf_number,
        req.socso_number,
        req.eis_number,
        req.working_spouse,
        req.num_children,
        req.epf_category,
        req.is_muslim,
        req.zakat_eligible,
        req.zakat_monthly_amount,
        req.ptptn_monthly_amount,
        req.tabung_haji_amount,
        req.payroll_group_id,
        req.salary_group,
        created_by,
    )
    .fetch_one(executor)
    .await?;
    Ok(emp)
}

pub async fn update(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    req: &UpdateEmployeeRequest,
    updated_by: Uuid,
) -> AppResult<Employee> {
    let emp = sqlx::query_as!(
        Employee,
        r#"UPDATE employees SET
            full_name = COALESCE($3, full_name),
            ic_number = COALESCE($4, ic_number),
            passport_number = COALESCE($5, passport_number),
            date_of_birth = COALESCE($6, date_of_birth),
            gender = COALESCE($7::text::gender_type, gender),
            nationality = COALESCE($8, nationality),
            race = COALESCE($9::text::race_type, race),
            residency_status = COALESCE($10::text::residency_status, residency_status),
            marital_status = COALESCE($11::text::marital_status, marital_status),
            email = COALESCE($12, email),
            phone = COALESCE($13, phone),
            address_line1 = COALESCE($14, address_line1),
            address_line2 = COALESCE($15, address_line2),
            city = COALESCE($16, city),
            state = COALESCE($17, state),
            postcode = COALESCE($18, postcode),
            department = COALESCE($19, department),
            designation = COALESCE($20, designation),
            cost_centre = COALESCE($21, cost_centre),
            branch = COALESCE($22, branch),
            employment_type = COALESCE($23::text::employment_type, employment_type),
            probation_start = COALESCE($24, probation_start),
            probation_end = COALESCE($25, probation_end),
            confirmation_date = COALESCE($26, confirmation_date),
            date_resigned = COALESCE($27, date_resigned),
            resignation_reason = COALESCE($28, resignation_reason),
            basic_salary = COALESCE($29, basic_salary),
            hourly_rate = COALESCE($30, hourly_rate),
            daily_rate = COALESCE($31, daily_rate),
            bank_name = COALESCE($32, bank_name),
            bank_account_number = COALESCE($33, bank_account_number),
            bank_account_type = COALESCE($34, bank_account_type),
            tax_identification_number = COALESCE($35, tax_identification_number),
            epf_number = COALESCE($36, epf_number),
            socso_number = COALESCE($37, socso_number),
            eis_number = COALESCE($38, eis_number),
            working_spouse = COALESCE($39, working_spouse),
            num_children = COALESCE($40, num_children),
            epf_category = COALESCE($41, epf_category),
            is_muslim = COALESCE($42, is_muslim),
            zakat_eligible = COALESCE($43, zakat_eligible),
            zakat_monthly_amount = COALESCE($44, zakat_monthly_amount),
            ptptn_monthly_amount = COALESCE($45, ptptn_monthly_amount),
            tabung_haji_amount = COALESCE($46, tabung_haji_amount),
            hrdf_contribution = COALESCE($47, hrdf_contribution),
            payroll_group_id = COALESCE($48, payroll_group_id),
            salary_group = COALESCE($49, salary_group),
            is_active = COALESCE($50, is_active),
            updated_by = $51,
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND deleted_at IS NULL
        RETURNING id, company_id, employee_number, full_name, ic_number, passport_number,
            date_of_birth, gender::text AS "gender?", nationality, race::text AS "race?", residency_status::text AS "residency_status!",
            marital_status::text AS "marital_status?", email, phone, address_line1, address_line2, city, state, postcode,
            department, designation, cost_centre, branch, employment_type::text AS "employment_type!",
            date_joined, probation_start, probation_end, confirmation_date,
            date_resigned, resignation_reason, basic_salary, hourly_rate, daily_rate,
            bank_name, bank_account_number, bank_account_type,
            tax_identification_number, epf_number, socso_number, eis_number,
            working_spouse, num_children, epf_category, is_muslim, zakat_eligible,
            zakat_monthly_amount, ptptn_monthly_amount, tabung_haji_amount,
            hrdf_contribution, payroll_group_id, salary_group, is_active,
            deleted_at, created_at, updated_at, created_by, updated_by"#,
        id,
        company_id,
        req.full_name,
        req.ic_number,
        req.passport_number,
        req.date_of_birth,
        req.gender,
        req.nationality,
        req.race,
        req.residency_status,
        req.marital_status,
        req.email,
        req.phone,
        req.address_line1,
        req.address_line2,
        req.city,
        req.state,
        req.postcode,
        req.department,
        req.designation,
        req.cost_centre,
        req.branch,
        req.employment_type,
        req.probation_start,
        req.probation_end,
        req.confirmation_date,
        req.date_resigned,
        req.resignation_reason,
        req.basic_salary,
        req.hourly_rate,
        req.daily_rate,
        req.bank_name,
        req.bank_account_number,
        req.bank_account_type,
        req.tax_identification_number,
        req.epf_number,
        req.socso_number,
        req.eis_number,
        req.working_spouse,
        req.num_children,
        req.epf_category,
        req.is_muslim,
        req.zakat_eligible,
        req.zakat_monthly_amount,
        req.ptptn_monthly_amount,
        req.tabung_haji_amount,
        req.hrdf_contribution,
        req.payroll_group_id,
        req.salary_group,
        req.is_active,
        updated_by,
    )
    .fetch_one(executor)
    .await?;
    Ok(emp)
}

pub async fn soft_delete(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        "UPDATE employees SET deleted_at = NOW(), is_active = FALSE, employee_number = employee_number || '_DEL_' || id::text WHERE id = $1 AND company_id = $2",
        id,
        company_id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}

pub async fn list_active_ids_and_joined_dates(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<(Uuid, NaiveDate)>> {
    let rows = sqlx::query!(
        "SELECT id, date_joined FROM employees WHERE company_id = $1 AND is_active = TRUE AND deleted_at IS NULL",
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows.into_iter().map(|r| (r.id, r.date_joined)).collect())
}
