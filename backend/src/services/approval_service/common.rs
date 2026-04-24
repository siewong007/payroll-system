//! Shared helpers used by leave / claim / overtime approval submodules.

use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};

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
    let exists = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(
            SELECT 1 FROM employees
            WHERE id = $1 AND company_id = $2 AND deleted_at IS NULL
        )"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .fetch_one(pool)
    .await?;

    if !exists {
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
    let exists = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(
            SELECT 1 FROM leave_types
            WHERE id = $1 AND company_id = $2 AND is_active = TRUE
        )"#,
    )
    .bind(leave_type_id)
    .bind(company_id)
    .fetch_one(pool)
    .await?;

    if !exists {
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
