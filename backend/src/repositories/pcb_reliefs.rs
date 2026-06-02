//! Data access for the `pcb_reliefs` table (tax relief / rebate amounts).

use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;

/// Relief/rebate amount (in sen) for a relief type in a tax year, if configured.
pub async fn get_amount(
    executor: impl Executor<'_, Database = Postgres>,
    relief_type: &str,
    tax_year: i32,
) -> AppResult<Option<i64>> {
    let amount = sqlx::query_scalar!(
        "SELECT amount FROM pcb_reliefs WHERE relief_type = $1 AND effective_year = $2",
        relief_type,
        tax_year,
    )
    .fetch_optional(executor)
    .await?;
    Ok(amount)
}
