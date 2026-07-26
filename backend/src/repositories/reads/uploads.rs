//! Reverse lookup from an uploaded file's URL to the records that reference it.
//!
//! `/api/uploads/{filename}` is backed by a bare directory, not a table, so
//! there is no row to read an owner off. A file is reachable through exactly
//! three columns — `claims.receipt_url`, `leave_requests.attachment_url` and
//! `documents.file_url` — and authorizing a download means asking all three, in
//! the caller's company, which employee the file belongs to.
//!
//! Adding a fourth column that stores an upload URL means adding a branch here,
//! or the file it points at becomes unreadable.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::upload::{UploadReference, UploadReferenceKind};

/// Every record in `company_id` that references `file_url`.
///
/// Scoped to one company by construction: a caller can never learn that a file
/// exists in another tenant, because rows outside their company are never
/// considered. An empty result means "no record here points at this file",
/// which the service treats identically to "no such file".
pub async fn find_references(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    file_url: &str,
) -> AppResult<Vec<UploadReference>> {
    // The literals are cast explicitly: an uncast string literal is `unknown` to
    // Postgres, which the query macro cannot map to a Rust type.
    let rows = sqlx::query!(
        r#"SELECT 'claim'::text AS "kind!", employee_id AS "employee_id?"
           FROM claims
           WHERE company_id = $1 AND receipt_url = $2
        UNION ALL
        SELECT 'leave_request'::text, employee_id
           FROM leave_requests
           WHERE company_id = $1 AND attachment_url = $2
        UNION ALL
        SELECT 'document'::text, employee_id
           FROM documents
           WHERE company_id = $1 AND file_url = $2 AND deleted_at IS NULL"#,
        company_id,
        file_url,
    )
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            UploadReferenceKind::from_wire(&row.kind).map(|kind| UploadReference {
                kind,
                employee_id: row.employee_id,
            })
        })
        .collect())
}
