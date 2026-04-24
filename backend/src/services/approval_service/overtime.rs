//! Overtime admin CRUD + approval/reject workflow.

use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::portal::{CreateOvertimeRequest, OvertimeApplication, UpdateOvertimeRequest};
use crate::services::audit_service::AuditRequestMeta;
use crate::services::notification_service;
use crate::services::settings_service;

use super::common::{ensure_employee_in_company, parse_overtime_times, validate_overtime_type};

pub async fn create_overtime_admin(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    req: CreateOvertimeRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
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

    let _ = crate::services::audit_service::log_action_with_metadata(
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
        audit_meta,
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
    audit_meta: Option<&AuditRequestMeta>,
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

    if let Some(employee_id) = req.employee_id {
        ensure_employee_in_company(pool, company_id, employee_id).await?;
    }

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
        SET employee_id = COALESCE($3, employee_id),
            ot_date = COALESCE($4, ot_date),
            start_time = $5,
            end_time = $6,
            hours = COALESCE($7, hours),
            ot_type = $8,
            reason = CASE WHEN $9::text IS NULL THEN reason ELSE NULLIF($9, '') END,
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *"#,
    )
    .bind(ot_id)
    .bind(company_id)
    .bind(req.employee_id)
    .bind(req.ot_date)
    .bind(start_time)
    .bind(end_time)
    .bind(req.hours)
    .bind(ot_type)
    .bind(&req.reason)
    .fetch_one(pool)
    .await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
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
        audit_meta,
    )
    .await;

    Ok(updated)
}

pub async fn delete_overtime_admin(
    pool: &PgPool,
    company_id: Uuid,
    ot_id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let current = sqlx::query_as::<_, OvertimeApplication>(
        r#"SELECT * FROM overtime_applications
        WHERE id = $1 AND company_id = $2
        AND status = 'cancelled'"#,
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

    let _ = crate::services::audit_service::log_action_with_metadata(
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
        audit_meta,
    )
    .await;

    Ok(())
}

pub async fn cancel_overtime_admin(
    pool: &PgPool,
    company_id: Uuid,
    ot_id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<OvertimeApplication> {
    let mut tx = pool.begin().await?;

    let current = sqlx::query_as::<_, OvertimeApplication>(
        r#"SELECT * FROM overtime_applications
        WHERE id = $1
          AND company_id = $2
          AND status IN ('pending', 'approved', 'rejected')"#,
    )
    .bind(ot_id)
    .bind(company_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest("OT application not found or cannot be cancelled".into())
    })?;

    if current.status == "approved" {
        let description = format!(
            "OT {} - {}%",
            current.ot_date,
            current.ot_type.replace('_', " ")
        );
        let period_year = current.ot_date.year();
        let period_month = current.ot_date.month() as i32;
        let processed = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS(
                SELECT 1 FROM payroll_entries
                WHERE employee_id = $1
                  AND company_id = $2
                  AND period_year = $3
                  AND period_month = $4
                  AND item_type = 'overtime'
                  AND description LIKE $5
                  AND is_processed = TRUE
            )"#,
        )
        .bind(current.employee_id)
        .bind(company_id)
        .bind(period_year)
        .bind(period_month)
        .bind(&description)
        .fetch_one(&mut *tx)
        .await?;

        if processed {
            return Err(AppError::BadRequest(
                "Approved OT already included in processed payroll and cannot be cancelled".into(),
            ));
        }

        sqlx::query(
            r#"DELETE FROM payroll_entries
            WHERE employee_id = $1
              AND company_id = $2
              AND period_year = $3
              AND period_month = $4
              AND item_type = 'overtime'
              AND description LIKE $5
              AND is_processed = FALSE"#,
        )
        .bind(current.employee_id)
        .bind(company_id)
        .bind(period_year)
        .bind(period_month)
        .bind(&description)
        .execute(&mut *tx)
        .await?;

        if current.ot_type == "public_holiday" {
            sqlx::query(
                r#"UPDATE leave_balances lb
                SET entitled_days = GREATEST(lb.entitled_days - 1, 0), updated_at = NOW()
                FROM leave_types lt
                WHERE lb.leave_type_id = lt.id
                  AND lb.employee_id = $1
                  AND lb.year = $2
                  AND lt.company_id = $3
                  AND lt.name = 'Replacement Leave'"#,
            )
            .bind(current.employee_id)
            .bind(period_year)
            .bind(company_id)
            .execute(&mut *tx)
            .await?;
        }
    }

    let cancelled = sqlx::query_as::<_, OvertimeApplication>(
        r#"UPDATE overtime_applications
        SET status = 'cancelled', updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *"#,
    )
    .bind(ot_id)
    .bind(company_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(actor_id),
        "cancel_overtime_admin",
        "overtime",
        Some(cancelled.id),
        Some(serde_json::to_value(&current).unwrap_or_default()),
        Some(serde_json::to_value(&cancelled).unwrap_or_default()),
        Some(&format!(
            "Cancelled overtime application for employee {}",
            cancelled.employee_id
        )),
        audit_meta,
    )
    .await;

    Ok(cancelled)
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

pub async fn get_overtime_with_employee_by_id(
    pool: &PgPool,
    company_id: Uuid,
    overtime_id: Uuid,
) -> AppResult<OvertimeWithEmployee> {
    sqlx::query_as::<_, OvertimeWithEmployee>(
        r#"SELECT oa.*,
            e.full_name as employee_name,
            e.employee_number
        FROM overtime_applications oa
        JOIN employees e ON oa.employee_id = e.id
        WHERE oa.id = $1 AND oa.company_id = $2"#,
    )
    .bind(overtime_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Overtime application not found".into()))
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
    audit_meta: Option<&AuditRequestMeta>,
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
                (id, employee_id, company_id, period_year, period_month, category, item_type,
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

    // Audit Log
    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(reviewer_id),
        "approve_overtime",
        "overtime",
        Some(ot.id),
        None,
        Some(serde_json::to_value(&ot).unwrap_or_default()),
        Some(&format!("Approved overtime for employee {}", ot.employee_id)),
        audit_meta,
    )
    .await;

    Ok(ot)
}

pub async fn reject_overtime(
    pool: &PgPool,
    company_id: Uuid,
    ot_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
    audit_meta: Option<&AuditRequestMeta>,
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

    // Audit Log
    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(reviewer_id),
        "reject_overtime",
        "overtime",
        Some(ot.id),
        None,
        Some(serde_json::to_value(&ot).unwrap_or_default()),
        Some(&format!("Rejected overtime for employee {}", ot.employee_id)),
        audit_meta,
    )
    .await;

    Ok(ot)
}
