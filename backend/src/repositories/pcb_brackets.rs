//! Data access for the `pcb_brackets` table (progressive tax brackets).

use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;
use crate::models::statutory::PcbBracketLookup;

/// All tax brackets for a year, ascending by income floor.
pub async fn list_for_year(
    executor: impl Executor<'_, Database = Postgres>,
    tax_year: i32,
) -> AppResult<Vec<PcbBracketLookup>> {
    let brackets = sqlx::query_as!(
        PcbBracketLookup,
        r#"
        SELECT chargeable_income_from, chargeable_income_to, tax_rate_percent, cumulative_tax
        FROM pcb_brackets
        WHERE effective_year = $1
        ORDER BY chargeable_income_from ASC
        "#,
        tax_year,
    )
    .fetch_all(executor)
    .await?;
    Ok(brackets)
}
