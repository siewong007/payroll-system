//! Shared helpers used by leave / claim / overtime approval submodules.

use chrono::{Datelike, NaiveDate, NaiveTime};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::approval::PendingApprovalCountResponse;
use crate::repositories::{
    employees as employee_repo, holidays as holiday_repo, leave_types as leave_type_repo,
    reads::dashboard as dashboard_reads, users as user_repo, working_day_config,
};

pub async fn get_pending_counts(
    pool: &PgPool,
    company_id: Uuid,
) -> AppResult<PendingApprovalCountResponse> {
    let counts = dashboard_reads::pending_approval_counts(pool, company_id).await?;
    Ok(PendingApprovalCountResponse {
        pending_leave: counts.pending_leave,
        pending_claims: counts.pending_claims,
        pending_overtime: counts.pending_overtime,
        total: counts.pending_leave + counts.pending_claims + counts.pending_overtime,
    })
}

pub fn ensure_positive_amount(amount: i64) -> AppResult<()> {
    if amount <= 0 || amount > crate::models::payroll::MAX_MONEY_SEN {
        return Err(AppError::BadRequest(
            "Amount must be greater than zero and within supported limits".into(),
        ));
    }
    Ok(())
}

pub(super) async fn ensure_employee_in_company(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
) -> AppResult<()> {
    if !employee_repo::exists_in_company(pool, employee_id, company_id).await? {
        return Err(AppError::NotFound(
            "Employee not found in the active company".into(),
        ));
    }
    Ok(())
}

pub(super) async fn ensure_leave_type_in_company(
    pool: &PgPool,
    company_id: Uuid,
    leave_type_id: Uuid,
) -> AppResult<()> {
    if !leave_type_repo::exists_active(pool, leave_type_id, company_id).await? {
        return Err(AppError::NotFound("Leave type not found".into()));
    }
    Ok(())
}

/// Re-exported from `approval_service` so `portal_service` shares the single
/// implementation instead of keeping the copy it used to own.
pub fn validate_overtime_type(ot_type: &str) -> AppResult<()> {
    if !["normal", "rest_day", "public_holiday"].contains(&ot_type) {
        return Err(AppError::BadRequest("Invalid ot_type".into()));
    }
    Ok(())
}

