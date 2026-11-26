//! Overtime admin CRUD + approval/reject workflow.

use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::portal::{CreateOvertimeRequest, OvertimeApplication, UpdateOvertimeRequest};
use crate::repositories::reads::approvals as approval_reads;
use crate::repositories::{
    employees as employee_repo, leave_balances, leave_types, overtime_applications,
    payroll_entries, payroll_runs, users as user_repo,
};
use crate::services::audit_service::AuditRequestMeta;
use crate::services::notification_service;
use crate::services::settings_service;

use super::common::{
    Reviewer, audit_self_approval_override, ensure_employee_in_company,
    ensure_not_creator_or_editor, ensure_not_self_approval_on, ensure_overtime_hours_within_window,
    parse_overtime_times, validate_overtime_type_for_date,
};

pub use crate::models::approval::OvertimeWithEmployee;

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
    let (start_time, end_time) = parse_overtime_times(&req.start_time, &req.end_time)?;
    ensure_overtime_hours_within_window(req.hours, start_time, end_time)?;
    validate_overtime_type_for_date(pool, company_id, req.ot_date, ot_type).await?;

    let mut tx = pool.begin().await?;
    employee_repo::lock_for_update(&mut *tx, employee_id, company_id).await?;
    if payroll_runs::has_non_draft_covering_dates(
        &mut *tx,
        company_id,
        employee_id,
        req.ot_date,
        req.ot_date,
    )
    .await?
    {
        return Err(AppError::BadRequest(
            "Overtime cannot be created for a date covered by a submitted or finalized payroll run"
                .into(),
        ));
    }
    if overtime_applications::has_overlapping_window(
        &mut *tx,
        employee_id,
        req.ot_date,
        start_time,
        end_time,
        None,
    )
    .await?
    {
        return Err(AppError::BadRequest(
            "This employee already has an overlapping overtime application on this date".into(),
        ));
    }

    let overtime = overtime_applications::insert(
        &mut *tx,
        employee_id,
        company_id,
        req.ot_date,
        start_time,
        end_time,
        req.hours,
        ot_type,
        req.reason,
        actor_id,
    )
    .await?;
    tx.commit().await?;

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
    let effective_employee_id = req.employee_id.unwrap_or(current.employee_id);
    let effective_ot_date = req.ot_date.unwrap_or(current.ot_date);

    // Validate the EFFECTIVE values, not just the supplied ones: `update_full`
    // COALESCEs, so validating only what the request carried would let an edit
    // keep out-of-range hours and shrink the window around them. The
    // consequence is intended — an edit that touches only `reason` on a legacy
    // out-of-range row fails until the hours are corrected.
    let hours = req.hours.unwrap_or(current.hours);
    ensure_overtime_hours_within_window(hours, start_time, end_time)?;
    validate_overtime_type_for_date(pool, company_id, effective_ot_date, ot_type).await?;

    let mut tx = pool.begin().await?;
    let (first_emp, second_emp) = if current.employee_id <= effective_employee_id {
        (current.employee_id, effective_employee_id)
    } else {
        (effective_employee_id, current.employee_id)
    };
    employee_repo::lock_for_update(&mut *tx, first_emp, company_id).await?;
    if second_emp != first_emp {
        employee_repo::lock_for_update(&mut *tx, second_emp, company_id).await?;
    }

    let locked = overtime_applications::get_pending_locked(&mut *tx, ot_id, company_id)
        .await?
        .ok_or_else(|| {
            AppError::Conflict(
                "This overtime application changed state while it was being updated. Reload and try again.".into(),
            )
        })?;
    if (
        locked.employee_id,
        locked.ot_date,
        locked.start_time,
        locked.end_time,
        locked.ot_type.as_str(),
        locked.hours,
    ) != (
        current.employee_id,
        current.ot_date,
        current.start_time,
        current.end_time,
        current.ot_type.as_str(),
        current.hours,
    ) {
        return Err(AppError::Conflict(
            "This overtime application was modified concurrently. Reload and try again.".into(),
        ));
    }

    if payroll_runs::has_non_draft_covering_dates(
        &mut *tx,
        company_id,
        locked.employee_id,
        locked.ot_date,
        locked.ot_date,
    )
    .await?
        || payroll_runs::has_non_draft_covering_dates(
            &mut *tx,
            company_id,
            effective_employee_id,
            effective_ot_date,
            effective_ot_date,
        )
        .await?
    {
        return Err(AppError::BadRequest(
            "Overtime cannot be updated for a date covered by a submitted or finalized payroll run"
                .into(),
        ));
    }

    if overtime_applications::has_overlapping_window(
        &mut *tx,
        effective_employee_id,
        effective_ot_date,
        start_time,
        end_time,
        Some(ot_id),
    )
    .await?
    {
        return Err(AppError::BadRequest(
            "This employee already has an overlapping overtime application on this date".into(),
        ));
    }

    let updated = overtime_applications::update_full(
        &mut *tx,
        ot_id,
        company_id,
        req.employee_id,
        req.ot_date,
        start_time,
        end_time,
        req.hours,
        ot_type,
        req.reason,
        actor_id,
    )
    .await?
    .ok_or_else(|| {
        AppError::Conflict(
            "This overtime application changed state while it was being updated. Reload and try again.".into(),
        )
    })?;
    tx.commit().await?;

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

    let rows = overtime_applications::delete(pool, ot_id, company_id).await?;
    if rows == 0 {
        return Err(AppError::Conflict(
            "This overtime application changed state while it was being deleted. Reload and try again.".into(),
        ));
    }

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
    let employee_id = overtime_applications::get_employee_id_for_company(pool, ot_id, company_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("OT application not found or cannot be cancelled".into())
        })?;

    let mut tx = pool.begin().await?;
    // Lock employee BEFORE locking overtime_applications to avoid AB-BA deadlocks
    // with `approve_overtime` and `update_overtime_admin`.
    employee_repo::lock_for_update(&mut *tx, employee_id, company_id).await?;

    let current = overtime_applications::get_cancellable(&mut *tx, ot_id, company_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("OT application not found or cannot be cancelled".into())
        })?;
    if current.employee_id != employee_id {
        return Err(AppError::Conflict(
            "This overtime application was reassigned concurrently. Reload and try again.".into(),
        ));
    }

    if current.status == "processed" || current.payroll_run_id.is_some() {
        return Err(AppError::BadRequest(
            "Approved OT already included in processed payroll and cannot be cancelled".into(),
        ));
    }

    if current.status == "approved" {
        if payroll_runs::has_non_draft_covering_dates(
            &mut *tx,
            company_id,
            current.employee_id,
            current.ot_date,
            current.ot_date,
        )
        .await?
        {
            return Err(AppError::BadRequest(
                "Approved OT covered by a submitted or finalized payroll run cannot be cancelled"
                    .into(),
            ));
        }
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
            ot_id,
            &description,
        )
        .await?;

        if processed {
            return Err(AppError::BadRequest(
                "Approved OT already included in processed payroll and cannot be cancelled".into(),
            ));
        }

        let deleted = payroll_entries::delete_unprocessed_overtime(
            &mut *tx,
            current.employee_id,
            company_id,
            period_year,
            period_month,
            ot_id,
            &description,
        )
        .await?;
        if deleted == 0
            && payroll_entries::exists_processed_overtime(
                &mut *tx,
                current.employee_id,
                company_id,
                period_year,
                period_month,
                ot_id,
                &description,
            )
            .await?
        {
            return Err(AppError::Conflict(
                "Approved OT was processed by payroll while being cancelled.".into(),
            ));
        }

        if current.ot_type == "public_holiday"
            && !overtime_applications::exists_replacement_granted_for_date(
                &mut *tx,
                current.employee_id,
                current.ot_date,
                ot_id,
            )
            .await?
            && !leave_balances::try_subtract_entitled_replacement(
                &mut *tx,
                current.employee_id,
                period_year,
                company_id,
            )
            .await?
        {
            return Err(AppError::BadRequest(
                "Replacement leave earned from this overtime has already been used or reserved; cancel that leave request first.".into(),
            ));
        }
    }

    let cancelled =
        overtime_applications::set_cancelled(&mut *tx, ot_id, company_id, &current.status)
            .await?
            .ok_or_else(|| {
                AppError::Conflict(
                    "This overtime application was already cancelled or modified.".into(),
                )
            })?;

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

