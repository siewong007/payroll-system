use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::config::AppConfig;
use crate::core::error::{AppError, AppResult};
use crate::models::portal::{
    Claim, CreateClaimRequest, CreateLeaveRequest, CreateOvertimeRequest, LeaveRequest,
    OvertimeApplication, UpdateClaimRequest, UpdateLeaveRequest, UpdateOvertimeRequest,
};
use crate::services::calendar_service;
use crate::services::email_service;
use crate::services::notification_service;
use crate::services::settings_service;

fn ensure_positive_amount(amount: i64) -> AppResult<()> {
    if amount <= 0 {
        return Err(AppError::BadRequest(
            "Amount must be greater than zero".into(),
        ));
    }
    Ok(())
}

async fn ensure_employee_in_company(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(
            SELECT 1 FROM employees
            WHERE id = $1 AND company_id = $2 AND deleted_at IS NULL
        )"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .fetch_one(pool)
    .await?;

    if !exists {
        return Err(AppError::NotFound(
            "Employee not found in the active company".into(),
        ));
    }

    Ok(())
}

async fn ensure_leave_type_in_company(
    pool: &PgPool,
    company_id: Uuid,
    leave_type_id: Uuid,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(
            SELECT 1 FROM leave_types
            WHERE id = $1 AND company_id = $2 AND is_active = TRUE
        )"#,
    )
    .bind(leave_type_id)
    .bind(company_id)
    .fetch_one(pool)
    .await?;

    if !exists {
        return Err(AppError::NotFound("Leave type not found".into()));
    }

    Ok(())
}

fn validate_overtime_type(ot_type: &str) -> AppResult<()> {
    if !["normal", "rest_day", "public_holiday"].contains(&ot_type) {
        return Err(AppError::BadRequest("Invalid ot_type".into()));
    }
    Ok(())
}

fn parse_overtime_times(
    start_time: &str,
    end_time: &str,
) -> AppResult<(chrono::NaiveTime, chrono::NaiveTime)> {
    let start = chrono::NaiveTime::parse_from_str(start_time, "%H:%M")
        .map_err(|_| AppError::BadRequest("Invalid start_time format, expected HH:MM".into()))?;
    let end = chrono::NaiveTime::parse_from_str(end_time, "%H:%M")
        .map_err(|_| AppError::BadRequest("Invalid end_time format, expected HH:MM".into()))?;
    Ok((start, end))
}

// ─── Admin CRUD ───

pub async fn create_leave_request_admin(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    req: CreateLeaveRequest,
    actor_id: Uuid,
) -> AppResult<LeaveRequest> {
    ensure_employee_in_company(pool, company_id, employee_id).await?;
    ensure_leave_type_in_company(pool, company_id, req.leave_type_id).await?;

    let mut tx = pool.begin().await?;

    let leave = sqlx::query_as::<_, LeaveRequest>(
        r#"WITH new_lr AS (
            INSERT INTO leave_requests
                (employee_id, company_id, leave_type_id, start_date, end_date, days, reason, attachment_url, attachment_name)
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
    .fetch_one(&mut *tx)
    .await?;

    let year = req.start_date.year();
    let rows = sqlx::query(
        r#"UPDATE leave_balances
        SET pending_days = pending_days + $3, updated_at = NOW()
        WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
    )
    .bind(employee_id)
    .bind(req.leave_type_id)
    .bind(req.days)
    .bind(year)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::BadRequest(
            "Leave balance not initialized for the selected employee/year".into(),
        ));
    }

    tx.commit().await?;

    let _ = crate::services::audit_service::log_action(
        pool,
        Some(actor_id),
        "create_leave_request_admin",
        "leave_request",
        Some(leave.id),
        None,
        Some(serde_json::to_value(&leave).unwrap_or_default()),
        Some(&format!(
            "Created leave request for employee {}",
            leave.employee_id
        )),
    )
    .await;

    Ok(leave)
}

