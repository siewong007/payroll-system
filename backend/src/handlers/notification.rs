use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::AppResult;
use crate::models::notification::{
    DeletePushSubscriptionRequest, Notification, NotificationCount, NotificationQuery,
    PushConfigResponse, PushSubscriptionStatusResponse, TestPushResponse,
    UpsertPushSubscriptionRequest,
};
use crate::services::{notification_service, push_service};

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<NotificationQuery>,
) -> AppResult<Json<Vec<Notification>>> {
    let company_id = auth.company_id()?;
    let notifications = notification_service::get_notifications(
        &state.pool,
        auth.0.sub,
        company_id,
        q.unread_only.unwrap_or(false),
        q.effective_limit(),
    )
    .await?;
    Ok(Json(notifications))
}

pub async fn count(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<NotificationCount>> {
    let company_id = auth.company_id()?;
    let count =
        notification_service::get_notification_count(&state.pool, auth.0.sub, company_id).await?;
    Ok(Json(count))
}

pub async fn mark_read(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;
    notification_service::mark_as_read(&state.pool, auth.0.sub, company_id, id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn mark_all_read(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;
    notification_service::mark_all_read(&state.pool, auth.0.sub, company_id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn push_config(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> AppResult<Json<PushConfigResponse>> {
    Ok(Json(notification_service::get_push_config(&state.push)))
}

pub async fn upsert_subscription(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(req): Json<UpsertPushSubscriptionRequest>,
) -> AppResult<Json<PushSubscriptionStatusResponse>> {
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok());
    let status = notification_service::upsert_push_subscription(
        &state.pool,
        auth.0.sub,
        auth.0.company_id,
        Some(auth.0.sid),
        req,
        user_agent,
    )
    .await?;
    Ok(Json(status))
}

pub async fn delete_subscription(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<DeletePushSubscriptionRequest>,
) -> AppResult<Json<PushSubscriptionStatusResponse>> {
    let status =
        notification_service::delete_push_subscription(&state.pool, auth.0.sub, &req.endpoint)
            .await?;
    Ok(Json(status))
}

pub async fn test_push(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<TestPushResponse>> {
    let target_url = if auth.0.employee_id.is_some() {
        "/portal/notifications"
    } else {
        "/"
    };
    let result =
        push_service::send_test_push(&state.pool, &state.push, auth.0.sub, target_url).await?;
    Ok(Json(result))
}
