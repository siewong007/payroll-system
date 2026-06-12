//! Data access for the `company_work_schedules` table.

use chrono::NaiveTime;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::work_schedule::WorkSchedule;

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

/// The company's default schedule row, if set.
pub async fn get_default(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Option<WorkSchedule>> {
    let schedule = sqlx::query_as!(
        WorkSchedule,
        "SELECT * FROM company_work_schedules WHERE company_id = $1 AND is_default = TRUE",
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(schedule)
}

pub async fn list_for_company(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<WorkSchedule>> {
    let schedules = sqlx::query_as!(
        WorkSchedule,
        "SELECT * FROM company_work_schedules WHERE company_id = $1 ORDER BY is_default DESC, name",
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(schedules)
}

pub async fn get_by_id(
    executor: impl Executor<'_, Database = Postgres>,
    schedule_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<WorkSchedule>> {
    let schedule = sqlx::query_as!(
        WorkSchedule,
        "SELECT * FROM company_work_schedules WHERE id = $1 AND company_id = $2",
        schedule_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(schedule)
}

/// Create or replace the company's single default schedule.
#[allow(clippy::too_many_arguments)]
pub async fn upsert_default(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    name: &str,
    start_time: NaiveTime,
    end_time: NaiveTime,
    grace_minutes: i32,
    half_day_hours: f64,
    timezone: &str,
) -> AppResult<WorkSchedule> {
    let schedule = sqlx::query_as!(
        WorkSchedule,
        r#"INSERT INTO company_work_schedules
           (company_id, name, start_time, end_time, grace_minutes, half_day_hours, timezone, is_default)
           VALUES ($1, $2, $3, $4, $5, $6::float8, $7, TRUE)
           ON CONFLICT (company_id) WHERE is_default = TRUE
           DO UPDATE SET
               name = EXCLUDED.name,
               start_time = EXCLUDED.start_time,
               end_time = EXCLUDED.end_time,
               grace_minutes = EXCLUDED.grace_minutes,
               half_day_hours = EXCLUDED.half_day_hours,
               timezone = EXCLUDED.timezone,
               updated_at = NOW()
           RETURNING *"#,
        company_id,
        name,
        start_time,
        end_time,
        grace_minutes,
        half_day_hours,
        timezone,
    )
    .fetch_one(executor)
    .await?;
    Ok(schedule)
}

#[allow(clippy::too_many_arguments)]
pub async fn update(
    executor: impl Executor<'_, Database = Postgres>,
    schedule_id: Uuid,
    company_id: Uuid,
    name: &str,
    start_time: NaiveTime,
    end_time: NaiveTime,
    grace_minutes: i32,
    half_day_hours: f64,
    timezone: &str,
) -> AppResult<WorkSchedule> {
    let schedule = sqlx::query_as!(
        WorkSchedule,
        r#"UPDATE company_work_schedules
           SET name = $3, start_time = $4, end_time = $5,
               grace_minutes = $6, half_day_hours = $7::float8, timezone = $8,
               updated_at = NOW()
           WHERE id = $1 AND company_id = $2
           RETURNING *"#,
        schedule_id,
        company_id,
        name,
        start_time,
        end_time,
        grace_minutes,
        half_day_hours,
        timezone,
    )
    .fetch_one(executor)
    .await?;
    Ok(schedule)
}
