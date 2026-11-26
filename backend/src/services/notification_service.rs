use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::notification::{
    Notification, NotificationCount, PushConfigResponse, PushSubscriptionStatusResponse,
    UpsertPushSubscriptionRequest,
};
use crate::repositories::{notifications, push_outbox, push_subscriptions};
use crate::services::push_service::{self, SharedPushClient};

pub fn notification_target_url(notification_type: &str, entity_type: Option<&str>) -> &'static str {
    match notification_type.to_ascii_lowercase().as_str() {
        "leave_submitted" => return "/approvals?tab=leave",
        "claim_submitted" => return "/approvals?tab=claims",
        "overtime_submitted" => return "/approvals?tab=overtime",
        _ => {}
    }
    let kind = entity_type
        .unwrap_or(notification_type)
        .to_ascii_lowercase();
    if kind.contains("leave") {
        "/portal/leave"
    } else if kind.contains("claim") || kind.contains("expense") {
        "/portal/claims"
    } else if kind.contains("overtime") || kind.contains("ot_") {
        "/portal/overtime"
    } else if kind.contains("payroll") || kind.contains("payslip") {
        "/portal/payslips"
    } else if kind.contains("attendance") {
        "/portal/attendance"
    } else {
        "/portal/notifications"
    }
}

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
    let (unread, total) = notifications::count_summary(pool, user_id, company_id).await?;
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
    let notification = notifications::insert(
        pool,
        user_id,
        company_id,
        notification_type,
        title,
        message,
        entity_type,
        entity_id,
    )
    .await?;

    let url = notification_target_url(notification_type, entity_type);
    let tag = entity_id
        .map(|id| format!("{notification_type}:{id}"))
        .unwrap_or_else(|| notification_type.to_string());
    if let Err(err) = push_outbox::enqueue_for_user(
        pool,
        user_id,
        Some(company_id),
        Some(notification.id),
        title,
        message,
        url,
        Some(&tag),
    )
    .await
    {
        tracing::warn!(user_id = %user_id, error = %err, "Failed to enqueue Web Push notification");
    }

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
    let ids = notifications::insert_for_admins_returning_ids(
        pool,
        company_id,
        notification_type,
        title,
        message,
        entity_type,
        entity_id,
    )
    .await?;

    if !ids.is_empty() {
        let url = notification_target_url(notification_type, entity_type);
        let tag = entity_id
            .map(|id| format!("{notification_type}:{id}"))
            .unwrap_or_else(|| notification_type.to_string());
        if let Err(err) = push_outbox::enqueue_for_notifications(
            pool,
            company_id,
            &ids,
            title,
            message,
            url,
            Some(&tag),
        )
        .await
        {
            tracing::warn!(company_id = %company_id, error = %err, "Failed to enqueue admin Web Push notifications");
        }
    }

    Ok(())
}

/// Notify employees in a payroll run once the run is marked paid and locked.
pub async fn notify_payroll_paid(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    period_month: i32,
    period_year: i32,
) -> AppResult<()> {
    let title = "Payslip Ready";
    let message = format!(
        "Your payslip for {:02}/{} is now available.",
        period_month, period_year
    );
    let ids =
        notifications::insert_for_payroll_run(pool, company_id, run_id, title, &message).await?;
    if !ids.is_empty() {
        let tag = format!("payroll_processed:{run_id}");
        if let Err(err) = push_outbox::enqueue_for_notifications(
            pool,
            company_id,
            &ids,
            title,
            &message,
            "/portal/payslips",
            Some(&tag),
        )
        .await
        {
            tracing::warn!(company_id = %company_id, run_id = %run_id, error = %err, "Failed to enqueue payroll Web Push notifications");
        }
    }
    Ok(())
}

/// Notify employees when the auto-absent job marks a missing check-in.
pub async fn notify_auto_absent(
    pool: &PgPool,
    company_id: Uuid,
    employee_ids: &[Uuid],
    date: NaiveDate,
) -> AppResult<()> {
    if employee_ids.is_empty() {
        return Ok(());
    }
    let title = "Attendance Notice";
    let message = format!(
        "You were marked absent for {} because no check-in was recorded.",
        date
    );
    let ids = notifications::insert_for_employees(
        pool,
        company_id,
        employee_ids,
        "attendance_absent",
        title,
        &message,
        Some("attendance"),
        None,
    )
    .await?;
    if !ids.is_empty() {
        let tag = format!("attendance_absent:{date}");
        if let Err(err) = push_outbox::enqueue_for_notifications(
            pool,
            company_id,
            &ids,
            title,
            &message,
            "/portal/attendance",
            Some(&tag),
        )
        .await
        {
            tracing::warn!(company_id = %company_id, date = %date, error = %err, "Failed to enqueue attendance Web Push notifications");
        }
    }
    Ok(())
}

pub fn get_push_config(push: &SharedPushClient) -> PushConfigResponse {
    PushConfigResponse {
        enabled: push.is_enabled(),
        vapid_public_key: push.public_key().map(str::to_string),
    }
}

const MAX_ACTIVE_PUSH_SUBSCRIPTIONS_PER_USER: i64 = 20;

pub async fn upsert_push_subscription(
    pool: &PgPool,
    user_id: Uuid,
    company_id: Option<Uuid>,
    session_id: Option<Uuid>,
    req: UpsertPushSubscriptionRequest,
    user_agent: Option<&str>,
) -> AppResult<PushSubscriptionStatusResponse> {
    let (endpoint, p256dh, auth) =
        push_service::validate_push_subscription(&req.endpoint, &req.keys.p256dh, &req.keys.auth)?;

    if !push_subscriptions::exists_active_for_user_endpoint(pool, user_id, &endpoint).await?
        && push_subscriptions::count_active_for_user(pool, user_id).await?
            >= MAX_ACTIVE_PUSH_SUBSCRIPTIONS_PER_USER
    {
        return Err(AppError::BadRequest(
            "Maximum active push subscriptions (20) reached. Remove an old device first.".into(),
        ));
    }

    let trimmed_ua = user_agent
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(512).collect::<String>());

    push_subscriptions::upsert(
        pool,
        user_id,
        company_id,
        session_id,
        &endpoint,
        &p256dh,
        &auth,
        trimmed_ua.as_deref(),
    )
    .await?;

    let active_subscriptions = push_subscriptions::count_active_for_user(pool, user_id).await?;
    Ok(PushSubscriptionStatusResponse {
        subscribed: active_subscriptions > 0,
        active_subscriptions,
    })
}

pub async fn delete_push_subscription(
    pool: &PgPool,
    user_id: Uuid,
    endpoint: &str,
) -> AppResult<PushSubscriptionStatusResponse> {
    let trimmed = endpoint.trim();
    if !trimmed.is_empty() {
        push_subscriptions::revoke_by_endpoint(pool, user_id, trimmed).await?;
    }
    let active_subscriptions = push_subscriptions::count_active_for_user(pool, user_id).await?;
    Ok(PushSubscriptionStatusResponse {
        subscribed: active_subscriptions > 0,
        active_subscriptions,
    })
}

#[cfg(test)]
mod tests {
    use super::notification_target_url;

    #[test]
    fn submitted_notifications_route_to_specific_approval_tab() {
        assert_eq!(
            notification_target_url("leave_submitted", Some("leave_request")),
            "/approvals?tab=leave"
        );
        assert_eq!(
            notification_target_url("claim_submitted", Some("claim")),
            "/approvals?tab=claims"
        );
        assert_eq!(
            notification_target_url("overtime_submitted", Some("overtime_application")),
            "/approvals?tab=overtime"
        );
    }
}
