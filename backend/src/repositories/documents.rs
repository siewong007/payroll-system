//! Data access for the `documents` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

/// Count of non-deleted documents for a company.
pub async fn count_active(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM documents WHERE company_id = $1 AND deleted_at IS NULL"#,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}
