//! Data access for the `leave_balances` table (per-employee/type/year day counters).

use rust_decimal::Decimal;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

/// Add `days` to the pending bucket. Returns rows affected (0 = no balance row
/// for that employee/type/year, which callers treat as "not initialized").
pub async fn add_pending(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    leave_type_id: Uuid,
    days: Decimal,
    year: i32,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        r#"UPDATE leave_balances
        SET pending_days = pending_days + $3, updated_at = NOW()
        WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
        employee_id,
        leave_type_id,
        days,
        year,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}

/// Remove `days` from the pending bucket (floored at zero).
pub async fn subtract_pending(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    leave_type_id: Uuid,
    days: Decimal,
    year: i32,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE leave_balances
        SET pending_days = GREATEST(pending_days - $3, 0), updated_at = NOW()
        WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
        employee_id,
        leave_type_id,
        days,
        year,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Remove `days` from the taken bucket (floored at zero), used when cancelling
/// an already-approved leave.
pub async fn subtract_taken(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    leave_type_id: Uuid,
    days: Decimal,
    year: i32,
) -> AppResult<()> {
    // NOTE: continuation indentation matches the byte-exact SQL in the offline
    // `.sqlx` cache (this UPDATE was originally nested inside an `else if`).
    sqlx::query!(
        r#"UPDATE leave_balances
            SET taken_days = GREATEST(taken_days - $3, 0), updated_at = NOW()
            WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
        employee_id,
        leave_type_id,
        days,
        year,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Move `days` from pending to taken (on approval).
pub async fn move_pending_to_taken(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    leave_type_id: Uuid,
    days: Decimal,
    year: i32,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE leave_balances SET
            taken_days = taken_days + $3,
            pending_days = GREATEST(pending_days - $3, 0),
            updated_at = NOW()
        WHERE employee_id = $1 AND leave_type_id = $2 AND year = $4"#,
        employee_id,
        leave_type_id,
        days,
        year,
    )
    .execute(executor)
    .await?;
    Ok(())
}
