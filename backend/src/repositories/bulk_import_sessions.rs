//! Data access for the `bulk_import_sessions` table (staged employee imports).

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

use crate::models::employee_import::ImportSession;

pub async fn get(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<Option<ImportSession>> {
    let row = sqlx::query!(
        r#"SELECT company_id, user_id, file_name, validated_data, status, expires_at
            FROM bulk_import_sessions WHERE id = $1"#,
        id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|r| ImportSession {
        company_id: r.company_id,
        user_id: r.user_id,
        file_name: r.file_name,
        validated_data: r.validated_data,
        status: r.status,
        expires_at: r.expires_at,
    }))
}

/// Stage a validated import as a `pending` session.
#[allow(clippy::too_many_arguments)]
pub async fn insert_pending(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    user_id: Uuid,
    file_name: &str,
    row_count: i32,
    valid_count: i32,
    validated_data: serde_json::Value,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO bulk_import_sessions (id, company_id, user_id, file_name, row_count, valid_count, validated_data, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending')"#,
        id,
        company_id,
        user_id,
        file_name,
        row_count,
        valid_count,
        validated_data,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Claim a `pending` session for confirmation, returning false if it was
/// already claimed.
///
/// Compare-and-swap rather than a blind UPDATE: the caller's `status ==
/// "pending"` gate is a separate earlier statement, so two concurrent
/// confirmations of the same session both passed it and both imported the file.
/// Run inside the import transaction, this makes claiming the session and
/// writing the employees one atomic act — the previous code marked the session
/// confirmed on a *separate* connection after the commit, so a failed import
/// still burned the session and left no way to retry it.
pub async fn claim_for_confirmation(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<bool> {
    let result = sqlx::query!(
        r#"UPDATE bulk_import_sessions SET status = 'confirmed', confirmed_at = NOW()
        WHERE id = $1 AND status = 'pending'"#,
        id,
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}
