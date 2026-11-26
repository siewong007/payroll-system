//! Leave admin CRUD + approval/reject workflow.

use chrono::Datelike;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::config::AppConfig;
use crate::core::error::{AppError, AppResult};
use crate::core::upload_path::validate_optional_file_url;
use crate::models::payroll::checked_round_sen;
use crate::models::portal::{CreateLeaveRequest, LeaveRequest, UpdateLeaveRequest};
use crate::repositories::reads::approvals as approval_reads;
use crate::repositories::{
    companies, employees as employee_repo, leave_balances, leave_requests, leave_types,
    payroll_entries, payroll_runs, users as user_repo,
};
use crate::services::audit_service::AuditRequestMeta;
use crate::services::calendar_service;
use crate::services::email_service;
use crate::services::notification_service;

use super::common::{
    Reviewer, audit_self_approval_override, ensure_employee_in_company,
    ensure_leave_type_in_company, ensure_not_creator_or_editor, ensure_not_self_approval_on,
};
use crate::models::upload::UploadAccess;
use crate::services::upload_service;

pub use crate::models::approval::LeaveRequestWithEmployee;

// ─── Admin CRUD ───

pub async fn create_leave_request_admin(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    req: CreateLeaveRequest,
    actor_id: Uuid,
    access: &UploadAccess,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<LeaveRequest> {
    // Bound the span before the calendar walk, as the portal path does.
    crate::services::leave_rules::validate_range(req.start_date, req.end_date)?;
    let working_days = calendar_service::count_working_days_between(
        pool,
        company_id,
        req.start_date,
        req.end_date,
    )
    .await?;
    crate::services::leave_rules::validate_period(
        req.start_date,
        req.end_date,
        req.days,
        working_days,
    )?;
    ensure_employee_in_company(pool, company_id, employee_id).await?;
    ensure_leave_type_in_company(pool, company_id, req.leave_type_id).await?;
    // `attachment_url` is joined onto a filesystem path by the backup export and
    // restore paths, so it is settled here rather than at every sink.
    validate_optional_file_url(req.attachment_url.as_deref())?;
    upload_service::ensure_upload_attach_authorized(
        pool,
        access,
        req.attachment_url.as_deref(),
        false,
    )
    .await?;

    let mut tx = pool.begin().await?;
    employee_repo::lock_for_update(&mut *tx, employee_id, company_id).await?;

    if leave_requests::overlaps_existing(&mut *tx, employee_id, req.start_date, req.end_date, None)
        .await?
    {
        return Err(AppError::BadRequest(
            "This employee already has a leave request covering some of these dates".into(),
        ));
    }

    let leave = leave_requests::insert_with_type(
        &mut *tx,
        employee_id,
        company_id,
        req.leave_type_id,
        req.start_date,
        req.end_date,
        req.days,
        req.reason,
        req.attachment_url,
        req.attachment_name,
        Some(actor_id),
    )
    .await?;

    let year = req.start_date.year();
    crate::services::leave_rules::reserve_pending_days(
        &mut tx,
        employee_id,
        req.leave_type_id,
        req.days,
        year,
    )
    .await?;

    tx.commit().await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
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
    access: &UploadAccess,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<LeaveRequest> {
    // Perform pool reads BEFORE opening the write transaction to avoid holding
    // a write tx connection while acquiring additional pooled connections.
    let preview = leave_requests::get_pending_with_type(pool, request_id, company_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("Leave request not found or cannot be edited".into())
        })?;

    let employee_id = req.employee_id.unwrap_or(preview.employee_id);
    ensure_employee_in_company(pool, company_id, employee_id).await?;
    let leave_type_id = req.leave_type_id.unwrap_or(preview.leave_type_id);
    ensure_leave_type_in_company(pool, company_id, leave_type_id).await?;
    let start_date = req.start_date.unwrap_or(preview.start_date);
    let end_date = req.end_date.unwrap_or(preview.end_date);
    let days = req.days.unwrap_or(preview.days);
    crate::services::leave_rules::validate_range(start_date, end_date)?;
    let working_days =
        calendar_service::count_working_days_between(pool, company_id, start_date, end_date)
            .await?;
    crate::services::leave_rules::validate_period(start_date, end_date, days, working_days)?;

    let non_empty_attachment = req.attachment_url.as_deref().filter(|s| !s.is_empty());
    validate_optional_file_url(non_empty_attachment)?;
    upload_service::ensure_upload_attach_authorized(pool, access, non_empty_attachment, false)
        .await?;

    let mut tx = pool.begin().await?;

    let current =
        leave_requests::get_pending_with_type_for_update(&mut *tx, request_id, company_id)
            .await?
            .ok_or_else(|| {
                AppError::Conflict(
                "This leave request changed state while it was being updated. Reload and try again."
                    .into(),
            )
            })?;

    if (
        current.employee_id,
        current.leave_type_id,
        current.start_date,
        current.end_date,
        current.days,
    ) != (
        preview.employee_id,
        preview.leave_type_id,
        preview.start_date,
        preview.end_date,
        preview.days,
    ) {
        return Err(AppError::Conflict(
            "This leave request was modified concurrently. Reload and try again.".into(),
        ));
    }

    let (first_emp, second_emp) = if current.employee_id <= employee_id {
        (current.employee_id, employee_id)
    } else {
        (employee_id, current.employee_id)
    };
    employee_repo::lock_for_update(&mut *tx, first_emp, company_id).await?;
    if second_emp != first_emp {
        employee_repo::lock_for_update(&mut *tx, second_emp, company_id).await?;
    }

    if leave_requests::overlaps_existing(
        &mut *tx,
        employee_id,
        start_date,
        end_date,
        Some(request_id),
    )
    .await?
    {
        return Err(AppError::BadRequest(
            "This employee already has another leave request covering some of these dates".into(),
        ));
    }

    let old_year = current.start_date.year();
    let new_year = start_date.year();
    let old_key = (current.employee_id, current.leave_type_id, old_year);
    let new_key = (employee_id, leave_type_id, new_year);
    let (lock_a, lock_b) = if old_key <= new_key {
        (old_key, new_key)
    } else {
        (new_key, old_key)
    };
    let _ = leave_balances::lock_for_update(&mut *tx, lock_a.0, lock_a.1, lock_a.2).await?;
    if lock_b != lock_a {
        let _ = leave_balances::lock_for_update(&mut *tx, lock_b.0, lock_b.1, lock_b.2).await?;
    }

    leave_balances::subtract_pending(
        &mut *tx,
        current.employee_id,
        current.leave_type_id,
        current.days,
        old_year,
    )
    .await?;

    crate::services::leave_rules::reserve_pending_days(
        &mut tx,
        employee_id,
        leave_type_id,
        days,
        new_year,
    )
    .await?;

    let updated = leave_requests::update_full(
        &mut *tx,
        request_id,
        company_id,
        employee_id,
        leave_type_id,
        start_date,
        end_date,
        days,
        req.reason,
        req.attachment_url,
        req.attachment_name,
        Some(actor_id),
    )
    .await?
    .ok_or_else(|| {
        AppError::Conflict(
            "This leave request changed state while it was being updated. Reload and try again."
                .into(),
        )
    })?;

    tx.commit().await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
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
    let current = leave_requests::get_cancelled_with_type(pool, request_id, company_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("Leave request not found or cannot be deleted".into())
        })?;

    let rows = leave_requests::delete(pool, request_id, company_id).await?;
    if rows == 0 {
        return Err(AppError::Conflict(
            "This leave request changed state while it was being deleted. Reload and try again."
                .into(),
        ));
    }

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
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

    let current = leave_requests::get_cancellable_with_type(&mut *tx, request_id, company_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("Leave request not found or cannot be cancelled".into())
        })?;

    if current.status == "approved" {
        if payroll_runs::has_paid_covering_dates(
            &mut *tx,
            company_id,
            current.employee_id,
            current.start_date,
            current.end_date,
        )
        .await?
        {
            return Err(AppError::BadRequest(
                "Approved leave covered by a paid payroll run cannot be cancelled".into(),
            ));
        }

        let is_paid = leave_types::get_is_paid(&mut *tx, current.leave_type_id).await?;

        if is_paid == Some(false) {
            let description = format!(
                "Unpaid leave: {} to {}%",
                current.start_date, current.end_date
            );
            let processed = payroll_entries::exists_processed_unpaid_leave(
                &mut *tx,
                current.employee_id,
                company_id,
                request_id,
                &description,
            )
            .await?;

            if processed {
                return Err(AppError::BadRequest(
                    "Approved unpaid leave already included in processed payroll and cannot be cancelled".into(),
                ));
            }

            let deleted = payroll_entries::delete_unprocessed_unpaid_leave(
                &mut *tx,
                current.employee_id,
                company_id,
                request_id,
                &description,
            )
            .await?;
            if deleted == 0
                && payroll_entries::exists_processed_unpaid_leave(
                    &mut *tx,
                    current.employee_id,
                    company_id,
                    request_id,
                    &description,
                )
                .await?
            {
                return Err(AppError::Conflict(
                    "Approved unpaid leave was processed by payroll while being cancelled.".into(),
                ));
            }
        }
    }

    let year = current.start_date.year();
    if current.status == "pending" {
        leave_balances::subtract_pending(
            &mut *tx,
            current.employee_id,
            current.leave_type_id,
            current.days,
            year,
        )
        .await?;
    } else if current.status == "approved" {
        leave_balances::subtract_taken(
            &mut *tx,
            current.employee_id,
            current.leave_type_id,
            current.days,
            year,
        )
        .await?;
    }

    // Guards the refunds above against a concurrent status change.
    let cancelled =
        leave_requests::set_cancelled(&mut *tx, request_id, company_id, &current.status)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest("Leave request has already been cancelled".into())
            })?;

    tx.commit().await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
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

