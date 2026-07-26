//! Data access for the `eis_rates` table.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;
use crate::models::statutory::EisBand;

/// Every EIS band applicable on a date, newest schedule first.
///
/// Mirrors `socso_rates::list_bands`: schedule selection and the ceiling clamp
/// move to `services::statutory_tables` so a run reads the schedule once.
pub async fn list_bands(
    executor: impl Executor<'_, Database = Postgres>,
    effective_date: NaiveDate,
) -> AppResult<Vec<EisBand>> {
    let bands = sqlx::query_as!(
        EisBand,
        r#"
        SELECT rates.wage_from, rates.wage_to, rates.employee_contribution,
               rates.employer_contribution, rates.effective_from
        FROM eis_rates rates
        JOIN statutory_rule_sets rules ON rules.id = rates.rule_set_id
        WHERE rules.rule_code = 'eis'
          AND rules.status = 'verified'
          AND rules.effective_from <= $1
          AND (rules.effective_to IS NULL OR rules.effective_to >= $1)
          AND rates.effective_from <= $1
          AND (rates.effective_to IS NULL OR rates.effective_to >= $1)
        ORDER BY rates.effective_from DESC
        "#,
        effective_date,
    )
    .fetch_all(executor)
    .await?;
    Ok(bands)
}
