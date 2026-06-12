use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::employee::{
    CreateEmployeeRequest, CreateTp3Request, Employee, SalaryHistory, Tp3Record,
    UpdateEmployeeRequest,
};
use crate::services::audit_service::AuditRequestMeta;

pub async fn list_employees(
    pool: &PgPool,
    company_id: Uuid,
    search: Option<&str>,
    department: Option<&str>,
    is_active: Option<bool>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<Employee>, i64)> {
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
    .fetch_one(pool)
    .await?;

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
    .fetch_all(pool)
    .await?;

    Ok((employees, count))
}

pub async fn get_employee(pool: &PgPool, id: Uuid, company_id: Uuid) -> AppResult<Employee> {
    sqlx::query_as!(
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
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Employee not found".into()))
}

pub async fn create_employee(
    pool: &PgPool,
    company_id: Uuid,
    req: CreateEmployeeRequest,
    created_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<(Employee, Option<EmployeeAccountInfo>)> {
    // Check for duplicate employee number within the same company
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM employees WHERE company_id = $1 AND employee_number = $2 AND deleted_at IS NULL) AS "exists!""#,
        company_id,
        req.employee_number,
    )
    .fetch_one(pool)
    .await?;

    if exists {
        return Err(AppError::Conflict(format!(
            "Employee number '{}' already exists in this company",
            req.employee_number
        )));
    }

    let id = Uuid::now_v7();
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
    .fetch_one(pool)
    .await?;

    // Auto-create a user account for the employee if they have an email
    let account_info = create_user_for_employee(pool, &emp).await?;

    // Initialize leave balances for the current year (prorated for mid-year joiners)
    let current_year = chrono::Utc::now().year();
    let _ = crate::services::portal_service::initialize_leave_balances(
        pool,
        emp.id,
        company_id,
        emp.date_joined,
        current_year,
    )
    .await;

    // Audit Log
    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(created_by),
        "create_employee",
        "employee",
        Some(emp.id),
        None,
        Some(serde_json::to_value(&emp).unwrap_or_default()),
        Some(&format!(
            "Created employee {} ({})",
            emp.full_name, emp.employee_number
        )),
        audit_meta,
    )
    .await;

    Ok((emp, account_info))
}

#[derive(Debug, serde::Serialize)]
pub struct EmployeeAccountInfo {
    pub created: bool,
    pub email: String,
    pub role: String,
    pub default_password: Option<String>,
    pub message: String,
}

pub async fn create_user_for_employee(
    pool: &PgPool,
    emp: &Employee,
) -> AppResult<Option<EmployeeAccountInfo>> {
    let Some(ref email) = emp.email else {
        return Ok(None);
    };

    // Check if email already exists
    let existing = sqlx::query!("SELECT id, roles FROM users WHERE email = $1", email)
        .fetch_optional(pool)
        .await?;

    if let Some(row) = existing {
        let existing_id = row.id;
        if row.roles.as_slice() == ["employee"] {
            // Stale employee account — clean up and recreate below
            sqlx::query!("DELETE FROM user_companies WHERE user_id = $1", existing_id)
                .execute(pool)
                .await?;
            sqlx::query!("DELETE FROM refresh_tokens WHERE user_id = $1", existing_id)
                .execute(pool)
                .await?;
            sqlx::query!("DELETE FROM users WHERE id = $1", existing_id)
                .execute(pool)
                .await?;
        } else {
            // Non-employee user (admin, etc.) — link to this employee silently
            sqlx::query!(
                "UPDATE users SET employee_id = $1, company_id = $2 WHERE id = $3",
                emp.id,
                emp.company_id,
                existing_id,
            )
            .execute(pool)
            .await?;
            sqlx::query!(
                "INSERT INTO user_companies (user_id, company_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                existing_id,
                emp.company_id,
            )
            .execute(pool)
            .await?;
            return Ok(None);
        }
    }

    // Default password: IC number or "Welcome@123" if no IC
    let default_password = emp.ic_number.as_deref().unwrap_or("Welcome@123");
    let password_hash = bcrypt::hash(default_password, 12)
        .map_err(|e| AppError::Internal(format!("Failed to hash password: {}", e)))?;

    let user_id = Uuid::now_v7();
    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash, full_name, roles, company_id, employee_id, must_change_password)
        VALUES ($1, $2, $3, $4, ARRAY['employee']::VARCHAR(50)[], $5, $6, TRUE)"#,
        user_id,
        email,
        password_hash,
        emp.full_name,
        emp.company_id,
        emp.id,
    )
    .execute(pool)
    .await?;

    // Link user to company
    sqlx::query!(
        r#"INSERT INTO user_companies (user_id, company_id)
        VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
        user_id,
        emp.company_id,
    )
    .execute(pool)
    .await?;

    Ok(Some(EmployeeAccountInfo {
        created: true,
        email: email.clone(),
        role: "employee".into(),
        default_password: Some(default_password.to_string()),
        message: format!(
            "User account created for {}. Default password is their IC number.",
            emp.full_name
        ),
    }))
}

