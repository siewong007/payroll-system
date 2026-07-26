//! Leave admin CRUD + approval/reject workflow.

use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::config::AppConfig;
use crate::core::error::{AppError, AppResult};
use crate::core::upload_path::validate_optional_file_url;
use crate::models::portal::{CreateLeaveRequest, LeaveRequest, UpdateLeaveRequest};
use crate::repositories::reads::approvals as approval_reads;
use crate::repositories::{
    employees as employee_repo, leave_balances, leave_requests, leave_types, payroll_entries,
    users as user_repo,
};
use crate::services::audit_service::AuditRequestMeta;
use crate::services::calendar_service;
use crate::services::email_service;
use crate::services::notification_service;

use super::common::{
    Reviewer, audit_self_approval_override, ensure_employee_in_company,
    ensure_leave_type_in_company, ensure_not_self_approval, reviewer_employee_id,
};

pub use crate::models::approval::LeaveRequestWithEmployee;

// ─── Admin CRUD ───

pub async fn create_leave_request_admin(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    req: CreateLeaveRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<LeaveRequest> {
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

    if leave_requests::overlaps_existing(pool, employee_id, req.start_date, req.end_date, None)
        .await?
    {
        return Err(AppError::BadRequest(
            "This employee already has a leave request covering some of these dates".into(),
        ));
    }

    let mut tx = pool.begin().await?;

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
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<LeaveRequest> {
    let mut tx = pool.begin().await?;

    let current = leave_requests::get_pending_with_type(&mut *tx, request_id, company_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("Leave request not found or cannot be edited".into())
        })?;

    let employee_id = req.employee_id.unwrap_or(current.employee_id);
    ensure_employee_in_company(pool, company_id, employee_id).await?;
    let leave_type_id = req.leave_type_id.unwrap_or(current.leave_type_id);
    ensure_leave_type_in_company(pool, company_id, leave_type_id).await?;
    let start_date = req.start_date.unwrap_or(current.start_date);
    let end_date = req.end_date.unwrap_or(current.end_date);
    let days = req.days.unwrap_or(current.days);
    let working_days =
        calendar_service::count_working_days_between(pool, company_id, start_date, end_date)
            .await?;
    crate::services::leave_rules::validate_period(start_date, end_date, days, working_days)?;
    validate_optional_file_url(req.attachment_url.as_deref())?;

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
    )
    .await?;

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

    leave_requests::delete(pool, request_id, company_id).await?;

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
                &description,
            )
            .await?;

            if processed {
                return Err(AppError::BadRequest(
                    "Approved unpaid leave already included in processed payroll and cannot be cancelled".into(),
                ));
            }

            payroll_entries::delete_unprocessed_unpaid_leave(
                &mut *tx,
                current.employee_id,
                company_id,
                &description,
            )
            .await?;
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

    // Guards the refunds above against a concurrent cancel that already refunded.
    let cancelled = leave_requests::set_cancelled(&mut *tx, request_id, company_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("Leave request has already been cancelled".into()))?;

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

pub async fn get_pending_leave_requests(
    pool: &PgPool,
    company_id: Uuid,
    status: Option<&str>,
) -> AppResult<Vec<LeaveRequestWithEmployee>> {
    approval_reads::list_pending_leave(pool, company_id, status).await
}

/// A staged unpaid-leave salary deduction, computed but not yet written.
struct UnpaidLeaveDeduction {
    period_year: i32,
    period_month: i32,
    description: String,
    amount: i64,
}

/// Compute the deduction for an unpaid leave request: (basic_salary /
/// working_days_in_month) * unpaid_leave_working_days.
///
/// Pure reads, so it runs before the approval transaction opens. Returns `None`
/// when there is nothing to stage (unknown employee, no working days in range,
/// or an amount that rounds to zero).
async fn compute_unpaid_leave_deduction(
    pool: &PgPool,
    lr: &LeaveRequest,
) -> AppResult<Option<UnpaidLeaveDeduction>> {
    let Some((basic_salary, emp_company_id)) =
        employee_repo::basic_salary_and_company(pool, lr.employee_id).await?
    else {
        return Ok(None);
    };

    // Count working days of unpaid leave (excluding weekends/holidays)
    let unpaid_working_days = calendar_service::count_working_days_between(
        pool,
        emp_company_id,
        lr.start_date,
        lr.end_date,
    )
    .await
    .unwrap_or_else(|_| {
        // Fallback when the calendar lookup fails. Rounding the Decimal
        // directly matters: formatting it and calling parse::<i32>() fails
        // for any fractional value like "1.5", yielding 0 working days and
        // silently staging no deduction at all.
        use rust_decimal::prelude::ToPrimitive;
        lr.days.round().to_i32().unwrap_or(0)
    });

    if unpaid_working_days <= 0 {
        return Ok(None);
    }

    // Working days in the leave month, for the daily rate.
    let leave_month = lr.start_date.month();
    let leave_year = lr.start_date.year();
    let total_working_days =
        calendar_service::get_working_days_in_month(pool, emp_company_id, leave_year, leave_month)
            .await
            .unwrap_or(22); // fallback to 22 days

    let daily_rate = if total_working_days > 0 {
        basic_salary / total_working_days as i64
    } else {
        0
    };
    let amount = daily_rate * unpaid_working_days as i64;
    if amount <= 0 {
        return Ok(None);
    }

    Ok(Some(UnpaidLeaveDeduction {
        // Staged against the month the leave was taken, not the month it happened
        // to be approved in. Using the approval date deducted December leave from
        // July payroll when approval lagged, and this matches how approved
        // overtime is staged (by ot_date).
        period_year: leave_year,
        period_month: leave_month as i32,
        description: format!(
            "Unpaid leave: {} to {} ({} working days)",
            lr.start_date, lr.end_date, unpaid_working_days
        ),
        amount,
    }))
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
    let deduction = if is_paid == Some(false) {
        compute_unpaid_leave_deduction(pool, &preview).await?
    } else {
        None
    };

    // Resolved on the pool for the same reason as the deduction inputs, and
    // compared inside the transaction against the row the CAS returned.
    let reviewer_employee = reviewer_employee_id(pool, &reviewer).await?;

    let mut tx = pool.begin().await?;
    let lr = leave_requests::set_approved(&mut *tx, request_id, company_id, reviewer_id, notes)
        .await?
        .ok_or_else(|| AppError::BadRequest("Leave request not found or not pending".into()))?;

    // Read from the CAS-returned row, not from `preview`: the drift check below
    // does not cover `employee_id`, so comparing the earlier read would leave the
    // guard raceable.
    ensure_not_self_approval(reviewer_employee, lr.employee_id, "leave request")?;

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

    if let Some(d) = &deduction {
        payroll_entries::insert_unpaid_leave_deduction(
            &mut *tx,
            Uuid::now_v7(),
            lr.employee_id,
            company_id,
            d.period_year,
            d.period_month,
            &d.description,
            d.amount,
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

    if let Some(emp) = emp_info {
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
        let body = email_service::approval_email_html(
            &emp.full_name,
            &emp.company_name,
            "Leave",
            &details,
            extra,
        );
        let _ = email_service::send_email(
            config,
            pool,
            company_id,
            Some(lr.employee_id),
            None,
            "leave_approved",
            &emp.email,
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
