use axum::{
    Json,
    extract::{Query, State},
};

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::services::audit_service::{self, AuditLogQuery};

fn require_admin(auth: &AuthUser) -> AppResult<uuid::Uuid> {
    if auth.has_any_role(&["super_admin", "admin"]) {
        return auth.company_id();
    }
    Err(AppError::Forbidden("Admin access required".into()))
}

pub async fn list_audit_logs(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<AuditLogQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = require_admin(&auth)?;
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(25);
    let (logs, total) = audit_service::list_audit_logs(&state.pool, company_id, &query).await?;

    Ok(Json(serde_json::json!({
        "data": logs,
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}
