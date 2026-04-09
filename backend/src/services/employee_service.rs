use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::employee::{
    CreateEmployeeRequest, CreateTp3Request, Employee, SalaryHistory, Tp3Record,
    UpdateEmployeeRequest,
};

pub async fn list_employees(
    pool: &PgPool,
    company_id: Uuid,
    search: Option<&str>,
    department: Option<&str>,
    is_active: Option<bool>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<Employee>, i64)> {
    let count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM employees
        WHERE company_id = $1 AND deleted_at IS NULL
        AND ($2::bool IS NULL OR is_active = $2)
        AND ($3::text IS NULL OR full_name ILIKE '%' || $3 || '%' OR employee_number ILIKE '%' || $3 || '%')
        AND ($4::text IS NULL OR department = $4)"#,
    )
    .bind(company_id)
    .bind(is_active)
    .bind(search)
    .bind(department)
    .fetch_one(pool)
    .await?;

    let employees = sqlx::query_as::<_, Employee>(
        r#"SELECT id, company_id, employee_number, full_name, ic_number, passport_number,
            date_of_birth, gender::text, nationality, race::text, residency_status::text,
            marital_status::text, email, phone, address_line1, address_line2, city, state, postcode,
            department, designation, cost_centre, branch, employment_type::text,
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
    )
    .bind(company_id)
    .bind(is_active)
    .bind(search)
    .bind(department)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok((employees, count))
}

pub async fn get_employee(pool: &PgPool, id: Uuid, company_id: Uuid) -> AppResult<Employee> {
    sqlx::query_as::<_, Employee>(
        r#"SELECT id, company_id, employee_number, full_name, ic_number, passport_number,
            date_of_birth, gender::text, nationality, race::text, residency_status::text,
            marital_status::text, email, phone, address_line1, address_line2, city, state, postcode,
            department, designation, cost_centre, branch, employment_type::text,
            date_joined, probation_start, probation_end, confirmation_date,
            date_resigned, resignation_reason, basic_salary, hourly_rate, daily_rate,
            bank_name, bank_account_number, bank_account_type,
            tax_identification_number, epf_number, socso_number, eis_number,
            working_spouse, num_children, epf_category, is_muslim, zakat_eligible,
            zakat_monthly_amount, ptptn_monthly_amount, tabung_haji_amount,
            hrdf_contribution, payroll_group_id, salary_group, is_active,
            deleted_at, created_at, updated_at, created_by, updated_by
        FROM employees WHERE id = $1 AND company_id = $2 AND deleted_at IS NULL"#,
    )
    .bind(id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Employee not found".into()))
}

pub async fn create_employee(
    pool: &PgPool,
    company_id: Uuid,
    req: CreateEmployeeRequest,
    created_by: Uuid,
) -> AppResult<Employee> {
    let id = Uuid::new_v4();
    let emp = sqlx::query_as::<_, Employee>(
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
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
            $13, $14, $15, $16, $17, $18, $19,
            $20, $21, $22, $23, $24, $25, $26, $27,
            $28, $29, $30, $31, $32, $33, $34, $35, $36, $37,
            $38, $39, $40, $41, $42, $43, $44, $45, $46, $47, $48
        ) RETURNING id, company_id, employee_number, full_name, ic_number, passport_number,
            date_of_birth, gender::text, nationality, race::text, residency_status::text,
            marital_status::text, email, phone, address_line1, address_line2, city, state, postcode,
            department, designation, cost_centre, branch, employment_type::text,
            date_joined, probation_start, probation_end, confirmation_date,
            date_resigned, resignation_reason, basic_salary, hourly_rate, daily_rate,
            bank_name, bank_account_number, bank_account_type,
            tax_identification_number, epf_number, socso_number, eis_number,
            working_spouse, num_children, epf_category, is_muslim, zakat_eligible,
            zakat_monthly_amount, ptptn_monthly_amount, tabung_haji_amount,
            hrdf_contribution, payroll_group_id, salary_group, is_active,
            deleted_at, created_at, updated_at, created_by, updated_by"#,
    )
    .bind(id)
    .bind(company_id)
    .bind(&req.employee_number)
    .bind(&req.full_name)
    .bind(&req.ic_number)
    .bind(&req.passport_number)
    .bind(req.date_of_birth)
    .bind(&req.gender)
    .bind(&req.nationality)
    .bind(&req.race)
    .bind(req.residency_status.as_deref().unwrap_or("citizen"))
    .bind(&req.marital_status)
    .bind(&req.email)
    .bind(&req.phone)
    .bind(&req.address_line1)
    .bind(&req.address_line2)
    .bind(&req.city)
    .bind(&req.state)
    .bind(&req.postcode)
    .bind(&req.department)
    .bind(&req.designation)
    .bind(&req.cost_centre)
    .bind(&req.branch)
    .bind(req.employment_type.as_deref().unwrap_or("permanent"))
    .bind(req.date_joined)
    .bind(req.probation_start)
    .bind(req.probation_end)
    .bind(req.basic_salary)
    .bind(req.hourly_rate)
    .bind(req.daily_rate)
    .bind(&req.bank_name)
    .bind(&req.bank_account_number)
    .bind(&req.bank_account_type)
    .bind(&req.tax_identification_number)
    .bind(&req.epf_number)
    .bind(&req.socso_number)
    .bind(&req.eis_number)
    .bind(req.working_spouse)
    .bind(req.num_children)
    .bind(&req.epf_category)
    .bind(req.is_muslim)
    .bind(req.zakat_eligible)
    .bind(req.zakat_monthly_amount)
    .bind(req.ptptn_monthly_amount)
    .bind(req.tabung_haji_amount)
    .bind(req.payroll_group_id)
    .bind(&req.salary_group)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    Ok(emp)
}

