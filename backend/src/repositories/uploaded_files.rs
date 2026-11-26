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

/// Remove a managed upload metadata row once no record references it.
pub async fn delete_by_stored_name(
    executor: impl Executor<'_, Database = Postgres>,
    stored_name: &str,
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM uploaded_files WHERE stored_name = $1",
        stored_name,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// List all managed upload filenames owned by `company_id` (used before
/// deleting or overwriting a company so physical files can be unlinked post-commit).
pub async fn list_stored_names_for_company(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<String>> {
    let names = sqlx::query_scalar!(
        "SELECT stored_name FROM uploaded_files WHERE company_id = $1",
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(names)
}

/// Delete and return stale uploaded filenames older than `hours` that have no
/// referencing claim, leave request, active document, or company logo.
pub async fn purge_unreferenced_older_than_hours(
    executor: impl Executor<'_, Database = Postgres>,
    hours: i32,
) -> AppResult<Vec<String>> {
    let names = sqlx::query_scalar!(
        r#"DELETE FROM uploaded_files uf
           WHERE uf.created_at < NOW() - make_interval(hours => $1)
             AND NOT EXISTS (
                 SELECT 1 FROM claims c
                 WHERE c.company_id = uf.company_id
                   AND c.receipt_url = '/api/uploads/' || uf.stored_name
             )
             AND NOT EXISTS (
                 SELECT 1 FROM leave_requests lr
                 WHERE lr.company_id = uf.company_id
                   AND lr.attachment_url = '/api/uploads/' || uf.stored_name
             )
             AND NOT EXISTS (
                 SELECT 1 FROM documents d
                 WHERE d.company_id = uf.company_id
                   AND d.file_url = '/api/uploads/' || uf.stored_name
                   AND d.deleted_at IS NULL
             )
             AND NOT EXISTS (
                 SELECT 1 FROM companies co
                 WHERE co.id = uf.company_id
                   AND co.logo_url = '/api/uploads/' || uf.stored_name
             )
           RETURNING uf.stored_name"#,
        hours,
    )
    .fetch_all(executor)
    .await?;
    Ok(names)
}
