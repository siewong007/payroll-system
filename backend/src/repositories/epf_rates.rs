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
        SELECT rates.employee_contribution, rates.employer_contribution
        FROM epf_rates rates
        JOIN statutory_rule_sets rules ON rules.id = rates.rule_set_id
        WHERE rules.rule_code = 'epf'
          AND rules.status = 'verified'
          AND rules.effective_from <= $3
          AND (rules.effective_to IS NULL OR rules.effective_to >= $3)
          AND rates.category = $1
          AND rates.wage_from <= $2 AND rates.wage_to >= $2
          AND rates.effective_from <= $3
          AND (rates.effective_to IS NULL OR rates.effective_to >= $3)
        ORDER BY rates.effective_from DESC
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