/// One page of the leave inbox, plus the true total matching the filter.
pub async fn get_pending_leave_requests(
    pool: &PgPool,
    company_id: Uuid,
    status: Option<&str>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<LeaveRequestWithEmployee>, i64)> {
    let total = approval_reads::count_leave(pool, company_id, status).await?;
    let requests =
        approval_reads::list_pending_leave(pool, company_id, status, limit, offset).await?;
    Ok((requests, total))
}

/// A staged unpaid-leave salary deduction, computed but not yet written.
struct UnpaidLeaveDeduction {
    period_year: i32,
    period_month: i32,
    description: String,
    amount: i64,
    /// Working days of unpaid leave, stored on the entry's `quantity` so the
    /// payslip's day count is a number rather than a substring of the
    /// description.
    working_days: Decimal,
}

/// Compute the deduction for an unpaid leave request:
/// `basic_salary × unpaid_working_days ÷ divisor`.
///
/// The divisor is the company's contractual paid days per month
/// (`companies.unpaid_leave_divisor`), falling back to the calendar's working-day
/// count for the leave month when it is unset. The numerator stays the calendar's
/// count of working days actually taken.
///
/// One Decimal expression, rounded once. It used to compute an integer daily rate
/// first — `basic_salary / total_working_days` in i64 — and only then multiply,
/// so a RM3,000 basic over a 21-day month lost 2 sen on *every* day taken. The
/// bare `.unwrap_or(22)` behind the calendar lookup is gone with it: a silent 22
/// in a 21-working-day month is itself a wrong figure, and the error is now
/// propagated so approval fails rather than quietly deducting the wrong amount.
///
/// Pure reads, so it runs before the approval transaction opens. Returns `None`
/// when there is nothing to stage (unknown employee, no working days in range,
/// or an amount that rounds to zero).
/// Compute per-month unpaid-leave deductions, splitting cross-month ranges across
/// calendar months so each month uses its own working-day divisor and lands in
/// that month's payroll run (H-06).
async fn compute_unpaid_leave_deductions(
    pool: &PgPool,
    lr: &LeaveRequest,
) -> AppResult<Vec<UnpaidLeaveDeduction>> {
    let Some((basic_salary, emp_company_id)) =
        employee_repo::basic_salary_and_company(pool, lr.employee_id).await?
    else {
        return Ok(Vec::new());
    };

    if lr.start_date > lr.end_date || lr.days <= Decimal::ZERO {
        return Ok(Vec::new());
    }

    let configured_divisor = companies::get_unpaid_leave_divisor(pool, emp_company_id)
        .await?
        .filter(|divisor| *divisor > 0);

    let is_single_month =
        lr.start_date.year() == lr.end_date.year() && lr.start_date.month() == lr.end_date.month();
    let mut remaining_days = lr.days;
    let mut deductions = Vec::new();
    let mut cursor = lr.start_date;

    while cursor <= lr.end_date && remaining_days > Decimal::ZERO {
        let year = cursor.year();
        let month = cursor.month();
        let next_month_start = if month == 12 {
            chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1)
        } else {
            chrono::NaiveDate::from_ymd_opt(year, month + 1, 1)
        }
        .ok_or_else(|| AppError::Internal("Invalid leave month boundary".into()))?;
        let month_end = next_month_start
            .pred_opt()
            .ok_or_else(|| AppError::Internal("Invalid leave month end".into()))?;
        let chunk_end = lr.end_date.min(month_end);

        let chunk_working_days =
            calendar_service::count_working_days_between(pool, emp_company_id, cursor, chunk_end)
                .await?;

        if chunk_working_days > 0 {
            let chunk_days = remaining_days.min(Decimal::from(chunk_working_days));
            remaining_days -= chunk_days;

            let divisor = match configured_divisor {
                Some(divisor) => divisor,
                None => {
                    calendar_service::get_working_days_in_month(pool, emp_company_id, year, month)
                        .await?
                }
            };

            if divisor > 0 && chunk_days > Decimal::ZERO {
                let amount = checked_round_sen(
                    Decimal::from(basic_salary) * chunk_days / Decimal::from(divisor),
                )?;
                if amount > 0 {
                    let description = if is_single_month {
                        format!(
                            "Unpaid leave: {} to {} ({} working days)",
                            lr.start_date,
                            lr.end_date,
                            chunk_days.normalize()
                        )
                    } else {
                        format!(
                            "Unpaid leave: {} to {} ({}/{:02}, {} working days)",
                            cursor,
                            chunk_end,
                            year,
                            month,
                            chunk_days.normalize()
                        )
                    };
                    deductions.push(UnpaidLeaveDeduction {
                        period_year: year,
                        period_month: month as i32,
                        description,
                        amount,
                        working_days: chunk_days,
                    });
                }
            }
        }

        cursor = next_month_start;
    }

    Ok(deductions)
}

