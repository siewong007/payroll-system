//! Leave admin CRUD + approval/reject workflow.

use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::config::AppConfig;
use crate::core::error::{AppError, AppResult};
use crate::models::portal::{CreateLeaveRequest, LeaveRequest, UpdateLeaveRequest};
use crate::services::audit_service::AuditRequestMeta;
use crate::services::calendar_service;
use crate::services::email_service;
use crate::services::notification_service;

use super::common::{ensure_employee_in_company, ensure_leave_type_in_company};

// ─── Admin CRUD ───

pub async fn create_leave_request_admin(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    req: CreateLeaveRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
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

    let _ = crate::services::audit_service::log_action_with_metadata(
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
        audit_meta,
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
    audit_meta: Option<&AuditRequestMeta>,
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

    let employee_id = req.employee_id.unwrap_or(current.employee_id);
    ensure_employee_in_company(pool, company_id, employee_id).await?;
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
    .bind(employee_id)
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
        SET employee_id = $3,
            leave_type_id = $4,
            start_date = $5,
            end_date = $6,
            days = $7,
            reason = CASE WHEN $8::text IS NULL THEN reason ELSE NULLIF($8, '') END,
            attachment_url = CASE WHEN $9::text IS NULL THEN attachment_url ELSE NULLIF($9, '') END,
            attachment_name = CASE WHEN $10::text IS NULL THEN attachment_name ELSE NULLIF($10, '') END,
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *,
            (SELECT name FROM leave_types WHERE id = leave_type_id) as leave_type_name"#,
    )
    .bind(request_id)
    .bind(company_id)
    .bind(employee_id)
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

    let _ = crate::services::audit_service::log_action_with_metadata(
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
        audit_meta,
    )
    .await;

    Ok(updated)
}

pub async fn delete_leave_request_admin(
    pool: &PgPool,
    company_id: Uuid,
    request_id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let current = sqlx::query_as::<_, LeaveRequest>(
        r#"SELECT lr.id, lr.employee_id, lr.company_id, lr.leave_type_id,
            lr.start_date, lr.end_date, lr.days, lr.reason, lr.status,
            lr.reviewed_by, lr.reviewed_at, lr.review_notes,
            lr.attachment_url, lr.attachment_name,
            lr.created_at, lr.updated_at,
            lt.name as leave_type_name
        FROM leave_requests lr
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        WHERE lr.id = $1 AND lr.company_id = $2 AND lr.status = 'cancelled'"#,
    )
    .bind(request_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Leave request not found or cannot be deleted".into()))?;

    sqlx::query("DELETE FROM leave_requests WHERE id = $1 AND company_id = $2")
        .bind(request_id)
        .bind(company_id)
        .execute(pool)
        .await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
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
        audit_meta,
    )
    .await;

    Ok(())
}

pub async fn cancel_leave_request_admin(
    pool: &PgPool,
    company_id: Uuid,
    request_id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
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
        WHERE lr.id = $1
          AND lr.company_id = $2
          AND lr.status IN ('pending', 'approved', 'rejected')"#,
    )
    .bind(request_id)
    .bind(company_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::BadRequest("Leave request not found or cannot be cancelled".into()))?;

    if current.status == "approved" {
        let is_paid: Option<bool> =
            sqlx::query_scalar("SELECT is_paid FROM leave_types WHERE id = $1")
                .bind(current.leave_type_id)
                .fetch_optional(&mut *tx)
                .await?;

        if is_paid == Some(false) {
            let description = format!(
                "Unpaid leave: {} to {}%",
                current.start_date, current.end_date
            );
            let processed = sqlx::query_scalar::<_, bool>(
                r#"SELECT EXISTS(
                    SELECT 1 FROM payroll_entries
                    WHERE employee_id = $1
                      AND company_id = $2
                      AND item_type = 'unpaid_leave'
                      AND description LIKE $3
                      AND is_processed = TRUE
                )"#,
            )
            .bind(current.employee_id)
            .bind(company_id)
            .bind(&description)
            .fetch_one(&mut *tx)
            .await?;

            if processed {
                return Err(AppError::BadRequest(
                    "Approved unpaid leave already included in processed payroll and cannot be cancelled".into(),
                ));
            }

            sqlx::query(
                r#"DELETE FROM payroll_entries
                WHERE employee_id = $1
                  AND company_id = $2
                  AND item_type = 'unpaid_leave'
                  AND description LIKE $3
                  AND is_processed = FALSE"#,
            )
            .bind(current.employee_id)
            .bind(company_id)
            .bind(&description)
            .execute(&mut *tx)
            .await?;
        }
    }

    let year = current.start_date.year();
    if current.status == "pending" {
        sqlx::query(
            r#"UPDATE leave_balances
            SET pending_days = GREATEST(pending_days - $3, 0), updated_at = NOW()
            WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
        )
        .bind(current.employee_id)
        .bind(current.leave_type_id)
        .bind(current.days)
        .bind(year)
        .execute(&mut *tx)
        .await?;
    } else if current.status == "approved" {
        sqlx::query(
            r#"UPDATE leave_balances
            SET taken_days = GREATEST(taken_days - $3, 0), updated_at = NOW()
            WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
        )
        .bind(current.employee_id)
        .bind(current.leave_type_id)
        .bind(current.days)
        .bind(year)
        .execute(&mut *tx)
        .await?;
    }

    let cancelled = sqlx::query_as::<_, LeaveRequest>(
        r#"UPDATE leave_requests
        SET status = 'cancelled', updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *, (SELECT name FROM leave_types WHERE id = leave_type_id) as leave_type_name"#,
    )
    .bind(request_id)
    .bind(company_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(actor_id),
        "cancel_leave_request_admin",
        "leave_request",
        Some(cancelled.id),
        Some(serde_json::to_value(&current).unwrap_or_default()),
        Some(serde_json::to_value(&cancelled).unwrap_or_default()),
        Some(&format!(
            "Cancelled leave request for employee {}",
            cancelled.employee_id
        )),
        audit_meta,
    )
    .await;

    Ok(cancelled)
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
    audit_meta: Option<&AuditRequestMeta>,
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
                            (id, employee_id, company_id, period_year, period_month, category, item_type, description, amount, created_by)
                        VALUES ($1, $2, $3, $4, $5, 'deduction', 'unpaid_leave', $6, $7, $8)"#,
                    )
                    .bind(Uuid::new_v4())
                    .bind(lr.employee_id)
                    .bind(company_id)
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
    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(reviewer_id),
        "approve_leave",
        "leave_request",
        Some(lr.id),
        None,
        Some(serde_json::to_value(&lr).unwrap_or_default()),
        Some(&format!("Approved leave for employee {}", lr.employee_id)),
        audit_meta,
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
    audit_meta: Option<&AuditRequestMeta>,
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
    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(reviewer_id),
        "reject_leave",
        "leave_request",
        Some(lr.id),
        None,
        Some(serde_json::to_value(&lr).unwrap_or_default()),
        Some(&format!("Rejected leave for employee {}", lr.employee_id)),
        audit_meta,
    )
    .await;

    Ok(lr)
}
