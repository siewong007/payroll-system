//! Data access for the `uploaded_files` ownership registry.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UploadedFileRow {
    pub stored_name: String,
    pub company_id: Uuid,
    pub uploaded_by: Option<Uuid>,
}

/// Record ownership of a newly written file under `uploads/`.
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    stored_name: &str,
    company_id: Uuid,
    uploaded_by: Option<Uuid>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO uploaded_files (stored_name, company_id, uploaded_by)
           VALUES ($1, $2, $3)
           ON CONFLICT (stored_name) DO NOTHING"#,
        stored_name,
        company_id,
        uploaded_by,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Look up the recorded tenant and uploader for a managed upload filename.
pub async fn find_by_stored_name(
    executor: impl Executor<'_, Database = Postgres>,
    stored_name: &str,
) -> AppResult<Option<UploadedFileRow>> {
    let row = sqlx::query_as!(
        UploadedFileRow,
        r#"SELECT stored_name, company_id, uploaded_by
           FROM uploaded_files
           WHERE stored_name = $1"#,
        stored_name,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}
