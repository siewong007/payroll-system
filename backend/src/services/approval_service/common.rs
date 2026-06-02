//! Shared helpers used by leave / claim / overtime approval submodules.

use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::repositories::{employees as employee_repo, leave_types as leave_type_repo};

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

pub(super) fn validate_overtime_type(ot_type: &str) -> AppResult<()> {
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