pub async fn update_leave_request_admin(
    pool: &PgPool,
    company_id: Uuid,
    request_id: Uuid,
    req: UpdateLeaveRequest,
    actor_id: Uuid,
) -> AppResult<LeaveRequest> {
    let mut tx = pool.begin().await?;

    let current = sqlx::query_as::<_, LeaveRequest>(
        r#"SELECT lr.id, lr.employee_id, lr.company_id, lr.leave_type_id,
            lr.start_date, lr.end_date, lr.days, lr.reason, lr.status,
            lr.reviewed_by, lr.reviewed_at, lr.review_notes,
            lr.attachment_url, lr.attachment_name,
            lr.created_at, lr.updated_at,
            lt.name as leave_type_name
        FROM leave_requests lr
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        WHERE lr.id = $1 AND lr.company_id = $2 AND lr.status = 'pending'"#,
    )
    .bind(request_id)
    .bind(company_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::BadRequest("Leave request not found or cannot be edited".into()))?;

    let leave_type_id = req.leave_type_id.unwrap_or(current.leave_type_id);
    ensure_leave_type_in_company(pool, company_id, leave_type_id).await?;
    let start_date = req.start_date.unwrap_or(current.start_date);
    let end_date = req.end_date.unwrap_or(current.end_date);
    let days = req.days.unwrap_or(current.days);

    let old_year = current.start_date.year();
    let new_year = start_date.year();

    sqlx::query(
        r#"UPDATE leave_balances
        SET pending_days = GREATEST(pending_days - $3, 0), updated_at = NOW()
        WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
    )
    .bind(current.employee_id)
    .bind(current.leave_type_id)
    .bind(current.days)
    .bind(old_year)
    .execute(&mut *tx)
    .await?;

    let add_rows = sqlx::query(
        r#"UPDATE leave_balances
        SET pending_days = pending_days + $3, updated_at = NOW()
        WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
    )
    .bind(current.employee_id)
    .bind(leave_type_id)
    .bind(days)
    .bind(new_year)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if add_rows == 0 {
        return Err(AppError::BadRequest(
            "Leave balance not initialized for the selected employee/year".into(),
        ));
    }

    let updated = sqlx::query_as::<_, LeaveRequest>(
        r#"UPDATE leave_requests
        SET leave_type_id = $3,
            start_date = $4,
            end_date = $5,
            days = $6,
            reason = CASE WHEN $7::text IS NULL THEN reason ELSE NULLIF($7, '') END,
            attachment_url = CASE WHEN $8::text IS NULL THEN attachment_url ELSE NULLIF($8, '') END,
            attachment_name = CASE WHEN $9::text IS NULL THEN attachment_name ELSE NULLIF($9, '') END,
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *,
            (SELECT name FROM leave_types WHERE id = leave_type_id) as leave_type_name"#,
    )
    .bind(request_id)
    .bind(company_id)
    .bind(leave_type_id)
    .bind(start_date)
    .bind(end_date)
    .bind(days)
    .bind(&req.reason)
    .bind(&req.attachment_url)
    .bind(&req.attachment_name)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    let _ = crate::services::audit_service::log_action(
        pool,
        Some(actor_id),
        "update_leave_request_admin",
        "leave_request",
        Some(updated.id),
        Some(serde_json::to_value(&current).unwrap_or_default()),
        Some(serde_json::to_value(&updated).unwrap_or_default()),
        Some(&format!(
            "Updated leave request for employee {}",
            updated.employee_id
        )),
    )
    .await;

    Ok(updated)
}

