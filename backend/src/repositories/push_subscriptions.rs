//! Data access for browser/device Web Push subscriptions (`push_subscriptions`).

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::notification::PushSubscription;

#[allow(clippy::too_many_arguments)]
pub async fn upsert(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    company_id: Option<Uuid>,
    session_id: Option<Uuid>,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    user_agent: Option<&str>,
) -> AppResult<PushSubscription> {
    let row = sqlx::query_as!(
        PushSubscription,
        r#"INSERT INTO push_subscriptions
            (user_id, company_id, session_id, endpoint, p256dh, auth, user_agent)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (endpoint) DO UPDATE SET
            user_id = EXCLUDED.user_id,
            company_id = EXCLUDED.company_id,
            session_id = EXCLUDED.session_id,
            p256dh = EXCLUDED.p256dh,
            auth = EXCLUDED.auth,
            user_agent = COALESCE(EXCLUDED.user_agent, push_subscriptions.user_agent),
            failure_count = 0,
            last_error = NULL,
            revoked_at = NULL,
            last_seen_at = NOW(),
            updated_at = NOW()
        RETURNING id, user_id, company_id, session_id, endpoint, p256dh, auth, user_agent,
                  failure_count, last_error, last_seen_at, revoked_at, created_at, updated_at"#,
        user_id,
        company_id,
        session_id,
        endpoint,
        p256dh,
        auth,
        user_agent,
    )
    .fetch_one(executor)
    .await?;
    Ok(row)
}

pub async fn revoke_by_endpoint(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    endpoint: &str,
) -> AppResult<u64> {
    let res = sqlx::query!(
        "UPDATE push_subscriptions
         SET revoked_at = NOW(), updated_at = NOW()
         WHERE user_id = $1 AND endpoint = $2 AND revoked_at IS NULL",
        user_id,
        endpoint,
    )
    .execute(executor)
    .await?;
    Ok(res.rows_affected())
}

pub async fn revoke_by_id(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    reason: &str,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE push_subscriptions
         SET revoked_at = NOW(), last_error = $2, updated_at = NOW()
         WHERE id = $1 AND revoked_at IS NULL",
        id,
        reason,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn revoke_for_session(
    executor: impl Executor<'_, Database = Postgres>,
    session_id: Uuid,
) -> AppResult<u64> {
    let res = sqlx::query!(
        "UPDATE push_subscriptions
         SET revoked_at = NOW(), updated_at = NOW()
         WHERE session_id = $1 AND revoked_at IS NULL",
        session_id,
    )
    .execute(executor)
    .await?;
    Ok(res.rows_affected())
}

pub async fn revoke_for_revoked_sessions(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    keep_session_id: Uuid,
) -> AppResult<u64> {
    let res = sqlx::query!(
        "UPDATE push_subscriptions
         SET revoked_at = NOW(), updated_at = NOW()
         WHERE user_id = $1
           AND revoked_at IS NULL
           AND (session_id IS NULL OR session_id <> $2)",
        user_id,
        keep_session_id,
    )
    .execute(executor)
    .await?;
    Ok(res.rows_affected())
}

pub async fn revoke_all_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<u64> {
    let res = sqlx::query!(
        "UPDATE push_subscriptions
         SET revoked_at = NOW(), updated_at = NOW()
         WHERE user_id = $1 AND revoked_at IS NULL",
        user_id,
    )
    .execute(executor)
    .await?;
    Ok(res.rows_affected())
}

pub async fn update_company_for_session(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    session_id: Uuid,
    company_id: Uuid,
) -> AppResult<u64> {
    let res = sqlx::query!(
        "UPDATE push_subscriptions
         SET company_id = $3, last_seen_at = NOW(), updated_at = NOW()
         WHERE user_id = $1 AND session_id = $2 AND revoked_at IS NULL",
        user_id,
        session_id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(res.rows_affected())
}

pub async fn list_active_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<Vec<PushSubscription>> {
    let rows = sqlx::query_as!(
        PushSubscription,
        r#"SELECT ps.id, ps.user_id, ps.company_id, ps.session_id, ps.endpoint, ps.p256dh, ps.auth,
                  ps.user_agent, ps.failure_count, ps.last_error, ps.last_seen_at, ps.revoked_at,
                  ps.created_at, ps.updated_at
           FROM push_subscriptions ps
           JOIN users u ON u.id = ps.user_id
           LEFT JOIN user_sessions us ON us.id = ps.session_id
           WHERE ps.user_id = $1
             AND ps.revoked_at IS NULL
             AND u.is_active = TRUE
             AND u.deleted_at IS NULL
             AND (ps.session_id IS NULL OR (us.revoked_at IS NULL AND us.expires_at > NOW()))
           ORDER BY ps.last_seen_at DESC"#,
        user_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn count_active_for_user(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!"
           FROM push_subscriptions ps
           LEFT JOIN user_sessions us ON us.id = ps.session_id
           WHERE ps.user_id = $1
             AND ps.revoked_at IS NULL
             AND (ps.session_id IS NULL OR (us.revoked_at IS NULL AND us.expires_at > NOW()))"#,
        user_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}

pub async fn record_delivery_success(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE push_subscriptions
         SET failure_count = 0, last_error = NULL, last_seen_at = NOW(), updated_at = NOW()
         WHERE id = $1",
        id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn record_delivery_failure(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    error: &str,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE push_subscriptions
         SET failure_count = failure_count + 1,
             last_error = $2,
             revoked_at = CASE WHEN failure_count + 1 >= 10 THEN COALESCE(revoked_at, NOW()) ELSE revoked_at END,
             updated_at = NOW()
         WHERE id = $1",
        id,
        error,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn purge_stale(
    executor: impl Executor<'_, Database = Postgres>,
    revoked_retention_days: i32,
    inactive_retention_days: i32,
) -> AppResult<u64> {
    let res = sqlx::query(
        r#"DELETE FROM push_subscriptions
           WHERE (revoked_at IS NOT NULL AND revoked_at < NOW() - ($1 || ' days')::interval)
              OR last_seen_at < NOW() - ($2 || ' days')::interval"#,
    )
    .bind(revoked_retention_days.to_string())
    .bind(inactive_retention_days.to_string())
    .execute(executor)
    .await?;
    Ok(res.rows_affected())
}
