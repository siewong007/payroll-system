use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::core::upload_path::validate_optional_file_url;
use crate::models::employee::Employee;
use crate::models::portal::*;
use crate::repositories::reads::portal as portal_reads;
use crate::repositories::{
    claims, employees as employee_repo, leave_balances, leave_requests, leave_types,
    overtime_applications, payroll_entries,
};
use crate::services::approval_service::{
    ensure_overtime_hours_within_window, validate_overtime_type,
};
use crate::services::notification_service;

// ─── Profile ───

pub async fn get_my_profile(pool: &PgPool, employee_id: Uuid) -> AppResult<Employee> {
    employee_repo::get_profile(pool, employee_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Employee profile not found".into()))
}

// ─── Payslips ───

pub async fn get_my_payslips(pool: &PgPool, employee_id: Uuid) -> AppResult<Vec<MyPayslip>> {
    portal_reads::my_payslips(pool, employee_id).await
}

// ─── Leave ───

pub async fn get_leave_types(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<LeaveType>> {
    leave_types::list_active(pool, company_id).await
}

pub async fn get_leave_balances(
    pool: &PgPool,
    employee_id: Uuid,
    year: i32,
) -> AppResult<Vec<LeaveBalanceWithType>> {
    portal_reads::leave_balances_with_type(pool, employee_id, year).await
}

pub async fn get_leave_requests(pool: &PgPool, employee_id: Uuid) -> AppResult<Vec<LeaveRequest>> {
    leave_requests::list_for_employee(pool, employee_id).await
}

pub async fn create_leave_request(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    req: CreateLeaveRequest,
) -> AppResult<LeaveRequest> {
    // Before the calendar is read, not after: the count walks the range a day
    // at a time, and this endpoint takes a bare `NaiveDate` pair from any
    // employee token.
    crate::services::leave_rules::validate_range(req.start_date, req.end_date)?;
    let working_days = crate::services::calendar_service::count_working_days_between(
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

    // Verify leave type exists
    if !leave_types::exists_active(pool, req.leave_type_id, company_id).await? {
        return Err(AppError::NotFound("Leave type not found".into()));
    }

    // `attachment_url` is joined onto a filesystem path by the backup export and
    // restore paths, so it is settled at write time rather than at every sink.
    validate_optional_file_url(req.attachment_url.as_deref())?;

    if leave_requests::overlaps_existing(pool, employee_id, req.start_date, req.end_date, None)
        .await?
    {
        return Err(AppError::BadRequest(
            "You already have a leave request covering some of these dates".into(),
        ));
    }

    // One transaction: the balance reservation below can legitimately fail
    // (insufficient entitlement, uninitialized balance), and a rejected request
    // must not leave a leave_requests row behind.
    let mut tx = pool.begin().await?;
    let leave = leave_requests::insert_self_service(
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

    // Update pending days in balance
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

    // Notify admins about new leave request
    let name = employee_repo::full_name(pool, employee_id)
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

    let lr = leave_requests::get_cancellable_for_employee(&mut *tx, request_id, employee_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("Leave request not found or cannot be cancelled".into())
        })?;

    if lr.status == "approved" {
        let is_paid = leave_types::get_is_paid(&mut *tx, lr.leave_type_id).await?;

        if is_paid == Some(false) {
            let description = format!("Unpaid leave: {} to {}%", lr.start_date, lr.end_date);
            let processed = payroll_entries::exists_processed_unpaid_leave(
                &mut *tx,
                lr.employee_id,
                lr.company_id,
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
                lr.employee_id,
                lr.company_id,
                &description,
            )
            .await?;
        }
    }

    // Guards the refund below: the status was read above, so a concurrent cancel
    // that already refunded must not refund a second time.
    if !leave_requests::mark_cancelled(&mut *tx, request_id).await? {
        return Err(AppError::BadRequest(
            "Leave request has already been cancelled".into(),
        ));
    }

    let year = lr.start_date.year();
    if lr.status == "pending" {
        leave_balances::subtract_pending(&mut *tx, employee_id, lr.leave_type_id, lr.days, year)
            .await?;
    } else if lr.status == "approved" {
        leave_balances::subtract_taken(&mut *tx, employee_id, lr.leave_type_id, lr.days, year)
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
    let rows = leave_requests::delete_cancelled_for_employee(pool, request_id, employee_id).await?;

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
    claims::list_for_employee(pool, employee_id, status).await
}

pub async fn create_claim(
    pool: &PgPool,
    employee_id: Uuid,
    company_id: Uuid,
    req: CreateClaimRequest,
) -> AppResult<Claim> {
    validate_optional_file_url(req.receipt_url.as_deref())?;

    let claim = claims::insert_draft(
        pool,
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
    .await?;
    Ok(claim)
}

pub async fn submit_claim(pool: &PgPool, employee_id: Uuid, claim_id: Uuid) -> AppResult<Claim> {
    let claim = claims::mark_submitted(pool, claim_id, employee_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("Claim not found or already submitted".into()))?;

    // Notify admins about submitted claim
    let name = employee_repo::full_name(pool, employee_id)
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

    // The `payroll_entries` probe this used to carry is gone with the staging it
    // inspected: a paid claim is now 'processed' with a `payroll_run_id`, which
    // `get_cancellable_for_employee` already excludes. The probe was keyed to
    // the approval month and reported "already in processed payroll" for claims
    // no run had paid.
    //
    // The read stays so a claim that was never cancellable still reports 400
    // rather than the 409 below, which means "someone got there first".
    if claims::get_cancellable_for_employee(&mut *tx, claim_id, employee_id)
        .await?
        .is_none()
    {
        return Err(AppError::BadRequest(
            "Claim not found or cannot be cancelled".into(),
        ));
    }

    // Compare-and-swap, so a run committing between the read and this write
    // cannot have its just-paid claim cancelled.
    if !claims::mark_cancelled(&mut *tx, claim_id).await? {
        return Err(AppError::Conflict(
            "This claim was paid or cancelled while it was being cancelled. Reload and try again."
                .into(),
        ));
    }

    tx.commit().await?;

    Ok(())
}

pub async fn delete_claim(pool: &PgPool, employee_id: Uuid, claim_id: Uuid) -> AppResult<()> {
    let rows = claims::delete_draft_or_cancelled(pool, claim_id, employee_id).await?;

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
    overtime_applications::list_for_employee(pool, employee_id).await
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

    // Both rules live in `approval_service::common` now. This path used to own
    // the only hours check in the codebase while the three admin paths had
    // none, and a duplicated ot_type allow-list besides.
    let ot_type = req.ot_type.as_deref().unwrap_or("normal");
    validate_overtime_type(ot_type)?;
    ensure_overtime_hours_within_window(req.hours, start_time, end_time)?;

    let app = overtime_applications::insert(
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

    // Notify admins
    let name = employee_repo::full_name(pool, employee_id)
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

    let app = overtime_applications::get_cancellable_for_employee(&mut *tx, id, employee_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("OT application not found or cannot be cancelled".into())
        })?;

    if app.status == "approved" {
        let description = format!("OT {} - {}%", app.ot_date, app.ot_type.replace('_', " "));
        let period_year = app.ot_date.year();
        let period_month = app.ot_date.month() as i32;
        let processed = payroll_entries::exists_processed_overtime(
            &mut *tx,
            app.employee_id,
            app.company_id,
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
            app.employee_id,
            app.company_id,
            period_year,
            period_month,
            &description,
        )
        .await?;

        if app.ot_type == "public_holiday" {
            leave_balances::subtract_entitled_replacement(
                &mut *tx,
                app.employee_id,
                period_year,
                app.company_id,
            )
            .await?;
        }
    }

    overtime_applications::mark_cancelled_for_employee(&mut *tx, id).await?;

    tx.commit().await?;

    Ok(())
}

pub async fn delete_overtime_application(
    pool: &PgPool,
    employee_id: Uuid,
    id: Uuid,
) -> AppResult<()> {
    let rows = overtime_applications::delete_cancelled_for_employee(pool, id, employee_id).await?;

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
    let leave_types = leave_types::list_active(pool, company_id).await?;

    let mut balances = vec![];
    for lt in &leave_types {
        let entitled = calculate_prorated_days(lt.default_days, date_joined, year);
        let balance =
            leave_balances::upsert_entitled(pool, employee_id, lt.id, year, entitled).await?;

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
    let leave_types = leave_types::list_active(pool, company_id).await?;
    let employees = employee_repo::list_active_ids_and_joined_dates(pool, company_id).await?;

    let mut count = 0;
    for (emp_id, date_joined) in employees {
        for lt in &leave_types {
            // Get current year balance
            let balance =
                leave_balances::get_balance_for_year(pool, emp_id, lt.id, from_year).await?;

            let carry = if let Some((entitled, taken, pending, carried_forward)) = balance {
                let remaining = entitled + carried_forward - taken - pending;
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
            let entitled = calculate_prorated_days(lt.default_days, date_joined, to_year);

            // UPSERT next year balance
            leave_balances::upsert_carried_forward(pool, emp_id, lt.id, to_year, entitled, carry)
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

    let entries =
        portal_reads::team_calendar(pool, employee_id, company_id, period_start, period_end)
            .await?;

    Ok(entries)
}