pub async fn delete_leave_request_admin(
    pool: &PgPool,
    company_id: Uuid,
    request_id: Uuid,
    actor_id: Uuid,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    let current = sqlx::query_as::<_, LeaveRequest>(
        r#"SELECT lr.id, lr.employee_id, lr.company_id, lr.leave_type_id,
            lr.start_date, lr.end_date, lr.days, lr.reason, lr.status,
            lr.reviewed_by, lr.reviewed_at, lr.review_notes,
            lr.attachment_url, lr.attachment_name,
            lr.created_at, lr.updated_at,
            lt.name as leave_type_name
        FROM leave_requests lr
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        WHERE lr.id = $1 AND lr.company_id = $2 AND lr.status = 'pending'"#,
    )
    .bind(request_id)
    .bind(company_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::BadRequest("Leave request not found or cannot be deleted".into()))?;

    sqlx::query(
        r#"UPDATE leave_balances
        SET pending_days = GREATEST(pending_days - $3, 0), updated_at = NOW()
        WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
    )
    .bind(current.employee_id)
    .bind(current.leave_type_id)
    .bind(current.days)
    .bind(current.start_date.year())
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM leave_requests WHERE id = $1 AND company_id = $2")
        .bind(request_id)
        .bind(company_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    let _ = crate::services::audit_service::log_action(
        pool,
        Some(actor_id),
        "delete_leave_request_admin",
        "leave_request",
        Some(current.id),
        Some(serde_json::to_value(&current).unwrap_or_default()),
        None,
        Some(&format!(
            "Deleted leave request for employee {}",
            current.employee_id
        )),
    )
    .await;

    Ok(())
}

pub async fn create_claim_admin(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    req: CreateClaimRequest,
    actor_id: Uuid,
) -> AppResult<Claim> {
    ensure_employee_in_company(pool, company_id, employee_id).await?;
    ensure_positive_amount(req.amount)?;

    let claim = sqlx::query_as::<_, Claim>(
        r#"INSERT INTO claims
            (employee_id, company_id, title, description, amount, category, receipt_url, receipt_file_name, expense_date, status, submitted_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending', NOW())
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

    let _ = crate::services::audit_service::log_action(
        pool,
        Some(actor_id),
        "create_claim_admin",
        "claim",
        Some(claim.id),
        None,
        Some(serde_json::to_value(&claim).unwrap_or_default()),
        Some(&format!("Created claim for employee {}", claim.employee_id)),
    )
    .await;

    Ok(claim)
}

pub async fn update_claim_admin(
    pool: &PgPool,
    company_id: Uuid,
    claim_id: Uuid,
    req: UpdateClaimRequest,
    actor_id: Uuid,
) -> AppResult<Claim> {
    let current = sqlx::query_as::<_, Claim>(
        r#"SELECT * FROM claims
        WHERE id = $1 AND company_id = $2
        AND status IN ('draft', 'pending', 'rejected')"#,
    )
    .bind(claim_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Claim not found or cannot be edited".into()))?;

    if let Some(amount) = req.amount {
        ensure_positive_amount(amount)?;
    }

    let updated = sqlx::query_as::<_, Claim>(
        r#"UPDATE claims
        SET title = COALESCE($3, title),
            description = CASE WHEN $4::text IS NULL THEN description ELSE NULLIF($4, '') END,
            amount = COALESCE($5, amount),
            category = CASE WHEN $6::text IS NULL THEN category ELSE NULLIF($6, '') END,
            receipt_url = CASE WHEN $7::text IS NULL THEN receipt_url ELSE NULLIF($7, '') END,
            receipt_file_name = CASE WHEN $8::text IS NULL THEN receipt_file_name ELSE NULLIF($8, '') END,
            expense_date = COALESCE($9, expense_date),
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *"#,
    )
    .bind(claim_id)
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

    let _ = crate::services::audit_service::log_action(
        pool,
        Some(actor_id),
        "update_claim_admin",
        "claim",
        Some(updated.id),
        Some(serde_json::to_value(&current).unwrap_or_default()),
        Some(serde_json::to_value(&updated).unwrap_or_default()),
        Some(&format!(
            "Updated claim for employee {}",
            updated.employee_id
        )),
    )
    .await;

    Ok(updated)
}

pub async fn delete_claim_admin(
    pool: &PgPool,
    company_id: Uuid,
    claim_id: Uuid,
    actor_id: Uuid,
) -> AppResult<()> {
    let current = sqlx::query_as::<_, Claim>(
        r#"SELECT * FROM claims
        WHERE id = $1 AND company_id = $2
        AND status IN ('draft', 'pending', 'rejected')"#,
    )
    .bind(claim_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Claim not found or cannot be deleted".into()))?;

    sqlx::query("DELETE FROM claims WHERE id = $1 AND company_id = $2")
        .bind(claim_id)
        .bind(company_id)
        .execute(pool)
        .await?;

    let _ = crate::services::audit_service::log_action(
        pool,
        Some(actor_id),
        "delete_claim_admin",
        "claim",
        Some(current.id),
        Some(serde_json::to_value(&current).unwrap_or_default()),
        None,
        Some(&format!(
            "Deleted claim for employee {}",
            current.employee_id
        )),
    )
    .await;

    Ok(())
}

pub async fn create_overtime_admin(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    req: CreateOvertimeRequest,
    actor_id: Uuid,
) -> AppResult<OvertimeApplication> {
    ensure_employee_in_company(pool, company_id, employee_id).await?;
    let ot_type = req.ot_type.as_deref().unwrap_or("normal");
    validate_overtime_type(ot_type)?;
    let (start_time, end_time) = parse_overtime_times(&req.start_time, &req.end_time)?;

    let overtime = sqlx::query_as::<_, OvertimeApplication>(
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

    let _ = crate::services::audit_service::log_action(
        pool,
        Some(actor_id),
        "create_overtime_admin",
        "overtime",
        Some(overtime.id),
        None,
        Some(serde_json::to_value(&overtime).unwrap_or_default()),
        Some(&format!(
            "Created overtime application for employee {}",
            overtime.employee_id
        )),
    )
    .await;

    Ok(overtime)
}

pub async fn update_overtime_admin(
    pool: &PgPool,
    company_id: Uuid,
    ot_id: Uuid,
    req: UpdateOvertimeRequest,
    actor_id: Uuid,
) -> AppResult<OvertimeApplication> {
    let current = sqlx::query_as::<_, OvertimeApplication>(
        r#"SELECT * FROM overtime_applications
        WHERE id = $1 AND company_id = $2 AND status = 'pending'"#,
    )
    .bind(ot_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("OT application not found or cannot be edited".into()))?;

    let start_time_raw = req
        .start_time
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(|| current.start_time.format("%H:%M").to_string());
    let end_time_raw = req
        .end_time
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(|| current.end_time.format("%H:%M").to_string());
    let (start_time, end_time) = parse_overtime_times(&start_time_raw, &end_time_raw)?;

    let ot_type = req.ot_type.as_deref().unwrap_or(&current.ot_type);
    validate_overtime_type(ot_type)?;

    let updated = sqlx::query_as::<_, OvertimeApplication>(
        r#"UPDATE overtime_applications
        SET ot_date = COALESCE($3, ot_date),
            start_time = $4,
            end_time = $5,
            hours = COALESCE($6, hours),
            ot_type = $7,
            reason = CASE WHEN $8::text IS NULL THEN reason ELSE NULLIF($8, '') END,
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *"#,
    )
    .bind(ot_id)
    .bind(company_id)
    .bind(req.ot_date)
    .bind(start_time)
    .bind(end_time)
    .bind(req.hours)
    .bind(ot_type)
    .bind(&req.reason)
    .fetch_one(pool)
    .await?;

    let _ = crate::services::audit_service::log_action(
        pool,
        Some(actor_id),
        "update_overtime_admin",
        "overtime",
        Some(updated.id),
        Some(serde_json::to_value(&current).unwrap_or_default()),
        Some(serde_json::to_value(&updated).unwrap_or_default()),
        Some(&format!(
            "Updated overtime application for employee {}",
            updated.employee_id
        )),
    )
    .await;

    Ok(updated)
}

pub async fn delete_overtime_admin(
    pool: &PgPool,
    company_id: Uuid,
    ot_id: Uuid,
    actor_id: Uuid,
) -> AppResult<()> {
    let current = sqlx::query_as::<_, OvertimeApplication>(
        r#"SELECT * FROM overtime_applications
        WHERE id = $1 AND company_id = $2
        AND status IN ('pending', 'rejected', 'cancelled')"#,
    )
    .bind(ot_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("OT application not found or cannot be deleted".into()))?;

    sqlx::query("DELETE FROM overtime_applications WHERE id = $1 AND company_id = $2")
        .bind(ot_id)
        .bind(company_id)
        .execute(pool)
        .await?;

    let _ = crate::services::audit_service::log_action(
        pool,
        Some(actor_id),
        "delete_overtime_admin",
        "overtime",
        Some(current.id),
        Some(serde_json::to_value(&current).unwrap_or_default()),
        None,
        Some(&format!(
            "Deleted overtime application for employee {}",
            current.employee_id
        )),
    )
    .await;

    Ok(())
}

// ─── Leave Approval ───

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct LeaveRequestWithEmployee {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub company_id: Uuid,
    pub leave_type_id: Uuid,
    pub start_date: chrono::NaiveDate,
    pub end_date: chrono::NaiveDate,
    pub days: rust_decimal::Decimal,
    pub reason: Option<String>,
    pub status: String,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub review_notes: Option<String>,
    pub attachment_url: Option<String>,
    pub attachment_name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub leave_type_name: Option<String>,
    pub employee_name: Option<String>,
    pub employee_number: Option<String>,
}

pub async fn get_pending_leave_requests(
    pool: &PgPool,
    company_id: Uuid,
    status: Option<&str>,
) -> AppResult<Vec<LeaveRequestWithEmployee>> {
    let requests = sqlx::query_as::<_, LeaveRequestWithEmployee>(
        r#"SELECT lr.id, lr.employee_id, lr.company_id, lr.leave_type_id,
            lr.start_date, lr.end_date, lr.days, lr.reason, lr.status,
            lr.reviewed_by, lr.reviewed_at, lr.review_notes,
            lr.attachment_url, lr.attachment_name,
            lr.created_at, lr.updated_at,
            lt.name as leave_type_name,
            e.full_name as employee_name,
            e.employee_number
        FROM leave_requests lr
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        JOIN employees e ON lr.employee_id = e.id
        WHERE lr.company_id = $1
        AND ($2::text IS NULL OR lr.status = $2)
        ORDER BY lr.created_at DESC
        LIMIT 100"#,
    )
    .bind(company_id)
    .bind(status)
    .fetch_all(pool)
    .await?;
    Ok(requests)
}

pub async fn approve_leave(
    pool: &PgPool,
    config: &AppConfig,
    company_id: Uuid,
    request_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
) -> AppResult<LeaveRequest> {
    let lr = sqlx::query_as::<_, LeaveRequest>(
        r#"UPDATE leave_requests SET
            status = 'approved', reviewed_by = $3, reviewed_at = NOW(),
            review_notes = $4, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending'
        RETURNING *, (SELECT name FROM leave_types WHERE id = leave_type_id) as leave_type_name"#,
    )
    .bind(request_id)
    .bind(company_id)
    .bind(reviewer_id)
    .bind(notes)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Leave request not found or not pending".into()))?;

    // Move from pending to taken
    let year = lr.start_date.year();
    sqlx::query(
        r#"UPDATE leave_balances SET
            taken_days = taken_days + $3,
            pending_days = GREATEST(pending_days - $3, 0),
            updated_at = NOW()
        WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
    )
    .bind(lr.employee_id)
    .bind(lr.leave_type_id)
    .bind(lr.days)
    .bind(year)
    .execute(pool)
    .await?;

    // Check if this is unpaid leave — if so, auto-create payroll deduction
    let is_paid: Option<bool> = sqlx::query_scalar("SELECT is_paid FROM leave_types WHERE id = $1")
        .bind(lr.leave_type_id)
        .fetch_optional(pool)
        .await?;

    if is_paid == Some(false) {
        // Calculate deduction: (basic_salary / working_days_in_month) * unpaid_leave_working_days
        let emp_data = sqlx::query_as::<_, (i64, Uuid)>(
            "SELECT basic_salary, company_id FROM employees WHERE id = $1",
        )
        .bind(lr.employee_id)
        .fetch_optional(pool)
        .await?;

        if let Some((basic_salary, emp_company_id)) = emp_data {
            // Count working days of unpaid leave (excluding weekends/holidays)
            let unpaid_working_days = calendar_service::count_working_days_between(
                pool,
                emp_company_id,
                lr.start_date,
                lr.end_date,
            )
            .await
            .unwrap_or(
                rust_decimal::Decimal::to_string(&lr.days)
                    .parse::<i32>()
                    .unwrap_or(0),
            );

            if unpaid_working_days > 0 {
                // Get working days in the leave month for daily rate calculation
                let leave_month = lr.start_date.month();
                let leave_year = lr.start_date.year();
                let total_working_days = calendar_service::get_working_days_in_month(
                    pool,
                    emp_company_id,
                    leave_year,
                    leave_month,
                )
                .await
                .unwrap_or(22); // fallback to 22 days

                let daily_rate = if total_working_days > 0 {
                    basic_salary / total_working_days as i64
                } else {
                    0
                };
                let deduction_amount = daily_rate * unpaid_working_days as i64;

                if deduction_amount > 0 {
                    // Determine which payroll period to stage the deduction
                    let now = chrono::Utc::now();
                    let period_year = now.year();
                    let period_month = now.month() as i32;

                    let _ = sqlx::query(
                        r#"INSERT INTO payroll_entries
                            (id, employee_id, period_year, period_month, category, entry_type, description, amount, created_by)
                        VALUES ($1, $2, $3, $4, 'deduction', 'unpaid_leave', $5, $6, $7)"#,
                    )
                    .bind(Uuid::new_v4())
                    .bind(lr.employee_id)
                    .bind(period_year)
                    .bind(period_month)
                    .bind(format!(
                        "Unpaid leave: {} to {} ({} working days)",
                        lr.start_date, lr.end_date, unpaid_working_days
                    ))
                    .bind(deduction_amount)
                    .bind(reviewer_id)
                    .execute(pool)
                    .await;
                }
            }
        }
    }

    // Notify employee
    let employee_user = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM users WHERE employee_id = $1 AND is_active = TRUE",
    )
    .bind(lr.employee_id)
    .fetch_optional(pool)
    .await?;

    let is_unpaid = is_paid == Some(false);
    if let Some((user_id,)) = employee_user {
        let mut msg = format!(
            "Your {} leave from {} to {} has been approved.",
            lr.leave_type_name.as_deref().unwrap_or(""),
            lr.start_date,
            lr.end_date
        );
        if is_unpaid {
            msg.push_str(" A salary deduction will be applied in your next payroll.");
        }
        let _ = notification_service::create_notification(
            pool,
            user_id,
            company_id,
            "leave_approved",
            "Leave Request Approved",
            &msg,
            Some("leave_request"),
            Some(lr.id),
        )
        .await;
    }

    // Send approval email
    let emp_info: Option<(String, String, String)> = sqlx::query_as(
        r#"SELECT e.full_name, e.email, COALESCE(c.name, '') as company_name
        FROM employees e
        JOIN companies c ON e.company_id = c.id
        WHERE e.id = $1"#,
    )
    .bind(lr.employee_id)
    .fetch_optional(pool)
    .await?;

    if let Some((emp_name, emp_email, company_name)) = emp_info {
        let leave_type = lr.leave_type_name.as_deref().unwrap_or("Leave");
        let details = format!(
            "<strong>Type:</strong> {}<br><strong>Period:</strong> {} to {} ({} day{})",
            leave_type,
            lr.start_date,
            lr.end_date,
            lr.days,
            if lr.days == rust_decimal::Decimal::ONE {
                ""
            } else {
                "s"
            }
        );
        let extra = if is_unpaid {
            "A salary deduction will be applied in your next payroll."
        } else {
            ""
        };
        let body =
            email_service::approval_email_html(&emp_name, &company_name, "Leave", &details, extra);
        let _ = email_service::send_email(
            config,
            pool,
            company_id,
            Some(lr.employee_id),
            None,
            "leave_approved",
            &emp_email,
            &emp_name,
            &format!(
                "Leave Request Approved - {} to {}",
                lr.start_date, lr.end_date
            ),
            &body,
            reviewer_id,
        )
        .await;
    }

    // Audit Log
    let _ = crate::services::audit_service::log_action(
        pool,
        Some(reviewer_id),
        "approve_leave",
        "leave_request",
        Some(lr.id),
        None,
        Some(serde_json::to_value(&lr).unwrap_or_default()),
        Some(&format!("Approved leave for employee {}", lr.employee_id)),
    )
    .await;

    Ok(lr)
}

pub async fn reject_leave(
    pool: &PgPool,
    company_id: Uuid,
    request_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
) -> AppResult<LeaveRequest> {
    let lr = sqlx::query_as::<_, LeaveRequest>(
        r#"UPDATE leave_requests SET
            status = 'rejected', reviewed_by = $3, reviewed_at = NOW(),
            review_notes = $4, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending'
        RETURNING *, (SELECT name FROM leave_types WHERE id = leave_type_id) as leave_type_name"#,
    )
    .bind(request_id)
    .bind(company_id)
    .bind(reviewer_id)
    .bind(notes)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Leave request not found or not pending".into()))?;

    // Remove from pending
    let year = lr.start_date.year();
    sqlx::query(
        r#"UPDATE leave_balances SET
            pending_days = GREATEST(pending_days - $3, 0),
            updated_at = NOW()
        WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
    )
    .bind(lr.employee_id)
    .bind(lr.leave_type_id)
    .bind(lr.days)
    .bind(year)
    .execute(pool)
    .await?;

    // Notify employee
    let employee_user = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM users WHERE employee_id = $1 AND is_active = TRUE",
    )
    .bind(lr.employee_id)
    .fetch_optional(pool)
    .await?;

    if let Some((user_id,)) = employee_user {
        let _ = notification_service::create_notification(
            pool,
            user_id,
            company_id,
            "leave_rejected",
            "Leave Request Rejected",
            &format!(
                "Your {} leave from {} to {} has been rejected.{}",
                lr.leave_type_name.as_deref().unwrap_or(""),
                lr.start_date,
                lr.end_date,
                notes.map(|n| format!(" Reason: {}", n)).unwrap_or_default()
            ),
            Some("leave_request"),
            Some(lr.id),
        )
        .await;
    }

    // Audit Log
    let _ = crate::services::audit_service::log_action(
        pool,
        Some(reviewer_id),
        "reject_leave",
        "leave_request",
        Some(lr.id),
        None,
        Some(serde_json::to_value(&lr).unwrap_or_default()),
        Some(&format!("Rejected leave for employee {}", lr.employee_id)),
    )
    .await;

    Ok(lr)
}

// ─── Claims Approval ───

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct ClaimWithEmployee {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub company_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub amount: i64,
    pub category: Option<String>,
    pub receipt_url: Option<String>,
    pub receipt_file_name: Option<String>,
    pub expense_date: chrono::NaiveDate,
    pub status: String,
    pub submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub review_notes: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub employee_name: Option<String>,
    pub employee_number: Option<String>,
}

pub async fn get_pending_claims(
    pool: &PgPool,
    company_id: Uuid,
    status: Option<&str>,
) -> AppResult<Vec<ClaimWithEmployee>> {
    let claims = sqlx::query_as::<_, ClaimWithEmployee>(
        r#"SELECT c.*,
            e.full_name as employee_name,
            e.employee_number
        FROM claims c
        JOIN employees e ON c.employee_id = e.id
        WHERE c.company_id = $1
        AND ($2::text IS NULL OR c.status = $2)
        ORDER BY c.created_at DESC
        LIMIT 100"#,
    )
    .bind(company_id)
    .bind(status)
    .fetch_all(pool)
    .await?;
    Ok(claims)
}

pub async fn approve_claim(
    pool: &PgPool,
    config: &AppConfig,
    company_id: Uuid,
    claim_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
) -> AppResult<Claim> {
    let claim = sqlx::query_as::<_, Claim>(
        r#"UPDATE claims SET
            status = 'approved', reviewed_by = $3, reviewed_at = NOW(),
            review_notes = $4, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending'
        RETURNING *"#,
    )
    .bind(claim_id)
    .bind(company_id)
    .bind(reviewer_id)
    .bind(notes)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Claim not found or not pending".into()))?;

    // Auto-create payroll entry for the approved claim amount
    // Stage it for the current payroll period (current month)
    let now = chrono::Utc::now();
    let period_year = now.year();
    let period_month = now.month() as i32;

    let _ = sqlx::query(
        r#"INSERT INTO payroll_entries
            (id, employee_id, period_year, period_month, category, entry_type, description, amount, created_by)
        VALUES ($1, $2, $3, $4, 'earning', 'claim_reimbursement', $5, $6, $7)"#,
    )
    .bind(Uuid::new_v4())
    .bind(claim.employee_id)
    .bind(period_year)
    .bind(period_month)
    .bind(format!("Claim: {}", claim.title))
    .bind(claim.amount)
    .bind(reviewer_id)
    .execute(pool)
    .await;

    // Notify employee
    let employee_user = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM users WHERE employee_id = $1 AND is_active = TRUE",
    )
    .bind(claim.employee_id)
    .fetch_optional(pool)
    .await?;

    if let Some((user_id,)) = employee_user {
        let _ = notification_service::create_notification(
            pool,
            user_id,
            company_id,
            "claim_approved",
            "Claim Approved",
            &format!(
                "Your claim \"{}\" (RM {:.2}) has been approved and will be included in your next payroll.",
                claim.title, claim.amount as f64 / 100.0
            ),
            Some("claim"),
            Some(claim.id),
        )
        .await;
    }

    // Send approval email
    let emp_info: Option<(String, String, String)> = sqlx::query_as(
        r#"SELECT e.full_name, e.email, COALESCE(c.name, '') as company_name
        FROM employees e
        JOIN companies c ON e.company_id = c.id
        WHERE e.id = $1"#,
    )
    .bind(claim.employee_id)
    .fetch_optional(pool)
    .await?;

    if let Some((emp_name, emp_email, company_name)) = emp_info {
        let amount_rm = claim.amount as f64 / 100.0;
        let details = format!(
            "<strong>Claim:</strong> {}<br><strong>Amount:</strong> RM {:.2}<br><strong>Category:</strong> {}",
            claim.title,
            amount_rm,
            claim.category.as_deref().unwrap_or("General")
        );
        let body = email_service::approval_email_html(
            &emp_name,
            &company_name,
            "Claim",
            &details,
            "The approved amount will be included in your next payroll.",
        );
        let _ = email_service::send_email(
            config,
            pool,
            company_id,
            Some(claim.employee_id),
            None,
            "claim_approved",
            &emp_email,
            &emp_name,
            &format!("Claim Approved - {} (RM {:.2})", claim.title, amount_rm),
            &body,
            reviewer_id,
        )
        .await;
    }

    Ok(claim)
}

// ─── Overtime Approval ───

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct OvertimeWithEmployee {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub company_id: Uuid,
    pub ot_date: chrono::NaiveDate,
    pub start_time: chrono::NaiveTime,
    pub end_time: chrono::NaiveTime,
    pub hours: rust_decimal::Decimal,
    pub ot_type: String,
    pub reason: Option<String>,
    pub status: String,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub review_notes: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub employee_name: Option<String>,
    pub employee_number: Option<String>,
}

pub async fn get_pending_overtime(
    pool: &PgPool,
    company_id: Uuid,
    status: Option<&str>,
) -> AppResult<Vec<OvertimeWithEmployee>> {
    let apps = sqlx::query_as::<_, OvertimeWithEmployee>(
        r#"SELECT oa.*,
            e.full_name as employee_name,
            e.employee_number
        FROM overtime_applications oa
        JOIN employees e ON oa.employee_id = e.id
        WHERE oa.company_id = $1
        AND ($2::text IS NULL OR oa.status = $2)
        ORDER BY oa.created_at DESC
        LIMIT 100"#,
    )
    .bind(company_id)
    .bind(status)
    .fetch_all(pool)
    .await?;
    Ok(apps)
}

pub async fn approve_overtime(
    pool: &PgPool,
    company_id: Uuid,
    ot_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
) -> AppResult<OvertimeApplication> {
    let ot = sqlx::query_as::<_, OvertimeApplication>(
        r#"UPDATE overtime_applications SET
            status = 'approved', reviewed_by = $3, reviewed_at = NOW(),
            review_notes = $4, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending'
        RETURNING *"#,
    )
    .bind(ot_id)
    .bind(company_id)
    .bind(reviewer_id)
    .bind(notes)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("OT application not found or not pending".into()))?;

    // Get employee hourly rate
    let hourly_rate: Option<(Option<i64>, i64)> =
        sqlx::query_as("SELECT hourly_rate, basic_salary FROM employees WHERE id = $1")
            .bind(ot.employee_id)
            .fetch_optional(pool)
            .await?;

    if let Some((hr, basic_salary)) = hourly_rate {
        // Use hourly_rate if set, otherwise calculate from basic: basic / working_days / effective_hours
        // effective_hours_per_day excludes rest time (e.g. 8h for a 9h day with 1h lunch)
        let effective_hours: i64 =
            settings_service::get_setting(pool, company_id, "payroll", "effective_hours_per_day")
                .await
                .ok()
                .and_then(|s| s.value.as_str().and_then(|v| v.parse::<i64>().ok()))
                .unwrap_or(8);
        let working_days: i64 =
            settings_service::get_setting(pool, company_id, "payroll", "unpaid_leave_divisor")
                .await
                .ok()
                .and_then(|s| s.value.as_str().and_then(|v| v.parse::<i64>().ok()))
                .unwrap_or(26);
        let base_hourly = hr.unwrap_or_else(|| basic_salary / working_days / effective_hours);

        // Get OT multiplier from company settings
        let multiplier_key = match ot.ot_type.as_str() {
            "rest_day" => "overtime_multiplier_rest",
            "public_holiday" => "overtime_multiplier_public",
            _ => "overtime_multiplier_normal",
        };

        let multiplier: f64 =
            settings_service::get_setting(pool, company_id, "payroll", multiplier_key)
                .await
                .ok()
                .and_then(|s| s.value.as_str().and_then(|v| v.parse::<f64>().ok()))
                .unwrap_or(match ot.ot_type.as_str() {
                    "rest_day" => 2.0,
                    "public_holiday" => 3.0,
                    _ => 1.5,
                });

        let ot_hours_f64 = rust_decimal::prelude::ToPrimitive::to_f64(&ot.hours).unwrap_or(0.0);
        let ot_rate = (base_hourly as f64 * multiplier) as i64;
        let ot_amount = (ot_rate as f64 * ot_hours_f64) as i64;

        // Stage payroll entry
        let period_year = ot.ot_date.year();
        let period_month = ot.ot_date.month0() as i32 + 1;

        let _ = sqlx::query(
            r#"INSERT INTO payroll_entries
                (id, employee_id, company_id, period_year, period_month, category, entry_type,
                 description, amount, quantity, rate, is_taxable, created_by)
            VALUES ($1, $2, $3, $4, $5, 'earning', 'overtime', $6, $7, $8, $9, TRUE, $10)"#,
        )
        .bind(Uuid::new_v4())
        .bind(ot.employee_id)
        .bind(company_id)
        .bind(period_year)
        .bind(period_month)
        .bind(format!(
            "OT {} - {} ({} {}h @ {:.1}x)",
            ot.ot_date,
            ot.ot_type.replace('_', " "),
            ot.start_time.format("%H:%M"),
            ot.hours,
            multiplier
        ))
        .bind(ot_amount)
        .bind(ot.hours)
        .bind(ot_rate)
        .bind(reviewer_id)
        .execute(pool)
        .await;
    }

    // Replacement leave: if OT was on a public holiday, grant 1 day replacement leave
    if ot.ot_type == "public_holiday" {
        // Find or create system "Replacement Leave" type for this company
        let rl_type_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO leave_types (company_id, name, description, default_days, is_paid, is_system)
            VALUES ($1, 'Replacement Leave', 'Auto-granted when working on public holidays', 0, TRUE, TRUE)
            ON CONFLICT (company_id, name) DO UPDATE SET updated_at = NOW()
            RETURNING id"#,
        )
        .bind(company_id)
        .fetch_one(pool)
        .await?;

        // UPSERT leave balance: increment entitled_days by 1
        let year = ot.ot_date.year();
        sqlx::query(
            r#"INSERT INTO leave_balances (employee_id, leave_type_id, year, entitled_days)
            VALUES ($1, $2, $3, 1)
            ON CONFLICT (employee_id, leave_type_id, year)
            DO UPDATE SET entitled_days = leave_balances.entitled_days + 1, updated_at = NOW()"#,
        )
        .bind(ot.employee_id)
        .bind(rl_type_id)
        .bind(year)
        .execute(pool)
        .await?;
    }

    // Notify employee
    let employee_user = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM users WHERE employee_id = $1 AND is_active = TRUE",
    )
    .bind(ot.employee_id)
    .fetch_optional(pool)
    .await?;

    if let Some((user_id,)) = employee_user {
        let _ = notification_service::create_notification(
            pool,
            user_id,
            company_id,
            "overtime_approved",
            "Overtime Approved",
            &format!(
                "Your overtime application for {} ({} hours) has been approved and will be included in your next payroll.",
                ot.ot_date, ot.hours
            ),
            Some("overtime"),
            Some(ot.id),
        )
        .await;
    }

    Ok(ot)
}

