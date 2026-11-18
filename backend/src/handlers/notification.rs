use axum::{
    Json,
    extract::{Path, Query, State},
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::AppResult;
use crate::models::notification::{Notification, NotificationCount, NotificationQuery};
use crate::services::notification_service;

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
