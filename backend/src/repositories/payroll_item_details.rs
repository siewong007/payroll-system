//! Data access for the `payroll_item_details` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

/// Delete all detail rows belonging to a run's payslip items.
pub async fn delete_for_run(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"DELETE FROM payroll_item_details pid
        USING payroll_items pi
        WHERE pid.payroll_item_id = pi.id
          AND pi.payroll_run_id = $1"#,
        run_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}
