//! Data access for the `claims` table.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

/// Mark an employee's approved claims within a period as processed (paid via payroll).
pub async fn mark_processed(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    company_id: Uuid,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> AppResult<()> {
    // NOTE: indentation matches the byte-exact SQL in the offline `.sqlx` cache
    // (this UPDATE was originally nested inside an `if`, hence the deeper indent).
    sqlx::query!(
        r#"UPDATE claims SET status = 'processed', updated_at = NOW()
            WHERE employee_id = $1 AND company_id = $2
              AND status = 'approved'
              AND expense_date >= $3 AND expense_date <= $4"#,
        employee_id,
        company_id,
        period_start,
        period_end,
    )
    .execute(executor)
    .await?;
    Ok(())
}
