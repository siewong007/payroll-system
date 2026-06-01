//! Data access for the `payroll_entries` table (staged variable earnings/deductions).

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

/// Mark an employee's staged entries for a period as processed, attaching the run id.
pub async fn mark_processed(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    employee_id: Uuid,
    year: i32,
    month: i32,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE payroll_entries SET is_processed = TRUE, payroll_run_id = $1
        WHERE employee_id = $2 AND period_year = $3 AND period_month = $4 AND is_processed = FALSE"#,
        run_id,
        employee_id,
        year,
        month,
    )
    .execute(executor)
    .await?;
    Ok(())
}
