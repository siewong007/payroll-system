//! Data access for the `push_outbox` delivery queue.

use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::notification::PushOutboxBatchRow;

#[allow(clippy::too_many_arguments)]
pub async fn enqueue_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    company_id: Option<Uuid>,
    notification_id: Option<Uuid>,
    title: &str,
    body: &str,
    url: &str,
    tag: Option<&str>,
) -> AppResult<u64> {
    let res = sqlx::query!(
        r#"INSERT INTO push_outbox (
            subscription_id, user_id, company_id, notification_id, title, body, url, tag
        )
        SELECT ps.id, ps.user_id, $2, $3, $4, $5, $6, $7
        FROM push_subscriptions ps
        JOIN users u ON u.id = ps.user_id
        LEFT JOIN user_sessions us ON us.id = ps.session_id
        WHERE ps.user_id = $1
          AND ps.revoked_at IS NULL
          AND u.is_active = TRUE
          AND u.deleted_at IS NULL
          AND (ps.session_id IS NULL OR (us.revoked_at IS NULL AND us.expires_at > NOW()))
        ORDER BY ps.last_seen_at DESC NULLS LAST, ps.updated_at DESC, ps.id ASC
        LIMIT 20"#,
        user_id,
        company_id,
        notification_id,
        title,
        body,
        url,
        tag,
    )
    .execute(executor)
    .await?;
    Ok(res.rows_affected())
}

pub async fn enqueue_for_notifications(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    notification_ids: &[Uuid],
    title: &str,
    body: &str,
    url: &str,
    tag: Option<&str>,
) -> AppResult<u64> {
    if notification_ids.is_empty() {
        return Ok(0);
    }
    let res = sqlx::query!(
        r#"WITH eligible_subs AS (
            SELECT ps.id, ps.user_id,
                   ROW_NUMBER() OVER (
                       PARTITION BY ps.user_id
                       ORDER BY ps.last_seen_at DESC NULLS LAST, ps.updated_at DESC, ps.id ASC
                   ) AS rn
            FROM push_subscriptions ps
            JOIN users u ON u.id = ps.user_id
            LEFT JOIN user_sessions us ON us.id = ps.session_id
            WHERE ps.revoked_at IS NULL
              AND u.is_active = TRUE
              AND u.deleted_at IS NULL
              AND (ps.session_id IS NULL OR (us.revoked_at IS NULL AND us.expires_at > NOW()))
        )
        INSERT INTO push_outbox (
            subscription_id, user_id, company_id, notification_id, title, body, url, tag
        )
        SELECT es.id, n.user_id, $1, n.id, $3, $4, $5, $6
        FROM notifications n
        JOIN eligible_subs es ON es.user_id = n.user_id AND es.rn <= 20
        WHERE n.id = ANY($2)
          AND n.company_id = $1"#,
        company_id,
        notification_ids,
        title,
        body,
        url,
        tag,
    )
    .execute(executor)
    .await?;
    Ok(res.rows_affected())
}

/// Claim a batch of due pending push rows using `FOR UPDATE SKIP LOCKED` and a
/// short 5-minute lease so concurrent workers do not double-send while HTTP I/O
/// is in flight.
pub async fn claim_pending_batch(pool: &PgPool, limit: i64) -> AppResult<Vec<PushOutboxBatchRow>> {
    let mut tx = pool.begin().await?;
    let rows = sqlx::query_as!(
        PushOutboxBatchRow,
        r#"WITH ranked AS (
            SELECT po.id, po.next_retry_at, po.created_at,
                   ROW_NUMBER() OVER (
                       PARTITION BY po.user_id
                       ORDER BY po.next_retry_at ASC, po.created_at ASC, po.id ASC
                   ) AS rn
            FROM push_outbox po
            WHERE po.status = 'pending'
              AND po.next_retry_at <= NOW()
        ),
        candidates AS (
            SELECT id, rn
            FROM ranked
            WHERE rn <= 5
            ORDER BY rn ASC, next_retry_at ASC, created_at ASC, id ASC
            LIMIT ($1::bigint * 4)
        ),
        claimed AS (
            SELECT po.id
            FROM push_outbox po
            JOIN candidates c ON c.id = po.id
            WHERE po.status = 'pending'
              AND po.next_retry_at <= NOW()
            ORDER BY c.rn ASC, po.next_retry_at ASC, po.created_at ASC, po.id ASC
            LIMIT $1
            FOR UPDATE OF po SKIP LOCKED
        ),
        leased AS (
            UPDATE push_outbox po
            SET next_retry_at = NOW() + INTERVAL '5 minutes'
            FROM claimed c
            WHERE po.id = c.id
            RETURNING po.id, po.subscription_id, po.user_id, po.company_id,
                      po.title, po.body, po.url, po.tag, po.attempts, po.max_attempts
        )
        SELECT l.id, l.subscription_id, l.user_id, l.company_id,
               l.title, l.body, l.url, l.tag, l.attempts, l.max_attempts,
               ps.endpoint, ps.p256dh, ps.auth,
               ps.revoked_at IS NOT NULL
                   OR ps.user_id <> l.user_id
                   OR u.is_active IS NOT TRUE
                   OR u.deleted_at IS NOT NULL
                   OR (ps.session_id IS NOT NULL AND (us.revoked_at IS NOT NULL OR us.expires_at <= NOW()))
                   AS "is_revoked!",
               (SELECT COUNT(*) FROM notifications n
                WHERE n.user_id = l.user_id
                  AND (l.company_id IS NULL OR n.company_id = l.company_id)
                  AND n.is_read = FALSE) AS "unread_count!"
        FROM leased l
        JOIN push_subscriptions ps ON ps.id = l.subscription_id
        JOIN users u ON u.id = l.user_id
        LEFT JOIN user_sessions us ON us.id = ps.session_id
        ORDER BY l.id"#,
        limit,
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows)
}

pub async fn mark_sent(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE push_outbox
         SET status = 'sent', attempts = attempts + 1, sent_at = NOW(), last_error = NULL
         WHERE id = $1",
        id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn mark_gone(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    error: &str,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE push_outbox
         SET status = 'gone', attempts = attempts + 1, last_error = $2
         WHERE id = $1",
        id,
        error,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn mark_retry(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    attempts: i32,
    next_retry_at: DateTime<Utc>,
    error: &str,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE push_outbox
         SET status = 'pending', attempts = $2, next_retry_at = $3, last_error = $4
         WHERE id = $1",
        id,
        attempts,
        next_retry_at,
        error,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn mark_failed(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    attempts: i32,
    error: &str,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE push_outbox
         SET status = 'failed', attempts = $2, last_error = $3
         WHERE id = $1",
        id,
        attempts,
        error,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn purge_old(
    executor: impl Executor<'_, Database = Postgres>,
    retention_days: i32,
) -> AppResult<u64> {
    let res = sqlx::query(
        r#"DELETE FROM push_outbox
           WHERE created_at < NOW() - ($1 || ' days')::interval"#,
    )
    .bind(retention_days.to_string())
    .execute(executor)
    .await?;
    Ok(res.rows_affected())
}
