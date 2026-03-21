use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::notification::{Notification, NotificationCount};

pub async fn get_notifications(
    pool: &PgPool,
    user_id: Uuid,
    unread_only: bool,
    limit: i64,
) -> AppResult<Vec<Notification>> {
    let notifications = sqlx::query_as::<_, Notification>(
        r#"SELECT * FROM notifications
        WHERE user_id = $1 AND ($2 = FALSE OR is_read = FALSE)
        ORDER BY created_at DESC
        LIMIT $3"#,
    )
    .bind(user_id)
    .bind(unread_only)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(notifications)
}

pub async fn get_notification_count(
    pool: &PgPool,
    user_id: Uuid,
) -> AppResult<NotificationCount> {
    let unread: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND is_read = FALSE")
            .bind(user_id)
            .fetch_one(pool)
            .await?;

    let total: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;

    Ok(NotificationCount {
        unread: unread.0,
        total: total.0,
    })
}

pub async fn mark_as_read(pool: &PgPool, user_id: Uuid, notification_id: Uuid) -> AppResult<()> {
    sqlx::query(
        "UPDATE notifications SET is_read = TRUE, read_at = NOW() WHERE id = $1 AND user_id = $2",
    )
    .bind(notification_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_all_read(pool: &PgPool, user_id: Uuid) -> AppResult<()> {
    sqlx::query(
        "UPDATE notifications SET is_read = TRUE, read_at = NOW() WHERE user_id = $1 AND is_read = FALSE",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn create_notification(
    pool: &PgPool,
    user_id: Uuid,
    company_id: Uuid,
    notification_type: &str,
    title: &str,
    message: &str,
    entity_type: Option<&str>,
    entity_id: Option<Uuid>,
) -> AppResult<Notification> {
    let notification = sqlx::query_as::<_, Notification>(
        r#"INSERT INTO notifications (user_id, company_id, notification_type, title, message, entity_type, entity_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *"#,
    )
    .bind(user_id)
    .bind(company_id)
    .bind(notification_type)
    .bind(title)
    .bind(message)
    .bind(entity_type)
    .bind(entity_id)
    .fetch_one(pool)
    .await?;
    Ok(notification)
}

/// Notify all admin/hr users in the company
pub async fn notify_admins(
    pool: &PgPool,
    company_id: Uuid,
    notification_type: &str,
    title: &str,
    message: &str,
    entity_type: Option<&str>,
    entity_id: Option<Uuid>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO notifications (user_id, company_id, notification_type, title, message, entity_type, entity_id)
        SELECT id, company_id, $2, $3, $4, $5, $6
        FROM users
        WHERE company_id = $1 AND role IN ('super_admin', 'payroll_admin', 'hr_manager') AND is_active = TRUE"#,
    )
    .bind(company_id)
    .bind(notification_type)
    .bind(title)
    .bind(message)
    .bind(entity_type)
    .bind(entity_id)
    .execute(pool)
    .await?;
    Ok(())
}