/// Validates that `ot_type` matches the company's actual holiday / working-day
/// configuration for `ot_date`: `'public_holiday'` requires a configured
/// holiday on `ot_date` and `'rest_day'` requires a non-working day.
pub async fn validate_overtime_type_for_date(
    pool: &PgPool,
    company_id: Uuid,
    ot_date: NaiveDate,
    ot_type: &str,
) -> AppResult<()> {
    validate_overtime_type(ot_type)?;
    match ot_type {
        "public_holiday" => {
            if holiday_repo::list_for_range(pool, company_id, ot_date, ot_date)
                .await?
                .is_empty()
            {
                return Err(AppError::BadRequest(
                    "Overtime type 'public_holiday' requires a configured public holiday on ot_date"
                        .into(),
                ));
            }
        }
        "rest_day" => {
            let config = working_day_config::list_for_company(pool, company_id).await?;
            let mut working_days_of_week = [false, true, true, true, true, true, false];
            for c in &config {
                if (0..=6).contains(&c.day_of_week) {
                    working_days_of_week[c.day_of_week as usize] = c.is_working_day;
                }
            }
            let dow = ot_date.weekday().num_days_from_sunday() as usize;
            if working_days_of_week[dow] {
                return Err(AppError::BadRequest(
                    "Overtime type 'rest_day' requires ot_date to fall on a non-working day".into(),
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn parse_overtime_times(
    start_time: &str,
    end_time: &str,
) -> AppResult<(chrono::NaiveTime, chrono::NaiveTime)> {
    let start = chrono::NaiveTime::parse_from_str(start_time, "%H:%M")
        .map_err(|_| AppError::BadRequest("Invalid start_time format, expected HH:MM".into()))?;
    let end = chrono::NaiveTime::parse_from_str(end_time, "%H:%M")
        .map_err(|_| AppError::BadRequest("Invalid end_time format, expected HH:MM".into()))?;
    Ok((start, end))
}

/// Bound declared overtime hours by the window the applicant declared.
///
/// `hours` is multiplied by an hourly rate to stage a payroll earning on
/// approval, and neither the schema nor the admin paths bounded it: a negative
/// value staged a negative earning, and 999.99 hours over a one-hour window
/// staged roughly RM65,000. Only the self-service portal checked, and it owned
/// its own copy of the rule — this is now the sole implementation, called by the
/// portal and by all three admin paths (create, update and approve).
///
/// The wrap past midnight is what a night shift needs; it also makes the
/// declared window at most 24 h by construction, so `hours <= declared_hours`
/// already entails the `hours <= 24` database CHECK rather than contradicting
/// it. Decimal throughout, per the repo's money rule.
///
/// Only a strictly negative difference wraps. `start == end` is a zero-length
/// window, not a 24-hour one: wrapping it turned "09:00 to 09:00" — the shape a
/// half-filled form produces — into a licence to declare a full day of
/// overtime. With the window at zero, the positivity check below rejects it
/// outright, matching migration 1011's `hours > 0` CHECK.
pub fn ensure_overtime_hours_within_window(
    hours: Decimal,
    start: NaiveTime,
    end: NaiveTime,
) -> AppResult<()> {
    let declared_minutes = (end - start).num_minutes();
    let declared_minutes = if declared_minutes < 0 {
        declared_minutes + 24 * 60
    } else {
        declared_minutes
    };
    let declared_hours = Decimal::from(declared_minutes) / Decimal::from(60);

    if hours <= Decimal::ZERO {
        return Err(AppError::BadRequest(
            "Overtime hours must be greater than zero".into(),
        ));
    }
    if hours > declared_hours {
        return Err(AppError::BadRequest(format!(
            "Overtime hours ({}) cannot exceed the {} hour(s) between start_time and end_time",
            hours, declared_hours
        )));
    }
    Ok(())
}

// ─── Maker-checker ───

/// Who is approving, and whether the self-approval guard applies to them.
///
/// Constructed in the handler from `AuthUser` rather than derived from it here,
/// so `services/` keeps no dependency on the Axum extractor. `may_self_approve`
/// is the `super_admin` override: `Permission::ManageUsers` belongs to no role
/// set except SUPER_ADMIN, so a tenant cannot mint itself the role that lifts
/// the guard.
#[derive(Debug, Clone, Copy)]
pub struct Reviewer {
    pub user_id: Uuid,
    pub may_self_approve: bool,
}

/// Refuse an approval whose subject belongs to the approver (checked inside the
/// approval transaction against `target_employee_id` via `EXISTS`, so duplicate
/// employee records can never shadow ownership).
pub(super) async fn ensure_not_self_approval_on(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    reviewer: &Reviewer,
    company_id: Uuid,
    target_employee_id: Uuid,
    what: &str,
) -> AppResult<()> {
    if reviewer.may_self_approve {
        return Ok(());
    }
    if user_repo::is_employee_owned_by_user(
        executor,
        reviewer.user_id,
        target_employee_id,
        company_id,
    )
    .await?
    {
        return Err(AppError::Forbidden(format!(
            "You cannot approve your own {what}. Ask another approver, or a super admin, to review it."
        )));
    }
    Ok(())
}

/// Record that a `super_admin` lifted a maker-checker guard (self-target,
/// creator, or prior editor).
#[allow(clippy::too_many_arguments)]
pub(super) async fn audit_self_approval_override(
    pool: &PgPool,
    reviewer: &Reviewer,
    company_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
    target_employee_id: Uuid,
    created_by: Option<Uuid>,
    updated_by: Option<Uuid>,
    editor_ids: &[Uuid],
    audit_meta: Option<&crate::services::audit_service::AuditRequestMeta>,
) {
    if !reviewer.may_self_approve {
        return;
    }
    let is_self_target = user_repo::is_employee_owned_by_user(
        pool,
        reviewer.user_id,
        target_employee_id,
        company_id,
    )
    .await
    .unwrap_or(false);
    let is_creator_or_editor = created_by == Some(reviewer.user_id)
        || updated_by == Some(reviewer.user_id)
        || editor_ids.contains(&reviewer.user_id);
    if !is_self_target && !is_creator_or_editor {
        return;
    }

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(reviewer.user_id),
        "self_approval_override",
        entity_type,
        Some(entity_id),
        None,
        Some(serde_json::json!({
            "employee_id": target_employee_id,
            "self_target": is_self_target,
            "creator_or_editor": is_creator_or_editor,
        })),
        Some("Super admin approved their own submission or edit; the maker-checker guard was overridden"),
        audit_meta,
    )
    .await;
}

/// Four-eyes maker-checker guard: the user who created or ever edited a
/// request may not be the same user who approves it.
pub(super) fn ensure_not_creator_or_editor(
    reviewer: &Reviewer,
    created_by: Option<Uuid>,
    updated_by: Option<Uuid>,
    editor_ids: &[Uuid],
    what: &str,
) -> AppResult<()> {
    if reviewer.may_self_approve {
        return Ok(());
    }
    if created_by == Some(reviewer.user_id)
        || updated_by == Some(reviewer.user_id)
        || editor_ids.contains(&reviewer.user_id)
    {
        return Err(AppError::Forbidden(format!(
            "You cannot approve a {what} you created or edited. Ask another approver to review it."
        )));
    }
    Ok(())
}
