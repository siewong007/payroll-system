//! Data access for the `email_logs` table.
//
// NOTE: the send-status updates keep their original (over-)indentation so their
// text is byte-identical to the offline `.sqlx` cache (they were nested inside
// the SMTP send if/match in email_service).

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::email::EmailLog;

pub async fn count(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Option<Uuid>,
) -> AppResult<i64> {
    let total = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM email_logs
        WHERE company_id = $1 AND ($2::uuid IS NULL OR employee_id = $2)"#,
        company_id,
        employee_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(total)
}

pub async fn list(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Option<Uuid>,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<EmailLog>> {
    let logs = sqlx::query_as!(
        EmailLog,
        r#"SELECT * FROM email_logs
        WHERE company_id = $1 AND ($2::uuid IS NULL OR employee_id = $2)
        ORDER BY created_at DESC
        LIMIT $3 OFFSET $4"#,
        company_id,
        employee_id,
        limit,
        offset,
    )
    .fetch_all(executor)
    .await?;
    Ok(logs)
}

/// Record a new email as `pending` before the SMTP send is attempted.
#[allow(clippy::too_many_arguments)]
pub async fn insert_pending(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Option<Uuid>,
    template_id: Option<Uuid>,
    letter_type: &str,
    recipient_email: &str,
    recipient_name: &str,
    subject: &str,
    body_html: &str,
    created_by: Uuid,
) -> AppResult<EmailLog> {
    let log = sqlx::query_as!(
        EmailLog,
        r#"INSERT INTO email_logs
        (company_id, employee_id, template_id, letter_type, recipient_email, recipient_name, subject, body_html, status, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending', $9)
        RETURNING *"#,
        company_id,
        employee_id,
        template_id,
        letter_type,
        recipient_email,
        recipient_name,
        subject,
        body_html,
        created_by,
    )
    .fetch_one(executor)
    .await?;
    Ok(log)
}

/// Mark a log `failed` because SMTP is not configured.
pub async fn mark_failed_not_configured(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<EmailLog> {
    let log = sqlx::query_as!(
        EmailLog,
        r#"UPDATE email_logs SET status = 'failed', error_message = 'SMTP not configured'
            WHERE id = $1 RETURNING *"#,
        id,
    )
    .fetch_one(executor)
    .await?;
    Ok(log)
}

/// Mark a log `sent` after a successful SMTP send.
pub async fn mark_sent(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<EmailLog> {
    let log = sqlx::query_as!(
        EmailLog,
        r#"UPDATE email_logs SET status = 'sent', sent_at = NOW()
                WHERE id = $1 RETURNING *"#,
        id,
    )
    .fetch_one(executor)
    .await?;
    Ok(log)
}

/// Mark a log `failed` with the SMTP error message.
pub async fn mark_failed(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    error_message: &str,
) -> AppResult<EmailLog> {
    let log = sqlx::query_as!(
        EmailLog,
        r#"UPDATE email_logs SET status = 'failed', error_message = $2
                WHERE id = $1 RETURNING *"#,
        id,
        error_message,
    )
    .fetch_one(executor)
    .await?;
    Ok(log)
}