pub async fn approve_leave(
    pool: &PgPool,
    config: &AppConfig,
    company_id: Uuid,
    request_id: Uuid,
    reviewer: Reviewer,
    notes: Option<&str>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<LeaveRequest> {
    let mailer = email_service::SharedMailer::from_config(config);
    approve_leave_with_mailer(
        pool, &mailer, company_id, request_id, reviewer, notes, audit_meta,
    )
    .await
}

pub async fn approve_leave_with_mailer(
    pool: &PgPool,
    mailer: &email_service::SharedMailer,
    company_id: Uuid,
    request_id: Uuid,
    reviewer: Reviewer,
    notes: Option<&str>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<LeaveRequest> {
    let reviewer_id = reviewer.user_id;

    // Everything this approval writes has to commit together — the state
    // transition, the pending->taken balance move, and (for unpaid leave) the
    // salary deduction. The deduction used to be staged on the pool *after* the
    // commit, and because `set_approved` is a compare-and-swap on
    // `status = 'pending'`, a retry after that failed returned "not found or not
    // pending". So a dropped insert was unrecoverable: the leave read as
    // approved, no `payroll_entries` row existed, and the next run paid the
    // employee full basic for a month they took unpaid.
    //
    // The deduction inputs are read *before* the transaction opens: the two
    // `calendar_service` helpers take `&PgPool`, and acquiring a second pooled
    // connection while holding this transaction risks exhausting the pool. The
    // fields they were computed from are re-checked against the row the CAS
    // actually approved, below, so a concurrent edit cannot slip past.
    let preview = leave_requests::get_pending_with_type(pool, request_id, company_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("Leave request not found or not pending".into()))?;

    let is_paid = leave_types::get_is_paid(pool, preview.leave_type_id).await?;
    let deductions = if is_paid == Some(false) {
        compute_unpaid_leave_deductions(pool, &preview).await?
    } else {
        Vec::new()
    };

    let mut tx = pool.begin().await?;
    let lr = leave_requests::set_approved(&mut *tx, request_id, company_id, reviewer_id, notes)
        .await?
        .ok_or_else(|| AppError::BadRequest("Leave request not found or not pending".into()))?;

    // Check inside the transaction against `lr.employee_id` via EXISTS so
    // duplicate or decoy employee rows can never bypass the guard.
    ensure_not_self_approval_on(
        &mut *tx,
        &reviewer,
        company_id,
        lr.employee_id,
        "leave request",
    )
    .await?;
    ensure_not_creator_or_editor(
        &reviewer,
        lr.created_by,
        lr.updated_by,
        &lr.editor_ids,
        "leave request",
    )?;

    // The request could have been edited between the read above and the CAS —
    // the CAS only guarantees it was still *pending*, not unchanged. Anything
    // the deduction depends on having moved means the figure is stale.
    // `employee_id` is in the tuple because the deduction was computed from that
    // employee's basic salary and company calendar.
    if (
        lr.employee_id,
        lr.leave_type_id,
        lr.start_date,
        lr.end_date,
        lr.days,
    ) != (
        preview.employee_id,
        preview.leave_type_id,
        preview.start_date,
        preview.end_date,
        preview.days,
    ) {
        return Err(AppError::Conflict(
            "This leave request changed while it was being approved. Please review it again."
                .into(),
        ));
    }

    // Move from pending to taken
    let year = lr.start_date.year();
    leave_balances::move_pending_to_taken(
        &mut *tx,
        lr.employee_id,
        lr.leave_type_id,
        lr.days,
        year,
    )
    .await?;

    for d in &deductions {
        payroll_entries::insert_unpaid_leave_deduction(
            &mut *tx,
            Uuid::now_v7(),
            lr.employee_id,
            company_id,
            d.period_year,
            d.period_month,
            &d.description,
            d.amount,
            d.working_days,
            Some(lr.id),
            reviewer_id,
        )
        .await?;
    }

    tx.commit().await?;

    audit_self_approval_override(
        pool,
        &reviewer,
        company_id,
        "leave_request",
        lr.id,
        lr.employee_id,
        lr.created_by,
        lr.updated_by,
        &lr.editor_ids,
        audit_meta,
    )
    .await;

    // Notify employee
    let employee_user = user_repo::active_id_for_employee(pool, lr.employee_id).await?;

    let is_unpaid = is_paid == Some(false);
    if let Some(user_id) = employee_user {
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
    let emp_info = approval_reads::employee_email_info(pool, lr.employee_id).await?;

    if let Some(emp) = emp_info
        && let Some(recipient_email) = emp.email.as_deref().filter(|e| !e.trim().is_empty())
    {
        let leave_type = lr.leave_type_name.as_deref().unwrap_or("Leave");
        let period_str = format!(
            "{} to {} ({} day{})",
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
        let body = email_service::approval_email_html(
            &emp.full_name,
            &emp.company_name,
            "Leave",
            &[("Type", leave_type), ("Period", &period_str)],
            extra,
        );
        let _ = email_service::enqueue_email_with_mailer(
            mailer,
            pool,
            company_id,
            Some(lr.employee_id),
            None,
            "leave_approved",
            recipient_email,
            &emp.full_name,
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
        Some(company_id),
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
    // Rejection and the refund of the reserved days commit together. Split
    // across two connections, a failure after `set_rejected` stranded the days
    // in `pending_days` forever — nothing recomputes that column, so the
    // employee's usable entitlement stayed squeezed until the year rolled over,
    // and the CAS on `status = 'pending'` meant a retry could not repair it.
    let mut tx = pool.begin().await?;
    let lr = leave_requests::set_rejected(&mut *tx, request_id, company_id, reviewer_id, notes)
        .await?
        .ok_or_else(|| AppError::BadRequest("Leave request not found or not pending".into()))?;

    // Remove from pending
    let year = lr.start_date.year();
    leave_balances::subtract_pending(&mut *tx, lr.employee_id, lr.leave_type_id, lr.days, year)
        .await?;
    tx.commit().await?;

    // Notify employee
    let employee_user = user_repo::active_id_for_employee(pool, lr.employee_id).await?;

    if let Some(user_id) = employee_user {
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
        Some(company_id),
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
