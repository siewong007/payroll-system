use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::employee::Employee;
use crate::models::portal::*;
use crate::services::notification_service;

// ─── Profile ───

pub async fn get_my_profile(pool: &PgPool, employee_id: Uuid) -> AppResult<Employee> {
    sqlx::query_as::<_, Employee>(
        r#"SELECT id, company_id, employee_number, full_name, ic_number, passport_number,
            date_of_birth, gender::text, nationality, race::text, residency_status::text,
            marital_status::text, email, phone, address_line1, address_line2, city, state,
            postcode, department, designation, cost_centre, branch, employment_type::text,
            date_joined, probation_start, probation_end, confirmation_date, date_resigned,
            resignation_reason, basic_salary, hourly_rate, daily_rate, bank_name,
            bank_account_number, bank_account_type, tax_identification_number, epf_number,
            socso_number, eis_number, working_spouse, num_children, epf_category, is_muslim,
            zakat_eligible, zakat_monthly_amount, ptptn_monthly_amount, tabung_haji_amount,
            hrdf_contribution, payroll_group_id, salary_group, is_active, deleted_at,
            created_at, updated_at, created_by, updated_by
        FROM employees WHERE id = $1 AND deleted_at IS NULL"#,
    )
    .bind(employee_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Employee profile not found".into()))
}

pub async fn update_my_profile(
    pool: &PgPool,
    employee_id: Uuid,
    req: UpdateMyProfileRequest,
) -> AppResult<Employee> {
    sqlx::query_as::<_, Employee>(
        r#"UPDATE employees SET
            phone = COALESCE($2, phone),
            email = COALESCE($3, email),
            address_line1 = COALESCE($4, address_line1),
            address_line2 = COALESCE($5, address_line2),
            city = COALESCE($6, city),
            state = COALESCE($7, state),
            postcode = COALESCE($8, postcode),
            marital_status = COALESCE($9, marital_status),
            bank_name = COALESCE($10, bank_name),
            bank_account_number = COALESCE($11, bank_account_number),
            updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        RETURNING id, company_id, employee_number, full_name, ic_number, passport_number,
            date_of_birth, gender::text, nationality, race::text, residency_status::text,
            marital_status::text, email, phone, address_line1, address_line2, city, state,
            postcode, department, designation, cost_centre, branch, employment_type::text,
            date_joined, probation_start, probation_end, confirmation_date, date_resigned,
            resignation_reason, basic_salary, hourly_rate, daily_rate, bank_name,
            bank_account_number, bank_account_type, tax_identification_number, epf_number,
            socso_number, eis_number, working_spouse, num_children, epf_category, is_muslim,
            zakat_eligible, zakat_monthly_amount, ptptn_monthly_amount, tabung_haji_amount,
            hrdf_contribution, payroll_group_id, salary_group, is_active, deleted_at,
            created_at, updated_at, created_by, updated_by"#,
    )
    .bind(employee_id)
    .bind(&req.phone)
    .bind(&req.email)
    .bind(&req.address_line1)
    .bind(&req.address_line2)
    .bind(&req.city)
    .bind(&req.state)
    .bind(&req.postcode)
    .bind(&req.marital_status)
    .bind(&req.bank_name)
    .bind(&req.bank_account_number)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Employee not found".into()))
}

// ─── Payslips ───

pub async fn get_my_payslips(
    pool: &PgPool,
    employee_id: Uuid,
) -> AppResult<Vec<MyPayslip>> {
    let payslips = sqlx::query_as::<_, MyPayslip>(
        r#"SELECT pi.id, pi.payroll_run_id,
            pr.period_year, pr.period_month, pr.period_start, pr.period_end, pr.pay_date,
            pi.basic_salary, pi.gross_salary, pi.total_allowances, pi.total_overtime,
            pi.total_bonus, pi.total_commission, pi.total_claims,
            pi.epf_employee, pi.epf_employer, pi.socso_employee, pi.socso_employer,
            pi.eis_employee, pi.eis_employer, pi.pcb_amount, pi.zakat_amount,
            pi.ptptn_amount, pi.tabung_haji_amount, pi.total_loan_deductions,
            pi.total_other_deductions, pi.unpaid_leave_deduction,
            pi.total_deductions, pi.net_salary, pi.employer_cost,
            pi.ytd_gross, pi.ytd_epf_employee, pi.ytd_pcb,
            pi.ytd_socso_employee, pi.ytd_eis_employee, pi.ytd_zakat, pi.ytd_net
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        WHERE pi.employee_id = $1
        AND pr.status::text IN ('processed', 'approved', 'paid')
        ORDER BY pr.period_year DESC, pr.period_month DESC"#,
    )
    .bind(employee_id)
    .fetch_all(pool)
    .await?;

    Ok(payslips)
}

// ─── Leave ───

pub async fn get_leave_types(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<LeaveType>> {
    let types = sqlx::query_as::<_, LeaveType>(
        "SELECT * FROM leave_types WHERE company_id = $1 AND is_active = TRUE ORDER BY name",
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;
    Ok(types)
}

pub async fn get_leave_balances(
    pool: &PgPool,
    employee_id: Uuid,
    year: i32,
) -> AppResult<Vec<LeaveBalanceWithType>> {
    let balances = sqlx::query_as::<_, LeaveBalanceWithType>(
        r#"SELECT lb.id, lb.leave_type_id, lt.name as leave_type_name, lt.is_paid,
            lb.year, lb.entitled_days, lb.taken_days, lb.pending_days, lb.carried_forward
        FROM leave_balances lb
        JOIN leave_types lt ON lb.leave_type_id = lt.id
        WHERE lb.employee_id = $1 AND lb.year = $2 AND lt.is_active = TRUE
        ORDER BY lt.name"#,
    )
    .bind(employee_id)
    .bind(year)
    .fetch_all(pool)
    .await?;
    Ok(balances)
}

pub async fn get_leave_requests(
    pool: &PgPool,
    employee_id: Uuid,
) -> AppResult<Vec<LeaveRequest>> {
    let requests = sqlx::query_as::<_, LeaveRequest>(
        r#"SELECT lr.id, lr.employee_id, lr.company_id, lr.leave_type_id,
            lr.start_date, lr.end_date, lr.days, lr.reason, lr.status,
            lr.reviewed_by, lr.reviewed_at, lr.review_notes,
            lr.attachment_url, lr.attachment_name,
            lr.created_at, lr.updated_at,
            lt.name as leave_type_name
        FROM leave_requests lr
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        WHERE lr.employee_id = $1
        ORDER BY lr.created_at DESC
        LIMIT 50"#,
    )
    .bind(employee_id)
    .fetch_all(pool)
    .await?;
    Ok(requests)
}

pub async fn create_leave_request(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    req: CreateLeaveRequest,
) -> AppResult<LeaveRequest> {
    // Verify leave type exists
    let _lt = sqlx::query_as::<_, LeaveType>(
        "SELECT * FROM leave_types WHERE id = $1 AND company_id = $2 AND is_active = TRUE",
    )
    .bind(req.leave_type_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Leave type not found".into()))?;

    let leave = sqlx::query_as::<_, LeaveRequest>(
        r#"WITH new_lr AS (
            INSERT INTO leave_requests (employee_id, company_id, leave_type_id, start_date, end_date, days, reason, attachment_url, attachment_name)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
        )
        SELECT nlr.id, nlr.employee_id, nlr.company_id, nlr.leave_type_id,
            nlr.start_date, nlr.end_date, nlr.days, nlr.reason, nlr.status,
            nlr.reviewed_by, nlr.reviewed_at, nlr.review_notes,
            nlr.attachment_url, nlr.attachment_name,
            nlr.created_at, nlr.updated_at,
            lt.name as leave_type_name
        FROM new_lr nlr
        JOIN leave_types lt ON nlr.leave_type_id = lt.id"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(req.leave_type_id)
    .bind(req.start_date)
    .bind(req.end_date)
    .bind(req.days)
    .bind(&req.reason)
    .bind(&req.attachment_url)
    .bind(&req.attachment_name)
    .fetch_one(pool)
    .await?;

    // Update pending days in balance
    let year = req.start_date.year();
    sqlx::query(
        r#"UPDATE leave_balances SET pending_days = pending_days + $3, updated_at = NOW()
        WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
    )
    .bind(employee_id)
    .bind(req.leave_type_id)
    .bind(req.days)
    .bind(year)
    .execute(pool)
    .await?;

    // Notify admins about new leave request
    let emp_name: Option<(String,)> = sqlx::query_as("SELECT full_name FROM employees WHERE id = $1")
        .bind(employee_id)
        .fetch_optional(pool)
        .await?;
    let name = emp_name.map(|e| e.0).unwrap_or_default();
    let _ = notification_service::notify_admins(
        pool,
        company_id,
        "leave_submitted",
        "New Leave Request",
        &format!("{} has submitted a {} leave request ({} days)", name, leave.leave_type_name.as_deref().unwrap_or(""), leave.days),
        Some("leave_request"),
        Some(leave.id),
    ).await;

    Ok(leave)
}

pub async fn cancel_leave_request(
    pool: &PgPool,
    employee_id: Uuid,
    request_id: Uuid,
) -> AppResult<()> {
    let lr = sqlx::query_as::<_, LeaveRequest>(
        r#"SELECT lr.id, lr.employee_id, lr.company_id, lr.leave_type_id,
            lr.start_date, lr.end_date, lr.days, lr.reason, lr.status,
            lr.reviewed_by, lr.reviewed_at, lr.review_notes,
            lr.attachment_url, lr.attachment_name,
            lr.created_at, lr.updated_at,
            lt.name as leave_type_name
        FROM leave_requests lr
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        WHERE lr.id = $1 AND lr.employee_id = $2 AND lr.status = 'pending'"#,
    )
    .bind(request_id)
    .bind(employee_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Leave request not found or cannot be cancelled".into()))?;

    sqlx::query("UPDATE leave_requests SET status = 'cancelled', updated_at = NOW() WHERE id = $1")
        .bind(request_id)
        .execute(pool)
        .await?;

    // Reduce pending days
    let year = lr.start_date.year();
    sqlx::query(
        r#"UPDATE leave_balances SET pending_days = GREATEST(pending_days - $3, 0), updated_at = NOW()
        WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
    )
    .bind(employee_id)
    .bind(lr.leave_type_id)
    .bind(lr.days)
    .bind(year)
    .execute(pool)
    .await?;

    Ok(())
}

// ─── Claims ───

pub async fn get_claims(
    pool: &PgPool,
    employee_id: Uuid,
    status: Option<&str>,
) -> AppResult<Vec<Claim>> {
    let claims = sqlx::query_as::<_, Claim>(
        r#"SELECT * FROM claims
        WHERE employee_id = $1
        AND ($2::text IS NULL OR status = $2)
        ORDER BY created_at DESC
        LIMIT 100"#,
    )
    .bind(employee_id)
    .bind(status)
    .fetch_all(pool)
    .await?;
    Ok(claims)
}

pub async fn create_claim(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    req: CreateClaimRequest,
) -> AppResult<Claim> {
    let claim = sqlx::query_as::<_, Claim>(
        r#"INSERT INTO claims (employee_id, company_id, title, description, amount, category, receipt_url, receipt_file_name, expense_date)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING *"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(&req.title)
    .bind(&req.description)
    .bind(req.amount)
    .bind(&req.category)
    .bind(&req.receipt_url)
    .bind(&req.receipt_file_name)
    .bind(req.expense_date)
    .fetch_one(pool)
    .await?;
    Ok(claim)
}

pub async fn submit_claim(
    pool: &PgPool,
    employee_id: Uuid,
    claim_id: Uuid,
) -> AppResult<Claim> {
    let claim = sqlx::query_as::<_, Claim>(
        r#"UPDATE claims SET status = 'pending', submitted_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND employee_id = $2 AND status = 'draft'
        RETURNING *"#,
    )
    .bind(claim_id)
    .bind(employee_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Claim not found or already submitted".into()))?;

    // Notify admins about submitted claim
    let emp_name: Option<(String,)> = sqlx::query_as("SELECT full_name FROM employees WHERE id = $1")
        .bind(employee_id)
        .fetch_optional(pool)
        .await?;
    let name = emp_name.map(|e| e.0).unwrap_or_default();
    let _ = notification_service::notify_admins(
        pool,
        claim.company_id,
        "claim_submitted",
        "New Claim Submitted",
        &format!("{} submitted a claim: \"{}\" (RM {:.2})", name, claim.title, claim.amount as f64 / 100.0),
        Some("claim"),
        Some(claim.id),
    ).await;

    Ok(claim)
}

pub async fn delete_claim(
    pool: &PgPool,
    employee_id: Uuid,
    claim_id: Uuid,
) -> AppResult<()> {
    let rows = sqlx::query(
        "DELETE FROM claims WHERE id = $1 AND employee_id = $2 AND status = 'draft'",
    )
    .bind(claim_id)
    .bind(employee_id)
    .execute(pool)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::BadRequest("Claim not found or cannot be deleted".into()));
    }
    Ok(())
}

// ─── Overtime Applications ───

pub async fn get_overtime_applications(
    pool: &PgPool,
    employee_id: Uuid,
) -> AppResult<Vec<OvertimeApplication>> {
    let apps = sqlx::query_as::<_, OvertimeApplication>(
        r#"SELECT * FROM overtime_applications
        WHERE employee_id = $1
        ORDER BY created_at DESC
        LIMIT 50"#,
    )
    .bind(employee_id)
    .fetch_all(pool)
    .await?;
    Ok(apps)
}

pub async fn create_overtime_application(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    req: CreateOvertimeRequest,
) -> AppResult<OvertimeApplication> {
    let start_time = chrono::NaiveTime::parse_from_str(&req.start_time, "%H:%M")
        .map_err(|_| AppError::BadRequest("Invalid start_time format, expected HH:MM".into()))?;
    let end_time = chrono::NaiveTime::parse_from_str(&req.end_time, "%H:%M")
        .map_err(|_| AppError::BadRequest("Invalid end_time format, expected HH:MM".into()))?;

    let ot_type = req.ot_type.as_deref().unwrap_or("normal");
    if !["normal", "rest_day", "public_holiday"].contains(&ot_type) {
        return Err(AppError::BadRequest("Invalid ot_type".into()));
    }

    let app = sqlx::query_as::<_, OvertimeApplication>(
        r#"INSERT INTO overtime_applications
            (employee_id, company_id, ot_date, start_time, end_time, hours, ot_type, reason)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING *"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(req.ot_date)
    .bind(start_time)
    .bind(end_time)
    .bind(req.hours)
    .bind(ot_type)
    .bind(&req.reason)
    .fetch_one(pool)
    .await?;

    // Notify admins
    let emp_name: Option<(String,)> = sqlx::query_as("SELECT full_name FROM employees WHERE id = $1")
        .bind(employee_id)
        .fetch_optional(pool)
        .await?;
    let name = emp_name.map(|e| e.0).unwrap_or_default();
    let _ = notification_service::notify_admins(
        pool,
        company_id,
        "overtime_submitted",
        "New Overtime Application",
        &format!(
            "{} submitted an overtime application for {} ({} hours, {})",
            name, req.ot_date, req.hours, ot_type.replace('_', " ")
        ),
        Some("overtime"),
        Some(app.id),
    )
    .await;

    Ok(app)
}

pub async fn cancel_overtime_application(
    pool: &PgPool,
    employee_id: Uuid,
    id: Uuid,
) -> AppResult<()> {
    let rows = sqlx::query(
        "UPDATE overtime_applications SET status = 'cancelled', updated_at = NOW() WHERE id = $1 AND employee_id = $2 AND status = 'pending'",
    )
    .bind(id)
    .bind(employee_id)
    .execute(pool)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::BadRequest("OT application not found or cannot be cancelled".into()));
    }
    Ok(())
}

use chrono::Datelike;
use rust_decimal::prelude::*;

// ─── Leave Proration ───

/// Calculate prorated leave days for a mid-year joiner.
/// Formula: default_days × remaining_months / 12, rounded to nearest 0.5
pub fn calculate_prorated_days(
    default_days: rust_decimal::Decimal,
    date_joined: chrono::NaiveDate,
    year: i32,
) -> rust_decimal::Decimal {
    let join_year = date_joined.year();
    if join_year < year {
        // Joined before this year — full entitlement
        return default_days;
    }
    if join_year > year {
        // Joined after this year — no entitlement
        return rust_decimal::Decimal::ZERO;
    }
    // Joined this year: remaining months including the joining month
    let join_month = date_joined.month() as i32;
    let remaining = (12 - join_month + 1).max(0);
    let prorated = default_days * rust_decimal::Decimal::from(remaining) / rust_decimal::Decimal::from(12);
    // Round to nearest 0.5
    let doubled = prorated * rust_decimal::Decimal::from(2);
    let rounded = doubled.round_dp(0);
    rounded / rust_decimal::Decimal::from(2)
}

/// Initialize leave balances for an employee for a given year.
/// Prorates based on date_joined if it's a mid-year join.
pub async fn initialize_leave_balances(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    date_joined: chrono::NaiveDate,
    year: i32,
) -> AppResult<Vec<LeaveBalance>> {
    let leave_types = sqlx::query_as::<_, LeaveType>(
        "SELECT * FROM leave_types WHERE company_id = $1 AND is_active = TRUE",
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;

    let mut balances = vec![];
    for lt in &leave_types {
        let entitled = calculate_prorated_days(lt.default_days, date_joined, year);
        let balance = sqlx::query_as::<_, LeaveBalance>(
            r#"INSERT INTO leave_balances (employee_id, leave_type_id, year, entitled_days)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (employee_id, leave_type_id, year) DO NOTHING
            RETURNING *"#,
        )
        .bind(employee_id)
        .bind(lt.id)
        .bind(year)
        .bind(entitled)
        .fetch_optional(pool)
        .await?;

        if let Some(b) = balance {
            balances.push(b);
        }
    }
    Ok(balances)
}

// ─── Year-End Carry-Forward ───

/// Process year-end carry-forward for all employees in a company.
/// Returns the count of balances created/updated.
pub async fn process_year_end_carry_forward(
    pool: &PgPool,
    company_id: Uuid,
    from_year: i32,
    to_year: i32,
) -> AppResult<i32> {
    let leave_types = sqlx::query_as::<_, LeaveType>(
        "SELECT * FROM leave_types WHERE company_id = $1 AND is_active = TRUE",
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;

    let employees: Vec<(Uuid, chrono::NaiveDate)> = sqlx::query_as(
        "SELECT id, date_joined FROM employees WHERE company_id = $1 AND is_active = TRUE AND deleted_at IS NULL",
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;

    let mut count = 0;
    for (emp_id, date_joined) in &employees {
        for lt in &leave_types {
            // Get current year balance
            let balance: Option<(rust_decimal::Decimal, rust_decimal::Decimal, rust_decimal::Decimal, rust_decimal::Decimal)> =
                sqlx::query_as(
                    r#"SELECT entitled_days, taken_days, pending_days, carried_forward
                    FROM leave_balances
                    WHERE employee_id = $1 AND leave_type_id = $2 AND year = $3"#,
                )
                .bind(emp_id)
                .bind(lt.id)
                .bind(from_year)
                .fetch_optional(pool)
                .await?;

            let carry = if let Some((entitled, taken, pending, cf)) = balance {
                let remaining = entitled + cf - taken - pending;
                let remaining = remaining.max(rust_decimal::Decimal::ZERO);
                if lt.max_carry_forward > rust_decimal::Decimal::ZERO {
                    remaining.min(lt.max_carry_forward)
                } else {
                    rust_decimal::Decimal::ZERO
                }
            } else {
                rust_decimal::Decimal::ZERO
            };

            // Calculate entitled for next year (prorate for mid-year joiners)
            let entitled = calculate_prorated_days(lt.default_days, *date_joined, to_year);

            // UPSERT next year balance
            sqlx::query(
                r#"INSERT INTO leave_balances (employee_id, leave_type_id, year, entitled_days, carried_forward)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (employee_id, leave_type_id, year)
                DO UPDATE SET carried_forward = $5, entitled_days = $4, updated_at = NOW()"#,
            )
            .bind(emp_id)
            .bind(lt.id)
            .bind(to_year)
            .bind(entitled)
            .bind(carry)
            .execute(pool)
            .await?;

            count += 1;
        }
    }

    Ok(count)
}

// ─── Team Calendar ───

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct TeamLeaveEntry {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub department: Option<String>,
    pub leave_type_name: String,
    pub start_date: chrono::NaiveDate,
    pub end_date: chrono::NaiveDate,
    pub days: rust_decimal::Decimal,
    pub status: String,
}

pub async fn get_team_calendar(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    year: i32,
    month: u32,
) -> AppResult<Vec<TeamLeaveEntry>> {
    let period_start = chrono::NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| AppError::BadRequest("Invalid month".into()))?;
    let period_end = if month == 12 {
        chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        chrono::NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .and_then(|d| d.pred_opt())
    .ok_or_else(|| AppError::BadRequest("Invalid month".into()))?;

    // Find all teammates via team_members table (employees sharing any team with this employee)
    let entries = sqlx::query_as::<_, TeamLeaveEntry>(
        r#"SELECT DISTINCT lr.id, lr.employee_id, e.full_name as employee_name,
            e.department, lt.name as leave_type_name,
            lr.start_date, lr.end_date, lr.days, lr.status::text
        FROM leave_requests lr
        JOIN employees e ON lr.employee_id = e.id
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        WHERE e.company_id = $1
          AND lr.status = 'approved'
          AND lr.start_date <= $4
          AND lr.end_date >= $3
          AND (lr.employee_id = $2
               OR lr.employee_id IN (
                  SELECT tm2.employee_id FROM team_members tm2
                  WHERE tm2.team_id IN (
                      SELECT tm1.team_id FROM team_members tm1
                      WHERE tm1.employee_id = $2
                  )
              ))
        ORDER BY lr.start_date, e.full_name"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .bind(period_start)
    .bind(period_end)
    .fetch_all(pool)
    .await?;

    Ok(entries)
}
