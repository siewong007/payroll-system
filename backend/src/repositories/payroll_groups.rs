//! Data access for the `payroll_groups` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::payroll::PayrollGroup;

/// Active payroll groups for a company.
pub async fn list_active(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<PayrollGroup>> {
    let groups = sqlx::query_as!(
        PayrollGroup,
        "SELECT * FROM payroll_groups WHERE company_id = $1 AND is_active = TRUE ORDER BY name",
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(groups)
}

/// Count of active payroll groups for a company.
pub async fn count_active(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM payroll_groups WHERE company_id = $1 AND is_active = TRUE"#,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}

/// Every payroll group ID belonging to `company_id`, for import validation.
pub async fn list_ids_for_company(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<Uuid>> {
    let ids = sqlx::query_scalar!(
        "SELECT id FROM payroll_groups WHERE company_id = $1",
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(ids)
}
