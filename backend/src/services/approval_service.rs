use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::config::AppConfig;
use crate::core::error::{AppError, AppResult};
use crate::models::portal::{Claim, LeaveRequest, OvertimeApplication};
use crate::services::calendar_service;
use crate::services::email_service;
use crate::services::notification_service;
use crate::services::settings_service;

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