pub async fn update_employee(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
    req: UpdateEmployeeRequest,
    updated_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Employee> {
    let existing = get_employee(pool, id, company_id).await?;

    // Track salary change
    if let Some(new_salary) = req.basic_salary
        && new_salary != existing.basic_salary
    {
        sqlx::query!(
            r#"INSERT INTO salary_history (id, employee_id, old_salary, new_salary, effective_date, created_by)
                VALUES ($1, $2, $3, $4, NOW()::date, $5)"#,
            Uuid::now_v7(),
            id,
            existing.basic_salary,
            new_salary,
            updated_by,
        )
        .execute(pool)
        .await?;
    }

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
    .fetch_one(pool)
    .await?;

    // Audit Log
    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(updated_by),
        "update_employee",
        "employee",
        Some(emp.id),
        Some(serde_json::to_value(&existing).unwrap_or_default()),
        Some(serde_json::to_value(&emp).unwrap_or_default()),
        Some(&format!(
            "Updated employee {} ({})",
            emp.full_name, emp.employee_number
        )),
        audit_meta,
    )
    .await;

    Ok(emp)
}

pub async fn soft_delete_employee(pool: &PgPool, id: Uuid, company_id: Uuid) -> AppResult<()> {
    let rows = sqlx::query!(
        "UPDATE employees SET deleted_at = NOW(), is_active = FALSE, employee_number = employee_number || '_DEL_' || id::text WHERE id = $1 AND company_id = $2",
        id,
        company_id,
    )
    .execute(pool)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound("Employee not found".into()));
    }

    // Delete the user account linked to this employee
    sqlx::query!(
        "DELETE FROM user_companies WHERE user_id IN (SELECT id FROM users WHERE employee_id = $1)",
        id,
    )
    .execute(pool)
    .await?;
    sqlx::query!(
        "DELETE FROM refresh_tokens WHERE user_id IN (SELECT id FROM users WHERE employee_id = $1)",
        id,
    )
    .execute(pool)
    .await?;
    sqlx::query!("DELETE FROM users WHERE employee_id = $1", id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn get_salary_history(pool: &PgPool, employee_id: Uuid) -> AppResult<Vec<SalaryHistory>> {
    let history = sqlx::query_as!(
        SalaryHistory,
        "SELECT * FROM salary_history WHERE employee_id = $1 ORDER BY effective_date DESC",
        employee_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(history)
}

pub async fn create_tp3(
    pool: &PgPool,
    employee_id: Uuid,
    req: CreateTp3Request,
    created_by: Uuid,
) -> AppResult<Tp3Record> {
    let record = sqlx::query_as!(
        Tp3Record,
        r#"INSERT INTO tp3_records (
            id, employee_id, tax_year, previous_employer_name,
            previous_income_ytd, previous_epf_ytd, previous_pcb_ytd,
            previous_socso_ytd, previous_zakat_ytd, created_by
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ON CONFLICT (employee_id, tax_year)
        DO UPDATE SET
            previous_employer_name = EXCLUDED.previous_employer_name,
            previous_income_ytd = EXCLUDED.previous_income_ytd,
            previous_epf_ytd = EXCLUDED.previous_epf_ytd,
            previous_pcb_ytd = EXCLUDED.previous_pcb_ytd,
            previous_socso_ytd = EXCLUDED.previous_socso_ytd,
            previous_zakat_ytd = EXCLUDED.previous_zakat_ytd
        RETURNING *"#,
        Uuid::now_v7(),
        employee_id,
        req.tax_year,
        req.previous_employer_name,
        req.previous_income_ytd,
        req.previous_epf_ytd,
        req.previous_pcb_ytd,
        req.previous_socso_ytd,
        req.previous_zakat_ytd.unwrap_or(0),
        created_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}
