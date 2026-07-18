//! Data access for the `pcb_reliefs` table (tax relief / rebate amounts).

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;

/// Relief/rebate amount (in sen) for a relief type in a tax year, if configured.
pub async fn get_amount(
    executor: impl Executor<'_, Database = Postgres>,
    relief_type: &str,
    tax_year: i32,
    effective_date: NaiveDate,
) -> AppResult<Option<i64>> {
    let amount = sqlx::query_scalar!(
        r#"
        SELECT reliefs.amount
        FROM pcb_reliefs reliefs
        JOIN statutory_rule_sets rules ON rules.id = reliefs.rule_set_id
        WHERE rules.rule_code = 'pcb'
          AND rules.status = 'verified'
          AND rules.effective_from <= $3
          AND (rules.effective_to IS NULL OR rules.effective_to >= $3)
          AND reliefs.relief_type = $1
          AND reliefs.effective_year = $2
        "#,
        relief_type,
        tax_year,
        effective_date,
    )
    .fetch_optional(executor)
    .await?;
    Ok(amount)
}
