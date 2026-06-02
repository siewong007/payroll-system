//! Overtime admin CRUD + approval/reject workflow.

use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::portal::{CreateOvertimeRequest, OvertimeApplication, UpdateOvertimeRequest};
use crate::repositories::reads::approvals as approval_reads;
use crate::repositories::{
    employees as employee_repo, leave_balances, leave_types, overtime_applications,
    payroll_entries, users as user_repo,
};
use crate::services::audit_service::AuditRequestMeta;
use crate::services::notification_service;
use crate::services::settings_service;

use super::common::{ensure_employee_in_company, parse_overtime_times, validate_overtime_type};

pub use crate::repositories::reads::approvals::OvertimeWithEmployee;

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

    let overtime = overtime_applications::insert(
        pool,
        employee_id,
        company_id,
        req.ot_date,
        start_time,
        end_time,
        req.hours,
        ot_type,
        req.reason,
    )
    .await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
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
    let current = overtime_applications::get_pending(pool, ot_id, company_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("OT application not found or cannot be edited".into())
        })?;

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

    let updated = overtime_applications::update_full(
        pool,
        ot_id,
        company_id,
        req.employee_id,
        req.ot_date,
        start_time,
        end_time,
        req.hours,
        ot_type,
        req.reason,
    )
    .await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
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
    let current = overtime_applications::get_cancelled(pool, ot_id, company_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("OT application not found or cannot be deleted".into())
        })?;

    overtime_applications::delete(pool, ot_id, company_id).await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
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

    let current = overtime_applications::get_cancellable(&mut *tx, ot_id, company_id)
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
        let processed = payroll_entries::exists_processed_overtime(
            &mut *tx,
            current.employee_id,
            company_id,
            period_year,
            period_month,
            &description,
        )
        .await?;

        if processed {
            return Err(AppError::BadRequest(
                "Approved OT already included in processed payroll and cannot be cancelled".into(),
            ));
        }

        payroll_entries::delete_unprocessed_overtime(
            &mut *tx,
            current.employee_id,
            company_id,
            period_year,
            period_month,
            &description,
        )
        .await?;

        if current.ot_type == "public_holiday" {
            leave_balances::subtract_entitled_replacement(
                &mut *tx,
                current.employee_id,
                period_year,
                company_id,
            )
            .await?;
        }
    }

    let cancelled = overtime_applications::set_cancelled(&mut *tx, ot_id, company_id).await?;

    tx.commit().await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
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

pub async fn get_overtime_with_employee_by_id(
    pool: &PgPool,
    company_id: Uuid,
    overtime_id: Uuid,
) -> AppResult<OvertimeWithEmployee> {
    approval_reads::overtime_with_employee_by_id(pool, company_id, overtime_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Overtime application not found".into()))
}

pub async fn get_pending_overtime(
    pool: &PgPool,
    company_id: Uuid,
    status: Option<&str>,
) -> AppResult<Vec<OvertimeWithEmployee>> {
    approval_reads::list_pending_overtime(pool, company_id, status).await
}

pub async fn approve_overtime(
    pool: &PgPool,
    company_id: Uuid,
    ot_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<OvertimeApplication> {
    let ot = overtime_applications::set_approved(pool, ot_id, company_id, reviewer_id, notes)
        .await?
        .ok_or_else(|| AppError::BadRequest("OT application not found or not pending".into()))?;

    // Get employee hourly rate
    if let Some((hourly_rate, basic_salary)) =
        employee_repo::overtime_rate_basis(pool, ot.employee_id).await?
    {
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
        let base_hourly =
            hourly_rate.unwrap_or_else(|| basic_salary / working_days / effective_hours);

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

        let _ = payroll_entries::insert_overtime(
            pool,
            Uuid::now_v7(),
            ot.employee_id,
            company_id,
            period_year,
            period_month,
            &format!(
                "OT {} - {} ({} {}h @ {:.1}x)",
                ot.ot_date,
                ot.ot_type.replace('_', " "),
                ot.start_time.format("%H:%M"),
                ot.hours,
                multiplier
            ),
            ot_amount,
            ot.hours,
            ot_rate,
            reviewer_id,
        )
        .await;
    }

    // Replacement leave: if OT was on a public holiday, grant 1 day replacement leave
    if ot.ot_type == "public_holiday" {
        // Find or create system "Replacement Leave" type for this company
        let rl_type_id = leave_types::upsert_replacement_leave(pool, company_id).await?;

        // UPSERT leave balance: increment entitled_days by 1
        let year = ot.ot_date.year();
        leave_balances::upsert_entitled_replacement(pool, ot.employee_id, rl_type_id, year).await?;
    }

    // Notify employee
    let employee_user = user_repo::active_id_for_employee(pool, ot.employee_id).await?;

    if let Some(user_id) = employee_user {
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
        Some(company_id),
        Some(reviewer_id),
        "approve_overtime",
        "overtime",
        Some(ot.id),
        None,
        Some(serde_json::to_value(&ot).unwrap_or_default()),
        Some(&format!(
            "Approved overtime for employee {}",
            ot.employee_id
        )),
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
    let ot = overtime_applications::set_rejected(pool, ot_id, company_id, reviewer_id, notes)
        .await?
        .ok_or_else(|| AppError::BadRequest("OT application not found or not pending".into()))?;

    let employee_user = user_repo::active_id_for_employee(pool, ot.employee_id).await?;

    if let Some(user_id) = employee_user {
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
        Some(company_id),
        Some(reviewer_id),
        "reject_overtime",
        "overtime",
        Some(ot.id),
        None,
        Some(serde_json::to_value(&ot).unwrap_or_default()),
        Some(&format!(
            "Rejected overtime for employee {}",
            ot.employee_id
        )),
        audit_meta,
    )
    .await;

    Ok(ot)
}