pub async fn update_employee(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
    req: UpdateEmployeeRequest,
    updated_by: Uuid,
) -> AppResult<Employee> {
    let existing = get_employee(pool, id, company_id).await?;

    // Track salary change
    if let Some(new_salary) = req.basic_salary {
        if new_salary != existing.basic_salary {
            sqlx::query(
                r#"INSERT INTO salary_history (id, employee_id, old_salary, new_salary, effective_date, created_by)
                VALUES ($1, $2, $3, $4, NOW()::date, $5)"#,
            )
            .bind(Uuid::new_v4())
            .bind(id)
            .bind(existing.basic_salary)
            .bind(new_salary)
            .bind(updated_by)
            .execute(pool)
            .await?;
        }
    }

    let emp = sqlx::query_as::<_, Employee>(
        r#"UPDATE employees SET
            full_name = COALESCE($3, full_name),
            ic_number = COALESCE($4, ic_number),
            passport_number = COALESCE($5, passport_number),
            date_of_birth = COALESCE($6, date_of_birth),
            gender = COALESCE($7::gender_type, gender),
            nationality = COALESCE($8, nationality),
            race = COALESCE($9::race_type, race),
            residency_status = COALESCE($10::residency_status, residency_status),
            marital_status = COALESCE($11::marital_status, marital_status),
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
            employment_type = COALESCE($23::employment_type, employment_type),
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
            date_of_birth, gender::text, nationality, race::text, residency_status::text,
            marital_status::text, email, phone, address_line1, address_line2, city, state, postcode,
            department, designation, cost_centre, branch, employment_type::text,
            date_joined, probation_start, probation_end, confirmation_date,
            date_resigned, resignation_reason, basic_salary, hourly_rate, daily_rate,
            bank_name, bank_account_number, bank_account_type,
            tax_identification_number, epf_number, socso_number, eis_number,
            working_spouse, num_children, epf_category, is_muslim, zakat_eligible,
            zakat_monthly_amount, ptptn_monthly_amount, tabung_haji_amount,
            hrdf_contribution, payroll_group_id, salary_group, is_active,
            deleted_at, created_at, updated_at, created_by, updated_by"#,
    )
    .bind(id)
    .bind(company_id)
    .bind(&req.full_name)
    .bind(&req.ic_number)
    .bind(&req.passport_number)
    .bind(req.date_of_birth)
    .bind(&req.gender)
    .bind(&req.nationality)
    .bind(&req.race)
    .bind(&req.residency_status)
    .bind(&req.marital_status)
    .bind(&req.email)
    .bind(&req.phone)
    .bind(&req.address_line1)
    .bind(&req.address_line2)
    .bind(&req.city)
    .bind(&req.state)
    .bind(&req.postcode)
    .bind(&req.department)
    .bind(&req.designation)
    .bind(&req.cost_centre)
    .bind(&req.branch)
    .bind(&req.employment_type)
    .bind(req.probation_start)
    .bind(req.probation_end)
    .bind(req.confirmation_date)
    .bind(req.date_resigned)
    .bind(&req.resignation_reason)
    .bind(req.basic_salary)
    .bind(req.hourly_rate)
    .bind(req.daily_rate)
    .bind(&req.bank_name)
    .bind(&req.bank_account_number)
    .bind(&req.bank_account_type)
    .bind(&req.tax_identification_number)
    .bind(&req.epf_number)
    .bind(&req.socso_number)
    .bind(&req.eis_number)
    .bind(req.working_spouse)
    .bind(req.num_children)
    .bind(&req.epf_category)
    .bind(req.is_muslim)
    .bind(req.zakat_eligible)
    .bind(req.zakat_monthly_amount)
    .bind(req.ptptn_monthly_amount)
    .bind(req.tabung_haji_amount)
    .bind(req.hrdf_contribution)
    .bind(req.payroll_group_id)
    .bind(&req.salary_group)
    .bind(req.is_active)
    .bind(updated_by)
    .fetch_one(pool)
    .await?;

    Ok(emp)
}

pub async fn soft_delete_employee(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<()> {
    let rows = sqlx::query(
        "UPDATE employees SET deleted_at = NOW(), is_active = FALSE WHERE id = $1 AND company_id = $2",
    )
    .bind(id)
    .bind(company_id)
    .execute(pool)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound("Employee not found".into()));
    }
    Ok(())
}

pub async fn get_salary_history(pool: &PgPool, employee_id: Uuid) -> AppResult<Vec<SalaryHistory>> {
    let history = sqlx::query_as::<_, SalaryHistory>(
        "SELECT * FROM salary_history WHERE employee_id = $1 ORDER BY effective_date DESC",
    )
    .bind(employee_id)
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
    let record = sqlx::query_as::<_, Tp3Record>(
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
    )
    .bind(Uuid::new_v4())
    .bind(employee_id)
    .bind(req.tax_year)
    .bind(&req.previous_employer_name)
    .bind(req.previous_income_ytd)
    .bind(req.previous_epf_ytd)
    .bind(req.previous_pcb_ytd)
    .bind(req.previous_socso_ytd)
    .bind(req.previous_zakat_ytd.unwrap_or(0))
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    Ok(record)
}
