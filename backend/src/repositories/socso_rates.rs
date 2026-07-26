//! Data access for the `socso_rates` table.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;
use crate::models::statutory::SocsoBand;

/// Every SOCSO band applicable on a date, newest schedule first.
///
/// Schedule selection and the wage-ceiling clamp that `find_rate` does in SQL are
/// applied by the caller (`services::statutory_tables`) so a payroll run resolves
/// all employees from one read rather than one read each.
pub async fn list_bands(
    executor: impl Executor<'_, Database = Postgres>,
    effective_date: NaiveDate,
) -> AppResult<Vec<SocsoBand>> {
    let bands = sqlx::query_as!(
        SocsoBand,
        r#"
        SELECT rates.wage_from, rates.wage_to, rates.first_cat_employee,
               rates.first_cat_employer, rates.second_cat_employer,
               rates.effective_from
        FROM socso_rates rates
        JOIN statutory_rule_sets rules ON rules.id = rates.rule_set_id
        WHERE rules.rule_code = 'socso'
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
