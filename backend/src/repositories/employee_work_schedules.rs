//! Data access for the `employee_work_schedules` table.

use chrono::NaiveTime;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

/// `(start_time, grace_minutes)` for an employee's active schedule on a weekday, if set.
pub async fn find_timing_for_day(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    day_of_week: i16,
) -> AppResult<Option<(NaiveTime, i32)>> {
    let row = sqlx::query!(
        "SELECT start_time, grace_minutes FROM employee_work_schedules
         WHERE employee_id = $1 AND day_of_week = $2 AND is_active = TRUE",
        employee_id,
        day_of_week,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|r| (r.start_time, r.grace_minutes)))
}
