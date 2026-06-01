//! Data access for the `holidays` table.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::calendar::Holiday;

pub async fn list_for_year(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    year: i32,
) -> AppResult<Vec<Holiday>> {
    let holidays = sqlx::query_as!(
        Holiday,
        r#"SELECT * FROM holidays
        WHERE company_id = $1
        AND EXTRACT(YEAR FROM date)::int = $2
        ORDER BY date"#,
        company_id,
        year,
    )
    .fetch_all(executor)
    .await?;
    Ok(holidays)
}

pub async fn get_by_id(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<Holiday>> {
    let holiday = sqlx::query_as!(
        Holiday,
        "SELECT * FROM holidays WHERE id = $1 AND company_id = $2",
        id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(holiday)
}

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    name: &str,
    date: NaiveDate,
    holiday_type: &str,
    description: Option<&str>,
    is_recurring: bool,
    state: Option<&str>,
    created_by: Uuid,
) -> AppResult<Holiday> {
    let holiday = sqlx::query_as!(
        Holiday,
        r#"INSERT INTO holidays (company_id, name, date, holiday_type, description, is_recurring, state, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING *"#,
        company_id,
        name,
        date,
        holiday_type,
        description,
        is_recurring,
        state,
        created_by,
    )
    .fetch_one(executor)
    .await?;
    Ok(holiday)
}

#[allow(clippy::too_many_arguments)]
pub async fn update(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    name: Option<&str>,
    date: Option<NaiveDate>,
    holiday_type: Option<&str>,
    description: Option<&str>,
    is_recurring: Option<bool>,
    state: Option<&str>,
    updated_by: Uuid,
) -> AppResult<Option<Holiday>> {
    let holiday = sqlx::query_as!(
        Holiday,
        r#"UPDATE holidays SET
            name = COALESCE($3, name),
            date = COALESCE($4, date),
            holiday_type = COALESCE($5, holiday_type),
            description = COALESCE($6, description),
            is_recurring = COALESCE($7, is_recurring),
            state = COALESCE($8, state),
            updated_by = $9,
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *"#,
        id,
        company_id,
        name,
        date,
        holiday_type,
        description,
        is_recurring,
        state,
        updated_by,
    )
    .fetch_optional(executor)
    .await?;
    Ok(holiday)
}

pub async fn delete(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        "DELETE FROM holidays WHERE id = $1 AND company_id = $2",
        id,
        company_id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}

/// Count holidays matching a (date, name) for a company — used to dedupe ICS imports.
pub async fn count_matching(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    date: NaiveDate,
    name: &str,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM holidays WHERE company_id = $1 AND date = $2 AND name = $3"#,
        company_id,
        date,
        name,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}
