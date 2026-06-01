//! Data access for the `company_work_schedules` table.

use chrono::NaiveTime;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

/// `(start_time, grace_minutes)` for the company's default schedule, if set.
pub async fn find_default_timing(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Option<(NaiveTime, i32)>> {
    let row = sqlx::query!(
        "SELECT start_time, grace_minutes FROM company_work_schedules WHERE company_id = $1 AND is_default = TRUE",
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|r| (r.start_time, r.grace_minutes)))
}

/// Timezone for the company's default schedule, if set.
pub async fn find_default_timezone(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Option<String>> {
    let tz = sqlx::query_scalar!(
        "SELECT timezone FROM company_work_schedules WHERE company_id = $1 AND is_default = TRUE",
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(tz)
}
