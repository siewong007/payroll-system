use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::core::upload_path::validate_optional_managed_upload_url;
use crate::models::employee::Employee;
use crate::models::portal::*;
use crate::repositories::reads::portal as portal_reads;
use crate::repositories::{
    claims, company_work_schedules, employees as employee_repo, leave_balances, leave_requests,
    leave_types, overtime_applications, payroll_entries, payroll_runs,
};
use crate::services::approval_service::{
    ensure_overtime_hours_within_window, validate_overtime_type_for_date,
};
use crate::services::{audit_service, notification_service};

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
    actor_user_id: Uuid,
    req: CreateLeaveRequest,
    access: &crate::models::upload::UploadAccess,
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
    validate_optional_managed_upload_url(req.attachment_url.as_deref())?;
    crate::services::upload_service::ensure_upload_attach_authorized(
        pool,
        access,
        req.attachment_url.as_deref(),
        false,
    )
    .await?;

    // One transaction: serialize concurrent overlap checks via the employee lock,
    // and keep balance reservation atomic with inserting the leave request row.
    let mut tx = pool.begin().await?;
    employee_repo::lock_for_update(&mut *tx, employee_id, company_id).await?;

    if leave_requests::overlaps_existing(&mut *tx, employee_id, req.start_date, req.end_date, None)
        .await?
    {
        return Err(AppError::BadRequest(
            "You already have a leave request covering some of these dates".into(),
        ));
    }
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
        Some(actor_user_id),
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
    actor_user_id: Uuid,
    request_id: Uuid,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    let lr = leave_requests::get_cancellable_for_employee(&mut *tx, request_id, employee_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("Leave request not found or cannot be cancelled".into())
        })?;

    if lr.status == "approved" {
        let tz_name = crate::core::timezone::sanitize(
            company_work_schedules::find_default_timezone(&mut *tx, lr.company_id).await?,
        );
        let tz = crate::core::timezone::parse(&tz_name).unwrap_or(chrono_tz::Asia::Kuala_Lumpur);
        let today_local = chrono::Utc::now().with_timezone(&tz).date_naive();
        if today_local >= lr.start_date {
            return Err(AppError::BadRequest(
                "Approved leave cannot be cancelled once it has started; contact HR".into(),
            ));
        }

        if payroll_runs::has_non_draft_covering_dates(
            &mut *tx,
            lr.company_id,
            lr.employee_id,
            lr.start_date,
            lr.end_date,
        )
        .await?
        {
            return Err(AppError::BadRequest(
                "Approved leave covered by a submitted or finalized payroll run cannot be cancelled"
                    .into(),
            ));
        }

        let is_paid = leave_types::get_is_paid(&mut *tx, lr.leave_type_id).await?;

        if is_paid == Some(false) {
            let description = format!("Unpaid leave: {} to {}%", lr.start_date, lr.end_date);
            let processed = payroll_entries::exists_processed_unpaid_leave(
                &mut *tx,
                lr.employee_id,
                lr.company_id,
                request_id,
                &description,
            )
            .await?;

            if processed {
                return Err(AppError::BadRequest(
                    "Approved unpaid leave already included in processed payroll and cannot be cancelled".into(),
                ));
            }

            let _deleted = payroll_entries::delete_unprocessed_unpaid_leave(
                &mut *tx,
                lr.employee_id,
                lr.company_id,
                request_id,
                &description,
            )
            .await?;
            if payroll_entries::exists_processed_unpaid_leave(
                &mut *tx,
                lr.employee_id,
                lr.company_id,
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

    // Guards the refund below: the locked row's status was read above, so a
    // concurrent status change cannot refund twice or take the wrong branch.
    if !leave_requests::mark_cancelled(&mut *tx, request_id, &lr.status).await? {
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

    audit_service::log_action_with_metadata(
        &mut *tx,
        Some(lr.company_id),
        Some(actor_user_id),
        "cancel_leave_request_portal",
        "leave_request",
        Some(request_id),
        Some(serde_json::to_value(&lr).unwrap_or_default()),
        Some(serde_json::json!({ "status": "cancelled" })),
        Some("Employee cancelled leave request"),
        None,
    )
    .await?;

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
    actor_user_id: Uuid,
    req: CreateClaimRequest,
    access: &crate::models::upload::UploadAccess,
) -> AppResult<Claim> {
    const MAX_DRAFT_CLAIMS_PER_EMPLOYEE: i64 = 25;

    crate::services::approval_service::ensure_positive_amount(req.amount)?;
    validate_optional_managed_upload_url(req.receipt_url.as_deref())?;
    crate::services::upload_service::ensure_upload_attach_authorized(
        pool,
        access,
        req.receipt_url.as_deref(),
        false,
    )
    .await?;

    if claims::count_drafts_for_employee(pool, employee_id, company_id).await?
        >= MAX_DRAFT_CLAIMS_PER_EMPLOYEE
    {
        return Err(AppError::BadRequest(format!(
            "You already have the maximum of {MAX_DRAFT_CLAIMS_PER_EMPLOYEE} draft claims. Submit or delete an existing draft first."
        )));
    }

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
        actor_user_id,
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
            "{} submitted a claim: \"{}\" (RM {})",
            name,
            claim.title,
            crate::services::pdf_helpers::sen_to_rm(claim.amount)
        ),
        Some("claim"),
        Some(claim.id),
    )
    .await;

    Ok(claim)
}

pub async fn cancel_claim(
    pool: &PgPool,
    employee_id: Uuid,
    claim_id: Uuid,
    actor_user_id: Uuid,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    // The `payroll_entries` probe this used to carry is gone with the staging it
    // inspected: a paid claim is now 'processed' with a `payroll_run_id`, which
    // `get_cancellable_for_employee` already excludes. The probe was keyed to
    // the approval month and reported "already in processed payroll" for claims
    // no run had paid.
    //
    // The read stays so a claim that was never cancellable still reports 400
    // rather than the 409 below, which means "someone got there first".
    let Some(claim) = claims::get_cancellable_for_employee(&mut *tx, claim_id, employee_id).await?
    else {
        return Err(AppError::BadRequest(
            "Claim not found or cannot be cancelled".into(),
        ));
    };

    // Compare-and-swap, so a run committing between the read and this write
    // cannot have its just-paid claim cancelled.
    if !claims::mark_cancelled(&mut *tx, claim_id).await? {
        return Err(AppError::Conflict(
            "This claim was paid or cancelled while it was being cancelled. Reload and try again."
                .into(),
        ));
    }

    audit_service::log_action_with_metadata(
        &mut *tx,
        Some(claim.company_id),
        Some(actor_user_id),
        "cancel_claim_portal",
        "claim",
        Some(claim_id),
        Some(serde_json::to_value(&claim).unwrap_or_default()),
        Some(serde_json::json!({ "status": "cancelled" })),
        Some("Employee cancelled claim"),
        None,
    )
    .await?;

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
    actor_user_id: Uuid,
    req: CreateOvertimeRequest,
) -> AppResult<OvertimeApplication> {
    let start_time = chrono::NaiveTime::parse_from_str(&req.start_time, "%H:%M")
        .map_err(|_| AppError::BadRequest("Invalid start_time format, expected HH:MM".into()))?;
    let end_time = chrono::NaiveTime::parse_from_str(&req.end_time, "%H:%M")
        .map_err(|_| AppError::BadRequest("Invalid end_time format, expected HH:MM".into()))?;

    let ot_type = req.ot_type.as_deref().unwrap_or("normal");
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
            "Overtime cannot be applied for a date covered by a submitted or finalized payroll run"
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
            "You already have an overlapping overtime application on this date".into(),
        ));
    }

    let app = overtime_applications::insert(
        &mut *tx,
        employee_id,
        company_id,
        req.ot_date,
        start_time,
        end_time,
        req.hours,
        ot_type,
        req.reason,
        actor_user_id,
    )
    .await?;
    tx.commit().await?;

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
    actor_user_id: Uuid,
) -> AppResult<()> {
    let company_id = overtime_applications::get_company_id_for_employee(pool, id, employee_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("OT application not found or cannot be cancelled".into())
        })?;

    let mut tx = pool.begin().await?;
    // Acquire employee row lock BEFORE overtime_applications FOR UPDATE to match
    // approve_overtime / cancel_overtime_admin canonical lock ordering.
    employee_repo::lock_for_update(&mut *tx, employee_id, company_id).await?;

    let app = overtime_applications::get_cancellable_for_employee(&mut *tx, id, employee_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("OT application not found or cannot be cancelled".into())
        })?;

    if app.status == "processed" || app.payroll_run_id.is_some() {
        return Err(AppError::BadRequest(
            "Approved OT already included in processed payroll and cannot be cancelled".into(),
        ));
    }

    if app.status == "approved" {
        if payroll_runs::has_non_draft_covering_dates(
            &mut *tx,
            app.company_id,
            app.employee_id,
            app.ot_date,
            app.ot_date,
        )
        .await?
        {
            return Err(AppError::BadRequest(
                "Approved OT covered by a submitted or finalized payroll run cannot be cancelled"
                    .into(),
            ));
        }
        let description = format!("OT {} - {}%", app.ot_date, app.ot_type.replace('_', " "));
        let period_year = app.ot_date.year();
        let period_month = app.ot_date.month() as i32;
        let processed = payroll_entries::exists_processed_overtime(
            &mut *tx,
            app.employee_id,
            app.company_id,
            period_year,
            period_month,
            id,
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
            app.employee_id,
            app.company_id,
            period_year,
            period_month,
            id,
            &description,
        )
        .await?;
        if deleted == 0
            && payroll_entries::exists_processed_overtime(
                &mut *tx,
                app.employee_id,
                app.company_id,
                period_year,
                period_month,
                id,
                &description,
            )
            .await?
        {
            return Err(AppError::Conflict(
                "Approved OT was processed by payroll while being cancelled.".into(),
            ));
        }

        if app.ot_type == "public_holiday"
            && !overtime_applications::exists_replacement_granted_for_date(
                &mut *tx,
                app.employee_id,
                app.ot_date,
                id,
            )
            .await?
            && !leave_balances::try_subtract_entitled_replacement(
                &mut *tx,
                app.employee_id,
                period_year,
                app.company_id,
            )
            .await?
        {
            return Err(AppError::BadRequest(
                "Replacement leave earned from this overtime has already been used or reserved; cancel that leave request first.".into(),
            ));
        }
    }

    if !overtime_applications::mark_cancelled_for_employee(&mut *tx, id, employee_id, &app.status)
        .await?
    {
        return Err(AppError::Conflict(
            "This overtime application has already been cancelled or modified.".into(),
        ));
    }

    audit_service::log_action_with_metadata(
        &mut *tx,
        Some(app.company_id),
        Some(actor_user_id),
        "cancel_overtime_portal",
        "overtime",
        Some(id),
        Some(serde_json::to_value(&app).unwrap_or_default()),
        Some(serde_json::json!({ "status": "cancelled" })),
        Some("Employee cancelled overtime application"),
        None,
    )
    .await?;

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

