//! Data access for the `eis_rates` table.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;
use crate::models::statutory::EisContributionRate;

/// EIS rate band for a wage/date, capped by the newest applicable schedule.
pub async fn find_rate(
    executor: impl Executor<'_, Database = Postgres>,
    wage: i64,
    effective_date: NaiveDate,
) -> AppResult<Option<EisContributionRate>> {
    let rate = sqlx::query_as!(
        EisContributionRate,
        r#"
        WITH applicable_schedule AS (
            SELECT rates.effective_from, MAX(rates.wage_to) AS wage_ceiling
            FROM eis_rates rates
            JOIN statutory_rule_sets rules ON rules.id = rates.rule_set_id
            WHERE rules.rule_code = 'eis'
              AND rules.status = 'verified'
              AND rules.effective_from <= $2
              AND (rules.effective_to IS NULL OR rules.effective_to >= $2)
              AND rates.effective_from <= $2
              AND (rates.effective_to IS NULL OR rates.effective_to >= $2)
            GROUP BY rates.effective_from
            ORDER BY rates.effective_from DESC
            LIMIT 1
        )
        SELECT r.employee_contribution, r.employer_contribution
        FROM eis_rates r
        JOIN applicable_schedule schedule
          ON schedule.effective_from = r.effective_from
        JOIN statutory_rule_sets rules ON rules.id = r.rule_set_id
        WHERE r.wage_from <= LEAST($1, schedule.wage_ceiling)
          AND r.wage_to >= LEAST($1, schedule.wage_ceiling)
          AND (r.effective_to IS NULL OR r.effective_to >= $2)
          AND rules.rule_code = 'eis'
          AND rules.status = 'verified'
          AND rules.effective_from <= $2
          AND (rules.effective_to IS NULL OR rules.effective_to >= $2)
        LIMIT 1
        "#,
        wage,
        effective_date,
    )
    .fetch_optional(executor)
    .await?;
    Ok(rate)
}
