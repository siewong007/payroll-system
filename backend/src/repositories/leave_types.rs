//! Data access for the `leave_types` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

/// Whether an active leave type with this id exists in the company.
pub async fn exists_active(
    executor: impl Executor<'_, Database = Postgres>,
    leave_type_id: Uuid,
    company_id: Uuid,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(
            SELECT 1 FROM leave_types
            WHERE id = $1 AND company_id = $2 AND is_active = TRUE
        ) AS "exists!""#,
        leave_type_id,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// The `is_paid` flag for a leave type, or `None` if the type does not exist.
pub async fn get_is_paid(
    executor: impl Executor<'_, Database = Postgres>,
    leave_type_id: Uuid,
) -> AppResult<Option<bool>> {
    let is_paid = sqlx::query_scalar!(
        "SELECT is_paid FROM leave_types WHERE id = $1",
        leave_type_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(is_paid)
}
