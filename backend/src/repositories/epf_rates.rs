//! Data access for the `epf_rates` table (EPF Third Schedule lookup).

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;
use crate::models::statutory::EpfBand;

/// Every EPF band applicable on a date, across all categories.
///
/// `find_contribution` resolves one wage with one round trip, which a payroll
/// run would repeat once per employee. This loads the whole schedule so the run
/// can resolve every employee in memory; the filters are identical, so the two
/// paths select from the same rows.
pub async fn list_bands(
    executor: impl Executor<'_, Database = Postgres>,
    effective_date: NaiveDate,
) -> AppResult<Vec<EpfBand>> {
    let bands = sqlx::query_as!(
        EpfBand,
        r#"
        SELECT rates.category, rates.wage_from, rates.wage_to,
               rates.employee_contribution, rates.employer_contribution,
               rates.effective_from
        FROM epf_rates rates
        JOIN statutory_rule_sets rules ON rules.id = rates.rule_set_id
        WHERE rules.rule_code = 'epf'
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
