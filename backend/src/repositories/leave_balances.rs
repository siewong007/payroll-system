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

/// Undo one day of "Replacement Leave" entitlement for an employee/year (used
/// when cancelling an approved public-holiday OT).
//
// NOTE: indentation matches the byte-exact SQL in the offline `.sqlx` cache
// (this UPDATE was originally nested several blocks deep).
pub async fn subtract_entitled_replacement(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    year: i32,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE leave_balances lb
                SET entitled_days = GREATEST(lb.entitled_days - 1, 0), updated_at = NOW()
                FROM leave_types lt
                WHERE lb.leave_type_id = lt.id
                  AND lb.employee_id = $1
                  AND lb.year = $2
                  AND lt.company_id = $3
                  AND lt.name = 'Replacement Leave'"#,
        employee_id,
        year,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Grant one day of replacement-leave entitlement, inserting the balance row if
/// it does not yet exist for the employee/type/year.
//
// NOTE: indentation matches the byte-exact SQL in the offline `.sqlx` cache.
pub async fn upsert_entitled_replacement(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    leave_type_id: Uuid,
    year: i32,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO leave_balances (employee_id, leave_type_id, year, entitled_days)
            VALUES ($1, $2, $3, 1)
            ON CONFLICT (employee_id, leave_type_id, year)
            DO UPDATE SET entitled_days = leave_balances.entitled_days + 1, updated_at = NOW()"#,
        employee_id,
        leave_type_id,
        year,
    )
    .execute(executor)
    .await?;
    Ok(())
}
