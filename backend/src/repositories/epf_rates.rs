//! Data access for the `epf_rates` table (EPF Third Schedule lookup).

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;
use crate::models::statutory::EpfContributionRate;

/// Third-Schedule EPF contribution for a category/wage/date, if a matching band exists.
pub async fn find_contribution(
    executor: impl Executor<'_, Database = Postgres>,
    category: &str,
    wage: i64,
    effective_date: NaiveDate,
) -> AppResult<Option<EpfContributionRate>> {
    let rate = sqlx::query_as!(
        EpfContributionRate,
        r#"
        SELECT employee_contribution, employer_contribution
        FROM epf_rates
        WHERE category = $1
          AND wage_from <= $2 AND wage_to >= $2
          AND effective_from <= $3
          AND (effective_to IS NULL OR effective_to >= $3)
        ORDER BY effective_from DESC
        LIMIT 1
        "#,
        category,
        wage,
        effective_date,
    )
    .fetch_optional(executor)
    .await?;
    Ok(rate)
}
