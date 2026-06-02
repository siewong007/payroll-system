use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::employee::Employee;
use crate::models::portal::*;
use crate::services::notification_service;

// ─── Profile ───

pub async fn get_my_profile(pool: &PgPool, employee_id: Uuid) -> AppResult<Employee> {
    sqlx::query_as!(
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
        employee_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Employee profile not found".into()))
}

// ─── Payslips ───

pub async fn get_my_payslips(pool: &PgPool, employee_id: Uuid) -> AppResult<Vec<MyPayslip>> {
    let payslips = sqlx::query_as!(
        MyPayslip,
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
        AND pr.status::text IN ('approved', 'paid')
        ORDER BY pr.period_year DESC, pr.period_month DESC"#,
        employee_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(payslips)
}

// ─── Leave ───

pub async fn get_leave_types(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<LeaveType>> {
    let types = sqlx::query_as!(
        LeaveType,
        r#"SELECT id, company_id, name, description, default_days, is_paid, is_active,
            max_carry_forward, carry_forward_expiry_months, is_system, created_at, updated_at
        FROM leave_types WHERE company_id = $1 AND is_active = TRUE ORDER BY name"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(types)
}

pub async fn get_leave_balances(
    pool: &PgPool,
    employee_id: Uuid,
    year: i32,
) -> AppResult<Vec<LeaveBalanceWithType>> {
    let balances = sqlx::query_as!(
        LeaveBalanceWithType,
        r#"SELECT lb.id, lb.leave_type_id, lt.name AS leave_type_name, lt.is_paid,
            lb.year, lb.entitled_days, lb.taken_days, lb.pending_days, lb.carried_forward
        FROM leave_balances lb
        JOIN leave_types lt ON lb.leave_type_id = lt.id
        WHERE lb.employee_id = $1 AND lb.year = $2 AND lt.is_active = TRUE
        ORDER BY lt.name"#,
        employee_id,
        year,
    )
    .fetch_all(pool)
    .await?;
    Ok(balances)
}

pub async fn get_leave_requests(pool: &PgPool, employee_id: Uuid) -> AppResult<Vec<LeaveRequest>> {
    let requests = sqlx::query_as!(
        LeaveRequest,
        r#"SELECT lr.id, lr.employee_id, lr.company_id, lr.leave_type_id,
            lr.start_date, lr.end_date, lr.days, lr.reason, lr.status,
            lr.reviewed_by, lr.reviewed_at, lr.review_notes,
            lr.attachment_url, lr.attachment_name,
            lr.created_at, lr.updated_at,
            lt.name AS "leave_type_name?"
        FROM leave_requests lr
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        WHERE lr.employee_id = $1
        ORDER BY lr.created_at DESC
        LIMIT 50"#,
        employee_id,
    )
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
    let _lt = sqlx::query_scalar!(
        "SELECT id FROM leave_types WHERE id = $1 AND company_id = $2 AND is_active = TRUE",
        req.leave_type_id,
        company_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Leave type not found".into()))?;

    let leave = sqlx::query_as!(
        LeaveRequest,
        r#"WITH new_lr AS (
            INSERT INTO leave_requests (employee_id, company_id, leave_type_id, start_date, end_date, days, reason, attachment_url, attachment_name)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
        )
        SELECT nlr.id AS "id!", nlr.employee_id AS "employee_id!", nlr.company_id AS "company_id!", nlr.leave_type_id AS "leave_type_id!",
            nlr.start_date AS "start_date!", nlr.end_date AS "end_date!", nlr.days AS "days!", nlr.reason, nlr.status AS "status!",
            nlr.reviewed_by, nlr.reviewed_at, nlr.review_notes,
            nlr.attachment_url, nlr.attachment_name,
            nlr.created_at AS "created_at!", nlr.updated_at AS "updated_at!",
            lt.name AS "leave_type_name?"
        FROM new_lr nlr
        JOIN leave_types lt ON nlr.leave_type_id = lt.id"#,
        employee_id,
        company_id,
        req.leave_type_id,
        req.start_date,
        req.end_date,
        req.days,
        req.reason,
        req.attachment_url,
        req.attachment_name,
    )
    .fetch_one(pool)
    .await?;

    // Update pending days in balance
    let year = req.start_date.year();
    sqlx::query!(
        r#"UPDATE leave_balances SET pending_days = pending_days + $3, updated_at = NOW()
        WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
        employee_id,
        req.leave_type_id,
        req.days,
        year,
    )
    .execute(pool)
    .await?;

    // Notify admins about new leave request
    let name = sqlx::query_scalar!("SELECT full_name FROM employees WHERE id = $1", employee_id)
        .fetch_optional(pool)
        .await?
        .unwrap_or_default();
    let _ = notification_service::notify_admins(
        pool,
        company_id,
        "leave_submitted",
        "New Leave Request",
        &format!(
            "{} has submitted a {} leave request ({} days)",
            name,
            leave.leave_type_name.as_deref().unwrap_or(""),
            leave.days
        ),
        Some("leave_request"),
        Some(leave.id),
    )
    .await;

    Ok(leave)
}

pub async fn cancel_leave_request(
    pool: &PgPool,
    employee_id: Uuid,
    request_id: Uuid,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    let lr = sqlx::query_as!(
        LeaveRequest,
        r#"SELECT lr.id, lr.employee_id, lr.company_id, lr.leave_type_id,
            lr.start_date, lr.end_date, lr.days, lr.reason, lr.status,
            lr.reviewed_by, lr.reviewed_at, lr.review_notes,
            lr.attachment_url, lr.attachment_name,
            lr.created_at, lr.updated_at,
            lt.name AS "leave_type_name?"
        FROM leave_requests lr
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        WHERE lr.id = $1
          AND lr.employee_id = $2
          AND lr.status IN ('pending', 'approved', 'rejected')"#,
        request_id,
        employee_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::BadRequest("Leave request not found or cannot be cancelled".into()))?;

    if lr.status == "approved" {
        let is_paid = sqlx::query_scalar!(
            "SELECT is_paid FROM leave_types WHERE id = $1",
            lr.leave_type_id,
        )
        .fetch_optional(&mut *tx)
        .await?;

        if is_paid == Some(false) {
            let description = format!("Unpaid leave: {} to {}%", lr.start_date, lr.end_date);
            let processed = sqlx::query_scalar!(
                r#"SELECT EXISTS(
                    SELECT 1 FROM payroll_entries
                    WHERE employee_id = $1
                      AND company_id = $2
                      AND item_type = 'unpaid_leave'
                      AND description LIKE $3
                      AND is_processed = TRUE
                ) AS "exists!""#,
                lr.employee_id,
                lr.company_id,
                description,
            )
            .fetch_one(&mut *tx)
            .await?;

            if processed {
                return Err(AppError::BadRequest(
                    "Approved unpaid leave already included in processed payroll and cannot be cancelled".into(),
                ));
            }

            sqlx::query!(
                r#"DELETE FROM payroll_entries
                WHERE employee_id = $1
                  AND company_id = $2
                  AND item_type = 'unpaid_leave'
                  AND description LIKE $3
                  AND is_processed = FALSE"#,
                lr.employee_id,
                lr.company_id,
                description,
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    sqlx::query!(
        "UPDATE leave_requests SET status = 'cancelled', updated_at = NOW() WHERE id = $1",
        request_id,
    )
    .execute(&mut *tx)
    .await?;

    let year = lr.start_date.year();
    if lr.status == "pending" {
        sqlx::query!(
            r#"UPDATE leave_balances SET pending_days = GREATEST(pending_days - $3, 0), updated_at = NOW()
            WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
            employee_id,
            lr.leave_type_id,
            lr.days,
            year,
        )
        .execute(&mut *tx)
        .await?;
    } else if lr.status == "approved" {
        sqlx::query!(
            r#"UPDATE leave_balances SET taken_days = GREATEST(taken_days - $3, 0), updated_at = NOW()
            WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
            employee_id,
            lr.leave_type_id,
            lr.days,
            year,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}

pub async fn delete_leave_request(
    pool: &PgPool,
    employee_id: Uuid,
    request_id: Uuid,
) -> AppResult<()> {
    let rows = sqlx::query!(
        "DELETE FROM leave_requests WHERE id = $1 AND employee_id = $2 AND status = 'cancelled'",
        request_id,
        employee_id,
    )
    .execute(pool)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::BadRequest(
            "Leave request not found or cannot be deleted".into(),
        ));
    }

    Ok(())
}

// ─── Claims ───

pub async fn get_claims(
    pool: &PgPool,
    employee_id: Uuid,
    status: Option<&str>,
) -> AppResult<Vec<Claim>> {
    let claims = sqlx::query_as!(
        Claim,
        r#"SELECT * FROM claims
        WHERE employee_id = $1
        AND ($2::text IS NULL OR status = $2)
        ORDER BY created_at DESC
        LIMIT 100"#,
        employee_id,
        status,
    )
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
    let claim = sqlx::query_as!(
        Claim,
        r#"INSERT INTO claims (employee_id, company_id, title, description, amount, category, receipt_url, receipt_file_name, expense_date)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING *"#,
        employee_id,
        company_id,
        req.title,
        req.description,
        req.amount,
        req.category,
        req.receipt_url,
        req.receipt_file_name,
        req.expense_date,
    )
    .fetch_one(pool)
    .await?;
    Ok(claim)
}

pub async fn submit_claim(pool: &PgPool, employee_id: Uuid, claim_id: Uuid) -> AppResult<Claim> {
    let claim = sqlx::query_as!(
        Claim,
        r#"UPDATE claims SET status = 'pending', submitted_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND employee_id = $2 AND status = 'draft'
        RETURNING *"#,
        claim_id,
        employee_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Claim not found or already submitted".into()))?;

    // Notify admins about submitted claim
    let name = sqlx::query_scalar!("SELECT full_name FROM employees WHERE id = $1", employee_id)
        .fetch_optional(pool)
        .await?
        .unwrap_or_default();
    let _ = notification_service::notify_admins(
        pool,
        claim.company_id,
        "claim_submitted",
        "New Claim Submitted",
        &format!(
            "{} submitted a claim: \"{}\" (RM {:.2})",
            name,
            claim.title,
            claim.amount as f64 / 100.0
        ),
        Some("claim"),
        Some(claim.id),
    )
    .await;

    Ok(claim)
}

pub async fn cancel_claim(pool: &PgPool, employee_id: Uuid, claim_id: Uuid) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    let claim = sqlx::query_as!(
        Claim,
        r#"SELECT * FROM claims
        WHERE id = $1
          AND employee_id = $2
          AND status IN ('pending', 'approved', 'rejected')"#,
        claim_id,
        employee_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::BadRequest("Claim not found or cannot be cancelled".into()))?;

    if claim.status == "approved" {
        let staged_at = claim.reviewed_at.unwrap_or_else(chrono::Utc::now);
        let description = format!("Claim: {}", claim.title);
        let staged_year = staged_at.year();
        let staged_month = staged_at.month() as i32;
        let processed = sqlx::query_scalar!(
            r#"SELECT EXISTS(
                SELECT 1 FROM payroll_entries
                WHERE employee_id = $1
                  AND company_id = $2
                  AND period_year = $3
                  AND period_month = $4
                  AND item_type = 'claim_reimbursement'
                  AND description = $5
                  AND amount = $6
                  AND is_processed = TRUE
            ) AS "exists!""#,
            claim.employee_id,
            claim.company_id,
            staged_year,
            staged_month,
            description,
            claim.amount,
        )
        .fetch_one(&mut *tx)
        .await?;

        if processed {
            return Err(AppError::BadRequest(
                "Approved claim already included in processed payroll and cannot be cancelled"
                    .into(),
            ));
        }

        sqlx::query!(
            r#"DELETE FROM payroll_entries
            WHERE employee_id = $1
              AND company_id = $2
              AND period_year = $3
              AND period_month = $4
              AND item_type = 'claim_reimbursement'
              AND description = $5
              AND amount = $6
              AND is_processed = FALSE"#,
            claim.employee_id,
            claim.company_id,
            staged_year,
            staged_month,
            description,
            claim.amount,
        )
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query!(
        "UPDATE claims SET status = 'cancelled', updated_at = NOW() WHERE id = $1",
        claim_id,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

pub async fn delete_claim(pool: &PgPool, employee_id: Uuid, claim_id: Uuid) -> AppResult<()> {
    let rows = sqlx::query!(
        "DELETE FROM claims WHERE id = $1 AND employee_id = $2 AND status IN ('draft', 'cancelled')",
        claim_id,
        employee_id,
    )
    .execute(pool)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::BadRequest(
            "Claim not found or cannot be deleted".into(),
        ));
    }
    Ok(())
}

// ─── Overtime Applications ───

pub async fn get_overtime_applications(
    pool: &PgPool,
    employee_id: Uuid,
) -> AppResult<Vec<OvertimeApplication>> {
    let apps = sqlx::query_as!(
        OvertimeApplication,
        r#"SELECT * FROM overtime_applications
        WHERE employee_id = $1
        ORDER BY created_at DESC
        LIMIT 50"#,
        employee_id,
    )
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

    let app = sqlx::query_as!(
        OvertimeApplication,
        r#"INSERT INTO overtime_applications
            (employee_id, company_id, ot_date, start_time, end_time, hours, ot_type, reason)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING *"#,
        employee_id,
        company_id,
        req.ot_date,
        start_time,
        end_time,
        req.hours,
        ot_type,
        req.reason,
    )
    .fetch_one(pool)
    .await?;

    // Notify admins
    let name = sqlx::query_scalar!("SELECT full_name FROM employees WHERE id = $1", employee_id)
        .fetch_optional(pool)
        .await?
        .unwrap_or_default();
    let _ = notification_service::notify_admins(
        pool,
        company_id,
        "overtime_submitted",
        "New Overtime Application",
        &format!(
            "{} submitted an overtime application for {} ({} hours, {})",
            name,
            req.ot_date,
            req.hours,
            ot_type.replace('_', " ")
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
    let mut tx = pool.begin().await?;

    let app = sqlx::query_as!(
        OvertimeApplication,
        r#"SELECT * FROM overtime_applications
        WHERE id = $1
          AND employee_id = $2
          AND status IN ('pending', 'approved', 'rejected')"#,
        id,
        employee_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest("OT application not found or cannot be cancelled".into())
    })?;

    if app.status == "approved" {
        let description = format!("OT {} - {}%", app.ot_date, app.ot_type.replace('_', " "));
        let period_year = app.ot_date.year();
        let period_month = app.ot_date.month() as i32;
        let processed = sqlx::query_scalar!(
            r#"SELECT EXISTS(
                SELECT 1 FROM payroll_entries
                WHERE employee_id = $1
                  AND company_id = $2
                  AND period_year = $3
                  AND period_month = $4
                  AND item_type = 'overtime'
                  AND description LIKE $5
                  AND is_processed = TRUE
            ) AS "exists!""#,
            app.employee_id,
            app.company_id,
            period_year,
            period_month,
            description,
        )
        .fetch_one(&mut *tx)
        .await?;

        if processed {
            return Err(AppError::BadRequest(
                "Approved OT already included in processed payroll and cannot be cancelled".into(),
            ));
        }

        sqlx::query!(
            r#"DELETE FROM payroll_entries
            WHERE employee_id = $1
              AND company_id = $2
              AND period_year = $3
              AND period_month = $4
              AND item_type = 'overtime'
              AND description LIKE $5
              AND is_processed = FALSE"#,
            app.employee_id,
            app.company_id,
            period_year,
            period_month,
            description,
        )
        .execute(&mut *tx)
        .await?;

        if app.ot_type == "public_holiday" {
            sqlx::query!(
                r#"UPDATE leave_balances lb
                SET entitled_days = GREATEST(lb.entitled_days - 1, 0), updated_at = NOW()
                FROM leave_types lt
                WHERE lb.leave_type_id = lt.id
                  AND lb.employee_id = $1
                  AND lb.year = $2
                  AND lt.company_id = $3
                  AND lt.name = 'Replacement Leave'"#,
                app.employee_id,
                period_year,
                app.company_id,
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    sqlx::query!(
        "UPDATE overtime_applications SET status = 'cancelled', updated_at = NOW() WHERE id = $1",
        id,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

pub async fn delete_overtime_application(
    pool: &PgPool,
    employee_id: Uuid,
    id: Uuid,
) -> AppResult<()> {
    let rows = sqlx::query!(
        "DELETE FROM overtime_applications WHERE id = $1 AND employee_id = $2 AND status = 'cancelled'",
        id,
        employee_id,
    )
    .execute(pool)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::BadRequest(
            "OT application not found or cannot be deleted".into(),
        ));
    }

    Ok(())
}

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
    let prorated =
        default_days * rust_decimal::Decimal::from(remaining) / rust_decimal::Decimal::from(12);
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
    let leave_types = sqlx::query_as!(
        LeaveType,
        r#"SELECT id, company_id, name, description, default_days, is_paid, is_active,
            max_carry_forward, carry_forward_expiry_months, is_system, created_at, updated_at
        FROM leave_types WHERE company_id = $1 AND is_active = TRUE"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let mut balances = vec![];
    for lt in &leave_types {
        let entitled = calculate_prorated_days(lt.default_days, date_joined, year);
        let balance = sqlx::query_as!(
            LeaveBalance,
            r#"INSERT INTO leave_balances (employee_id, leave_type_id, year, entitled_days)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (employee_id, leave_type_id, year) DO NOTHING
            RETURNING *"#,
            employee_id,
            lt.id,
            year,
            entitled,
        )
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
    let leave_types = sqlx::query_as!(
        LeaveType,
        r#"SELECT id, company_id, name, description, default_days, is_paid, is_active,
            max_carry_forward, carry_forward_expiry_months, is_system, created_at, updated_at
        FROM leave_types WHERE company_id = $1 AND is_active = TRUE"#,
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let employees = sqlx::query!(
        "SELECT id, date_joined FROM employees WHERE company_id = $1 AND is_active = TRUE AND deleted_at IS NULL",
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let mut count = 0;
    for emp in &employees {
        for lt in &leave_types {
            // Get current year balance
            let balance = sqlx::query!(
                r#"SELECT entitled_days, taken_days, pending_days, carried_forward
                    FROM leave_balances
                    WHERE employee_id = $1 AND leave_type_id = $2 AND year = $3"#,
                emp.id,
                lt.id,
                from_year,
            )
            .fetch_optional(pool)
            .await?;

            let carry = if let Some(b) = balance {
                let remaining = b.entitled_days + b.carried_forward - b.taken_days - b.pending_days;
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
            let entitled = calculate_prorated_days(lt.default_days, emp.date_joined, to_year);

            // UPSERT next year balance
            sqlx::query!(
                r#"INSERT INTO leave_balances (employee_id, leave_type_id, year, entitled_days, carried_forward)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (employee_id, leave_type_id, year)
                DO UPDATE SET carried_forward = $5, entitled_days = $4, updated_at = NOW()"#,
                emp.id,
                lt.id,
                to_year,
                entitled,
                carry,
            )
            .execute(pool)
            .await?;

            count += 1;
        }
    }

    Ok(count)
}

// ─── Team Calendar ───

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
    let entries = sqlx::query_as!(
        TeamLeaveEntry,
        r#"SELECT DISTINCT lr.id, lr.employee_id, e.full_name AS employee_name,
            e.department, lt.name AS leave_type_name,
            lr.start_date, lr.end_date, lr.days, lr.status
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
        company_id,
        employee_id,
        period_start,
        period_end,
    )
    .fetch_all(pool)
    .await?;

    Ok(entries)
}
