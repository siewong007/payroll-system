//! Data access for the `notifications` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::notification::Notification;

/// A user's notifications, newest first; `unread_only` filters to unread.
pub async fn list(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    unread_only: bool,
    limit: i64,
) -> AppResult<Vec<Notification>> {
    let notifications = sqlx::query_as!(
        Notification,
        r#"SELECT * FROM notifications
        WHERE user_id = $1 AND ($2 = FALSE OR is_read = FALSE)
        ORDER BY created_at DESC
        LIMIT $3"#,
        user_id,
        unread_only,
        limit,
    )
    .fetch_all(executor)
    .await?;
    Ok(notifications)
}

pub async fn count_unread(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<i64> {
    let unread = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM notifications WHERE user_id = $1 AND is_read = FALSE"#,
        user_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(unread)
}

pub async fn count_total(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<i64> {
    let total = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM notifications WHERE user_id = $1"#,
        user_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(total)
}

pub async fn mark_read(
    executor: impl Executor<'_, Database = Postgres>,
    notification_id: Uuid,
    user_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE notifications SET is_read = TRUE, read_at = NOW() WHERE id = $1 AND user_id = $2",
        notification_id,
        user_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn mark_all_read(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE notifications SET is_read = TRUE, read_at = NOW() WHERE user_id = $1 AND is_read = FALSE",
        user_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    company_id: Uuid,
    notification_type: &str,
    title: &str,
    message: &str,
    entity_type: Option<&str>,
    entity_id: Option<Uuid>,
) -> AppResult<Notification> {
    let notification = sqlx::query_as!(
        Notification,
        r#"INSERT INTO notifications (user_id, company_id, notification_type, title, message, entity_type, entity_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *"#,
        user_id,
        company_id,
        notification_type,
        title,
        message,
        entity_type,
        entity_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(notification)
}

/// Fan out one notification to every active admin/HR user in the company.
pub async fn insert_for_admins(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    notification_type: &str,
    title: &str,
    message: &str,
    entity_type: Option<&str>,
    entity_id: Option<Uuid>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO notifications (user_id, company_id, notification_type, title, message, entity_type, entity_id)
        SELECT id, company_id, $2, $3, $4, $5, $6
        FROM users
        WHERE company_id = $1
            AND roles && ARRAY['super_admin', 'payroll_admin', 'hr_manager']::VARCHAR(50)[]
            AND is_active = TRUE"#,
        company_id,
        notification_type,
        title,
        message,
        entity_type,
        entity_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}