/// Initialize leave balances for an employee using pre-loaded active leave types.
pub(crate) async fn initialize_leave_balances_with_types(
    pool: &PgPool,
    employee_id: Uuid,
    date_joined: chrono::NaiveDate,
    year: i32,
    leave_types: &[LeaveType],
) -> AppResult<Vec<LeaveBalance>> {
    if leave_types.is_empty() {
        return Ok(Vec::new());
    }
    let mut emp_ids = Vec::with_capacity(leave_types.len());
    let mut lt_ids = Vec::with_capacity(leave_types.len());
    let mut years = Vec::with_capacity(leave_types.len());
    let mut entitled = Vec::with_capacity(leave_types.len());
    for lt in leave_types {
        emp_ids.push(employee_id);
        lt_ids.push(lt.id);
        years.push(year);
        entitled.push(calculate_prorated_days(lt.default_days, date_joined, year));
    }
    leave_balances::upsert_entitled_batch(pool, &emp_ids, &lt_ids, &years, &entitled).await
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
    initialize_leave_balances_with_types(pool, employee_id, date_joined, year, &leave_types).await
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
    if !(2000..=2100).contains(&from_year) || to_year != from_year + 1 {
        return Err(AppError::BadRequest(
            "to_year must be exactly from_year + 1 (within 2000..=2100)".into(),
        ));
    }

    let leave_types = leave_types::list_active(pool, company_id).await?;
    let employees = employee_repo::list_active_ids_and_joined_dates(pool, company_id).await?;

    let mut tx = pool.begin().await?;
    let existing_balances: std::collections::HashMap<
        (Uuid, Uuid),
        (
            rust_decimal::Decimal,
            rust_decimal::Decimal,
            rust_decimal::Decimal,
            rust_decimal::Decimal,
        ),
    > = leave_balances::list_for_company_year(&mut *tx, company_id, from_year)
        .await?
        .into_iter()
        .map(|(emp_id, lt_id, entitled, taken, pending, carried)| {
            ((emp_id, lt_id), (entitled, taken, pending, carried))
        })
        .collect();

    let eligible_types: Vec<&LeaveType> = leave_types
        .iter()
        // Replacement Leave is earned dynamically per holiday worked, not
        // provisioned via annual carry-forward; standard seeded leave types also
        // carry `is_system = TRUE`, so do not skip on `is_system` alone.
        .filter(|lt| !lt.name.eq_ignore_ascii_case("Replacement Leave"))
        .collect();

    let cap = employees.len().saturating_mul(eligible_types.len());
    let mut emp_ids = Vec::with_capacity(cap);
    let mut lt_ids = Vec::with_capacity(cap);
    let mut years = Vec::with_capacity(cap);
    let mut entitled_list = Vec::with_capacity(cap);
    let mut carried_list = Vec::with_capacity(cap);

    for (emp_id, date_joined) in employees {
        for lt in &eligible_types {
            let carry = if let Some(&(entitled, taken, pending, carried_forward)) =
                existing_balances.get(&(emp_id, lt.id))
            {
                let remaining =
                    (entitled + carried_forward - taken - pending).max(rust_decimal::Decimal::ZERO);
                if lt.is_paid && lt.max_carry_forward > rust_decimal::Decimal::ZERO {
                    remaining.min(lt.max_carry_forward)
                } else {
                    rust_decimal::Decimal::ZERO
                }
            } else {
                rust_decimal::Decimal::ZERO
            };

            let entitled = calculate_prorated_days(lt.default_days, date_joined, to_year);
            emp_ids.push(emp_id);
            lt_ids.push(lt.id);
            years.push(to_year);
            entitled_list.push(entitled);
            carried_list.push(carry);
        }
    }

    let count = emp_ids.len() as i32;
    leave_balances::upsert_carried_forward_batch(
        &mut *tx,
        &emp_ids,
        &lt_ids,
        &years,
        &entitled_list,
        &carried_list,
    )
    .await?;
    tx.commit().await?;

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
