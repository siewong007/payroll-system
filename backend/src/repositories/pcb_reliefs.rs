//! Data access for the `pcb_reliefs` table (tax relief / rebate amounts).

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;

/// Every relief/rebate amount configured for a tax year, keyed by relief type.
///
/// A single PCB calculation reads six reliefs; `get_amount` makes that six round
/// trips per employee. The run-scoped snapshot reads them all once.
pub async fn list_for_year(
    executor: impl Executor<'_, Database = Postgres>,
    tax_year: i32,
    effective_date: NaiveDate,
) -> AppResult<Vec<(String, i64)>> {
    let rows = sqlx::query!(
        r#"
        SELECT reliefs.relief_type, reliefs.amount
        FROM pcb_reliefs reliefs
        JOIN statutory_rule_sets rules ON rules.id = reliefs.rule_set_id
        WHERE rules.rule_code = 'pcb'
          AND rules.status = 'verified'
          AND rules.effective_from <= $2
          AND (rules.effective_to IS NULL OR rules.effective_to >= $2)
          AND reliefs.effective_year = $1
        "#,
        tax_year,
        effective_date,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.relief_type, r.amount))
        .collect())
}
