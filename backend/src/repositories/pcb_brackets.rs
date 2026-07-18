//! Data access for the `pcb_brackets` table (progressive tax brackets).

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;
use crate::models::statutory::PcbBracketLookup;

/// All tax brackets for a year, ascending by income floor.
pub async fn list_for_year(
    executor: impl Executor<'_, Database = Postgres>,
    tax_year: i32,
    effective_date: NaiveDate,
) -> AppResult<Vec<PcbBracketLookup>> {
    let brackets = sqlx::query_as!(
        PcbBracketLookup,
        r#"
        SELECT brackets.chargeable_income_from, brackets.chargeable_income_to,
               brackets.tax_rate_percent, brackets.cumulative_tax
        FROM pcb_brackets brackets
        JOIN statutory_rule_sets rules ON rules.id = brackets.rule_set_id
        WHERE rules.rule_code = 'pcb'
          AND rules.status = 'verified'
          AND rules.effective_from <= $2
          AND (rules.effective_to IS NULL OR rules.effective_to >= $2)
          AND brackets.effective_year = $1
        ORDER BY brackets.chargeable_income_from ASC
        "#,
        tax_year,
        effective_date,
    )
    .fetch_all(executor)
    .await?;
    Ok(brackets)
}
