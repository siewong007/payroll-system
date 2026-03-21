use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::AppResult;
use crate::models::notification::{Notification, NotificationCount};
use crate::services::notification_service;

#[derive(Debug, Deserialize)]
pub struct NotificationQuery {
    pub unread_only: Option<bool>,
    pub limit: Option<i64>,
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<NotificationQuery>,
) -> AppResult<Json<Vec<Notification>>> {
    let notifications = notification_service::get_notifications(
        &state.pool,
        auth.0.sub,
        q.unread_only.unwrap_or(false),
        q.limit.unwrap_or(50),
    )
    .await?;
    Ok(Json(notifications))
}

pub async fn count(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<NotificationCount>> {
    let count = notification_service::get_notification_count(&state.pool, auth.0.sub).await?;
    Ok(Json(count))
}

pub async fn mark_read(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    notification_service::mark_as_read(&state.pool, auth.0.sub, id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn mark_all_read(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    notification_service::mark_all_read(&state.pool, auth.0.sub).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}
