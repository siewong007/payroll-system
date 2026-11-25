use crate::core::error::AppResult;
use crate::models::notification::{Notification, NotificationCount};
use crate::repositories::notifications;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn get_notifications(
    pool: &PgPool,
    user_id: Uuid,
    company_id: Uuid,
    unread_only: bool,
    limit: i64,
) -> AppResult<Vec<Notification>> {
    notifications::list(pool, user_id, company_id, unread_only, limit).await
}

pub async fn get_notification_count(
    pool: &PgPool,
    user_id: Uuid,
    company_id: Uuid,
) -> AppResult<NotificationCount> {
    let unread = notifications::count_unread(pool, user_id, company_id).await?;
    let total = notifications::count_total(pool, user_id, company_id).await?;
    Ok(NotificationCount { unread, total })
}

pub async fn mark_as_read(
    pool: &PgPool,
    user_id: Uuid,
    company_id: Uuid,
    notification_id: Uuid,
) -> AppResult<()> {
    notifications::mark_read(pool, notification_id, user_id, company_id).await
}

pub async fn mark_all_read(pool: &PgPool, user_id: Uuid, company_id: Uuid) -> AppResult<()> {
    notifications::mark_all_read(pool, user_id, company_id).await
}

#[allow(clippy::too_many_arguments)]
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
    notifications::insert(
        pool,
        user_id,
        company_id,
        notification_type,
        title,
        message,
        entity_type,
        entity_id,
    )
    .await
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
    notifications::insert_for_admins(
        pool,
        company_id,
        notification_type,
        title,
        message,
        entity_type,
        entity_id,
    )
    .await
}