pub async fn reject_overtime(
    pool: &PgPool,
    company_id: Uuid,
    ot_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
) -> AppResult<OvertimeApplication> {
    let ot = sqlx::query_as::<_, OvertimeApplication>(
        r#"UPDATE overtime_applications SET
            status = 'rejected', reviewed_by = $3, reviewed_at = NOW(),
            review_notes = $4, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending'
        RETURNING *"#,
    )
    .bind(ot_id)
    .bind(company_id)
    .bind(reviewer_id)
    .bind(notes)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("OT application not found or not pending".into()))?;

    let employee_user = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM users WHERE employee_id = $1 AND is_active = TRUE",
    )
    .bind(ot.employee_id)
    .fetch_optional(pool)
    .await?;

    if let Some((user_id,)) = employee_user {
        let _ = notification_service::create_notification(
            pool,
            user_id,
            company_id,
            "overtime_rejected",
            "Overtime Rejected",
            &format!(
                "Your overtime application for {} has been rejected.{}",
                ot.ot_date,
                notes.map(|n| format!(" Reason: {}", n)).unwrap_or_default()
            ),
            Some("overtime"),
            Some(ot.id),
        )
        .await;
    }

    Ok(ot)
}

pub async fn reject_claim(
    pool: &PgPool,
    company_id: Uuid,
    claim_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
) -> AppResult<Claim> {
    let claim = sqlx::query_as::<_, Claim>(
        r#"UPDATE claims SET
            status = 'rejected', reviewed_by = $3, reviewed_at = NOW(),
            review_notes = $4, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending'
        RETURNING *"#,
    )
    .bind(claim_id)
    .bind(company_id)
    .bind(reviewer_id)
    .bind(notes)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Claim not found or not pending".into()))?;

    let employee_user = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM users WHERE employee_id = $1 AND is_active = TRUE",
    )
    .bind(claim.employee_id)
    .fetch_optional(pool)
    .await?;

    if let Some((user_id,)) = employee_user {
        let _ = notification_service::create_notification(
            pool,
            user_id,
            company_id,
            "claim_rejected",
            "Claim Rejected",
            &format!(
                "Your claim \"{}\" has been rejected.{}",
                claim.title,
                notes.map(|n| format!(" Reason: {}", n)).unwrap_or_default()
            ),
            Some("claim"),
            Some(claim.id),
        )
        .await;
    }

    Ok(claim)
}
