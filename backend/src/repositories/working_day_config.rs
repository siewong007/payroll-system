//! Data access for the `working_day_config` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::calendar::WorkingDayConfig;

pub async fn list_for_company(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<WorkingDayConfig>> {
    let config = sqlx::query_as!(
        WorkingDayConfig,
        "SELECT * FROM working_day_config WHERE company_id = $1 ORDER BY day_of_week",
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(config)
}

/// Insert-or-update a single day's working flag.
pub async fn upsert(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    day_of_week: i16,
    is_working_day: bool,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO working_day_config (company_id, day_of_week, is_working_day)
            VALUES ($1, $2, $3)
            ON CONFLICT (company_id, day_of_week) DO UPDATE SET
                is_working_day = $3, updated_at = NOW()"#,
        company_id,
        day_of_week,
        is_working_day,
    )
    .execute(executor)
    .await?;
    Ok(())
}