/// One page of the overtime inbox, plus the true total matching the filter.
pub async fn get_pending_overtime(
    pool: &PgPool,
    company_id: Uuid,
    status: Option<&str>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<OvertimeWithEmployee>, i64)> {
    let total = approval_reads::count_overtime(pool, company_id, status).await?;
    let apps =
        approval_reads::list_pending_overtime(pool, company_id, status, limit, offset).await?;
    Ok((apps, total))
}

/// An overtime payroll entry, computed but not yet written.
struct StagedOvertime {
    period_year: i32,
    period_month: i32,
    description: String,
    amount: i64,
    rate: i64,
}

pub async fn approve_overtime(
    pool: &PgPool,
    company_id: Uuid,
    ot_id: Uuid,
    reviewer: Reviewer,
    notes: Option<&str>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<OvertimeApplication> {
    // Approval, the staged payroll entry and the replacement-leave grant all
    // commit together. They used to be four writes on four separate connections
    // after the state transition had already committed, and `set_approved` is a
    // compare-and-swap on `status = 'pending'`, so a retry after any of them
    // failed returned "not found or not pending". Two concrete losses: the
    // staged `payroll_entries` row is the marker
    // `portal_service::exists_processed_overtime` gates cancellation on, so
    // dropping it let an employee cancel already-paid overtime; and the
    // public-holiday replacement-leave day simply vanished.
    //
    // The settings reads stay on the pool, ahead of the transaction — they are
    // company configuration, not request state, and `settings_service` takes
    // `&PgPool`. The row the CAS approves is re-checked below.
    let reviewer_id = reviewer.user_id;
    let preview = overtime_applications::get_pending(pool, ot_id, company_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("OT application not found or not pending".into()))?;

    // Approval is the third call site of the hours rule, and the one the
    // database cannot cover. Rows written before the bound existed are outside
    // the NOT VALID CHECK entirely, and the CHECK is in any case only the outer
    // 0 < h <= 24 bound — it cannot see the declared window. Without this a
    // legacy application still stages the money it was always going to stage.
    //
    // The message keeps the offending figure and the window the validator
    // names, so an HR user can correct the row without raising a ticket.
    ensure_overtime_hours_within_window(preview.hours, preview.start_time, preview.end_time)
        .map_err(|err| match err {
            AppError::BadRequest(detail) => AppError::BadRequest(format!(
                "The overtime application for {} cannot be approved. {detail}. Edit the hours or the times, then approve it again.",
                preview.ot_date
            )),
            other => other,
        })?;

    // Rated through the same `OvertimeSettings::rate_overtime` the payroll engine
    // uses, so the amount quoted here is the amount the run pays. This used to
    // parse the settings itself as `i64`/`f64` and truncate four times — an
    // `effective_hours_per_day` of "7.5" failed `parse::<i64>()` and silently
    // became 8, a saved "0" divided by zero, and the RM5,000/3 h case quoted
    // RM108.12 against the run's RM108.17.
    let ot_settings = settings_service::overtime_settings(pool, company_id).await;
    let staged = if let Some((hourly_rate, basic_salary)) =
        employee_repo::overtime_rate_basis(pool, preview.employee_id).await?
    {
        let ot = &preview;
        let multiplier = ot_settings.multiplier_for(&ot.ot_type);
        let rating =
            ot_settings.try_rate_overtime(hourly_rate, basic_salary, &ot.ot_type, ot.hours)?;

        Some(StagedOvertime {
            period_year: ot.ot_date.year(),
            period_month: ot.ot_date.month0() as i32 + 1,
            description: format!(
                "OT {} - {} ({} {}h @ {}x)",
                ot.ot_date,
                ot.ot_type.replace('_', " "),
                ot.start_time.format("%H:%M"),
                ot.hours,
                multiplier.normalize()
            ),
            amount: rating.amount_sen,
            rate: rating.rate_sen,
        })
    } else {
        None
    };

    let mut tx = pool.begin().await?;
    employee_repo::lock_for_update(&mut *tx, preview.employee_id, company_id).await?;
    if payroll_runs::has_non_draft_covering_dates(
        &mut *tx,
        company_id,
        preview.employee_id,
        preview.ot_date,
        preview.ot_date,
    )
    .await?
    {
        return Err(AppError::BadRequest(
            "Overtime cannot be approved for a date covered by a submitted or finalized payroll run"
                .into(),
        ));
    }
    let ot = overtime_applications::set_approved(&mut *tx, ot_id, company_id, reviewer_id, notes)
        .await?
        .ok_or_else(|| AppError::BadRequest("OT application not found or not pending".into()))?;

    // Compared against the row the CAS returned, inside the transaction, so a
    // denial rolls the approval back rather than leaving it approved.
    ensure_not_self_approval_on(&mut *tx, &reviewer, company_id, ot.employee_id, "overtime")
        .await?;
    ensure_not_creator_or_editor(
        &reviewer,
        ot.created_by,
        ot.updated_by,
        &ot.editor_ids,
        "overtime application",
    )?;

    // The CAS guarantees the application was still pending, not that it was
    // unchanged since the figures above were computed.
    if (
        ot.employee_id,
        ot.ot_date,
        ot.start_time,
        ot.end_time,
        ot.ot_type.as_str(),
        ot.hours,
    ) != (
        preview.employee_id,
        preview.ot_date,
        preview.start_time,
        preview.end_time,
        preview.ot_type.as_str(),
        preview.hours,
    ) {
        return Err(AppError::Conflict(
            "This overtime application changed while it was being approved. Please review it again."
                .into(),
        ));
    }

    if let Some(s) = &staged {
        payroll_entries::insert_overtime(
            &mut *tx,
            Uuid::now_v7(),
            ot.employee_id,
            company_id,
            s.period_year,
            s.period_month,
            &s.description,
            s.amount,
            ot.hours,
            s.rate,
            Some(ot.id),
            reviewer_id,
        )
        .await?;
    }

    // Replacement leave: grant at most 1 day per public holiday date per employee.
    if ot.ot_type == "public_holiday"
        && !overtime_applications::exists_replacement_granted_for_date(
            &mut *tx,
            ot.employee_id,
            ot.ot_date,
            ot.id,
        )
        .await?
    {
        // Find or create system "Replacement Leave" type for this company
        let rl_type_id = leave_types::upsert_replacement_leave(&mut *tx, company_id).await?;

        // UPSERT leave balance: increment entitled_days by 1
        let year = ot.ot_date.year();
        leave_balances::upsert_entitled_replacement(&mut *tx, ot.employee_id, rl_type_id, year)
            .await?;
    }

    tx.commit().await?;

    audit_self_approval_override(
        pool,
        &reviewer,
        company_id,
        "overtime",
        ot.id,
        ot.employee_id,
        ot.created_by,
        ot.updated_by,
        &ot.editor_ids,
        audit_meta,
    )
    .await;

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
