//! Data access for the `email_logs` table.
//
// NOTE: the send-status updates keep their original (over-)indentation so their
// text is byte-identical to the offline `.sqlx` cache (they were nested inside
// the SMTP send if/match in email_service).

use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::email::{EmailLog, EmailLogSummary};

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

/// One page of log rows *without* their bodies.
///
/// The columns are listed explicitly on purpose: the previous `SELECT *` meant
/// `body_html` started being returned the moment the column existed, and the
/// welcome letter's body contains the account's initial password.
pub async fn list(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Option<Uuid>,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<EmailLogSummary>> {
    let logs = sqlx::query_as!(
        EmailLogSummary,
        r#"SELECT id, company_id, employee_id, template_id, letter_type,
            recipient_email, recipient_name, subject, status, error_message,
            sent_at, created_at, created_by
        FROM email_logs
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

/// Record a new email before SMTP send or outbox retry.
#[allow(clippy::too_many_arguments)]
pub async fn insert_with_delivery_state(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Option<Uuid>,
    template_id: Option<Uuid>,
    letter_type: &str,
    recipient_email: &str,
    recipient_name: &str,
    subject: &str,
    body_html: &str,
    status: &str,
    error_message: Option<&str>,
    attempts: i32,
    max_attempts: i32,
    next_retry_at: Option<DateTime<Utc>>,
    created_by: Uuid,
) -> AppResult<EmailLog> {
    let log = sqlx::query_as!(
        EmailLog,
        r#"INSERT INTO email_logs
        (company_id, employee_id, template_id, letter_type, recipient_email, recipient_name,
         subject, body_html, status, error_message, attempts, max_attempts, next_retry_at, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        RETURNING *"#,
        company_id,
        employee_id,
        template_id,
        letter_type,
        recipient_email,
        recipient_name,
        subject,
        body_html,
        status,
        error_message,
        attempts,
        max_attempts,
        next_retry_at,
        created_by,
    )
    .fetch_one(executor)
    .await?;
    Ok(log)
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
    insert_with_delivery_state(
        executor,
        company_id,
        employee_id,
        template_id,
        letter_type,
        recipient_email,
        recipient_name,
        subject,
        body_html,
        "pending",
        None,
        0,
        3,
        None,
        created_by,
    )
    .await
}

/// Mark a log `failed` because SMTP is not configured.
pub async fn mark_failed_not_configured(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<EmailLog> {
    let log = sqlx::query_as!(
        EmailLog,
        r#"UPDATE email_logs
           SET status = 'failed', error_message = 'SMTP not configured', next_retry_at = NULL
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
) -> AppResult<Option<EmailLog>> {
    let log = sqlx::query_as!(
        EmailLog,
        r#"UPDATE email_logs
           SET status = 'sent', sent_at = NOW(), error_message = NULL,
               next_retry_at = NULL, attempts = GREATEST(attempts, 1)
           WHERE id = $1 AND status = 'pending' RETURNING *"#,
        id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(log)
}

/// Schedule an automatic retry for a queued email after an SMTP failure.
pub async fn schedule_retry(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    attempts: i32,
    next_retry_at: DateTime<Utc>,
    error_message: &str,
) -> AppResult<Option<EmailLog>> {
    let log = sqlx::query_as!(
        EmailLog,
        r#"UPDATE email_logs
           SET status = 'pending', attempts = $2, next_retry_at = $3, error_message = $4
           WHERE id = $1 AND status = 'pending' RETURNING *"#,
        id,
        attempts,
        next_retry_at,
        error_message,
    )
    .fetch_optional(executor)
    .await?;
    Ok(log)
}

/// Mark a log `failed` with the SMTP error message.
pub async fn mark_failed(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    error_message: &str,
) -> AppResult<Option<EmailLog>> {
    let log = sqlx::query_as!(
        EmailLog,
        r#"UPDATE email_logs
           SET status = 'failed', error_message = $2, next_retry_at = NULL
           WHERE id = $1 AND status = 'pending' RETURNING *"#,
        id,
        error_message,
    )
    .fetch_optional(executor)
    .await?;
    Ok(log)
}

/// Claim up to `limit` due outbox rows using `FOR UPDATE SKIP LOCKED`, bumping
/// `attempts` and pushing `next_retry_at` 5 minutes forward as a visibility
/// lease so crashed workers do not drop retries and concurrent workers never
/// double-send.
pub async fn claim_due_outbox(pool: &PgPool, limit: i64) -> AppResult<Vec<EmailLog>> {
    let mut tx = pool.begin().await?;
    // Any row whose final-attempt worker crashed before recording completion
    // must not stay stuck in 'pending' forever.
    sqlx::query!(
        r#"UPDATE email_logs
           SET status = 'failed',
               next_retry_at = NULL,
               error_message = COALESCE(error_message, 'Delivery worker lease expired on final attempt')
           WHERE status = 'pending'
             AND next_retry_at IS NOT NULL
             AND next_retry_at <= NOW()
             AND attempts >= max_attempts"#
    )
    .execute(&mut *tx)
    .await?;
    let rows = sqlx::query_as!(
        EmailLog,
        r#"WITH claimed AS (
               SELECT id
               FROM email_logs
               WHERE status = 'pending'
                 AND next_retry_at IS NOT NULL
                 AND next_retry_at <= NOW()
                 AND attempts < max_attempts
                 AND max_attempts > 1
               ORDER BY next_retry_at ASC, created_at ASC
               LIMIT $1
               FOR UPDATE SKIP LOCKED
           )
           UPDATE email_logs e
           SET attempts = e.attempts + 1,
               next_retry_at = NOW() + INTERVAL '5 minutes'
           FROM claimed
           WHERE e.id = claimed.id
           RETURNING e.*"#,
        limit,
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows)
}

/// Delete email log rows older than the retention window (plan item 26).
pub async fn purge_older_than(
    executor: impl Executor<'_, Database = Postgres>,
    days: i32,
) -> AppResult<u64> {
    let result =
        sqlx::query("DELETE FROM email_logs WHERE created_at < NOW() - ($1 || ' days')::interval")
            .bind(days.to_string())
            .execute(executor)
            .await?;
    Ok(result.rows_affected())
}
