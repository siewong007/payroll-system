//! Shared helpers used by leave / claim / overtime approval submodules.

use chrono::NaiveTime;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::repositories::{
    employees as employee_repo, leave_types as leave_type_repo, users as user_repo,
};

pub(super) fn ensure_positive_amount(amount: i64) -> AppResult<()> {
    if amount <= 0 {
        return Err(AppError::BadRequest(
            "Amount must be greater than zero".into(),
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
pub fn ensure_overtime_hours_within_window(
    hours: Decimal,
    start: NaiveTime,
    end: NaiveTime,
) -> AppResult<()> {
    let declared_minutes = (end - start).num_minutes();
    let declared_minutes = if declared_minutes <= 0 {
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

/// The employee record the reviewer is, if the guard applies to them.
///
/// `None` for an override holder (nothing to compare against) and for a
/// reviewer with no employee record at all — an admin who is not on the payroll
/// can approve anyone. Resolved on the pool *before* the approval transaction
/// opens: acquiring a second pooled connection while holding one risks
/// exhausting the pool.
pub(super) async fn reviewer_employee_id(
    pool: &PgPool,
    reviewer: &Reviewer,
) -> AppResult<Option<Uuid>> {
    if reviewer.may_self_approve {
        return Ok(None);
    }
    user_repo::employee_id_for_user(pool, reviewer.user_id).await
}

/// Record that the guard was lifted, when it actually was.
///
/// A super admin approving their own claim must not be indistinguishable in
/// `/api/audit-logs` from an ordinary approval, so the override gets an action
/// of its own alongside the normal `approve_*` row. The extra `users` read
/// happens only on the override branch — `reviewer_employee_id` returns `None`
/// for an override holder, so there is nothing to compare until here.
pub(super) async fn audit_self_approval_override(
    pool: &PgPool,
    reviewer: &Reviewer,
    company_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
    target_employee_id: Uuid,
    audit_meta: Option<&crate::services::audit_service::AuditRequestMeta>,
) {
    if !reviewer.may_self_approve {
        return;
    }
    let Ok(Some(employee_id)) = user_repo::employee_id_for_user(pool, reviewer.user_id).await
    else {
        return;
    };
    if employee_id != target_employee_id {
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
        Some(serde_json::json!({ "employee_id": target_employee_id })),
        Some("Super admin approved their own submission; the maker-checker guard was overridden"),
        audit_meta,
    )
    .await;
}

/// Refuse an approval whose subject is the approver.
///
/// Pure, and compared against the employee id of the row the compare-and-swap
/// actually returned rather than an earlier read, so the caller's `?` rolls the
/// approval back. Only `approve_*` is guarded: rejecting yourself costs nothing,
/// cancelling removes money, and raising your own claim through the portal is
/// legitimate — the approval is the control point in maker-checker.
pub(super) fn ensure_not_self_approval(
    reviewer_employee_id: Option<Uuid>,
    target_employee_id: Uuid,
    what: &str,
) -> AppResult<()> {
    if reviewer_employee_id == Some(target_employee_id) {
        return Err(AppError::Forbidden(format!(
            "You cannot approve your own {what}. Ask another approver, or a super admin, to review it."
        )));
    }
    Ok(())
}
