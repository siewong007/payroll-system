//! Data access for the `socso_rates` table.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;
use crate::models::statutory::SocsoContributionRate;

/// SOCSO rate band for a (capped) wage/date, if a matching band exists.
pub async fn find_rate(
    executor: impl Executor<'_, Database = Postgres>,
    wage: i64,
    effective_date: NaiveDate,
) -> AppResult<Option<SocsoContributionRate>> {
    let rate = sqlx::query_as!(
        SocsoContributionRate,
        r#"
        SELECT first_cat_employee, first_cat_employer, second_cat_employer
        FROM socso_rates
        WHERE wage_from <= $1 AND wage_to >= $1
          AND effective_from <= $2
          AND (effective_to IS NULL OR effective_to >= $2)
        ORDER BY effective_from DESC
        LIMIT 1
        "#,
        wage,
        effective_date,
    )
    .fetch_optional(executor)
    .await?;
    Ok(rate)
}
