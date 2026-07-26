//! Database clock helpers (current day-of-week / local time in a timezone).
//!
//! Not table-scoped: these read the *database* clock so attendance logic agrees with
//! server time regardless of the app process's timezone.

use chrono::NaiveTime;
use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;

/// Day of week (0=Sunday … 6=Saturday) in `tz`, per the DB clock.
pub async fn dow_in_tz(
    executor: impl Executor<'_, Database = Postgres>,
    tz: &str,
) -> AppResult<i16> {
    let dow = sqlx::query_scalar!(
        r#"SELECT EXTRACT(DOW FROM (NOW() AT TIME ZONE $1))::int2 AS "dow!""#,
        tz,
    )
    .fetch_one(executor)
    .await?;
    Ok(dow)
}

/// Current local time-of-day in `tz`, per the DB clock.
pub async fn local_time_in_tz(
    executor: impl Executor<'_, Database = Postgres>,
    tz: &str,
) -> AppResult<NaiveTime> {
    let now_local = sqlx::query_scalar!(
        r#"SELECT (NOW() AT TIME ZONE $1)::time AS "now_local!""#,
        tz,
    )
    .fetch_one(executor)
    .await?;
    Ok(now_local)
}

/// Local calendar date and time-of-day in `tz`, per the DB clock, in a single
/// round trip. The auto-absent catch-up uses both (date targeting + the
/// "past the daily cutoff yet?" decision).
pub async fn date_and_time_in_tz(
    executor: impl Executor<'_, Database = Postgres>,
    tz: &str,
) -> AppResult<(chrono::NaiveDate, NaiveTime)> {
    let row = sqlx::query!(
        r#"SELECT (NOW() AT TIME ZONE $1)::date AS "today!",
                  (NOW() AT TIME ZONE $1)::time AS "now_local!""#,
        tz,
    )
    .fetch_one(executor)
    .await?;
    Ok((row.today, row.now_local))
}

/// Day-of-week (0=Sunday … 6=Saturday) and local time-of-day in `tz`, per the
/// DB clock, in a single round trip. Check-in status determination needs both.
pub async fn dow_and_time_in_tz(
    executor: impl Executor<'_, Database = Postgres>,
    tz: &str,
) -> AppResult<(i16, NaiveTime)> {
    let row = sqlx::query!(
        r#"SELECT EXTRACT(DOW FROM (NOW() AT TIME ZONE $1))::int2 AS "dow!",
                  (NOW() AT TIME ZONE $1)::time AS "now_local!""#,
        tz,
    )
    .fetch_one(executor)
    .await?;
    Ok((row.dow, row.now_local))
}
