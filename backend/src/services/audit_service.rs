use axum::http::{HeaderMap, header};
use chrono::NaiveDate;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::repositories::audit_logs;
use crate::repositories::reads::audit as audit_reads;

pub use crate::repositories::reads::audit::AuditLogWithUser;

#[derive(Debug, Clone, Default)]
pub struct AuditRequestMeta {
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

impl AuditRequestMeta {
    pub fn from_headers(headers: &HeaderMap) -> Self {
        let ip_address = headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(',').next())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                headers
                    .get("x-real-ip")
                    .and_then(|value| value.to_str().ok())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            });

        let user_agent = headers
            .get(header::USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.chars().take(500).collect());

        Self {
            ip_address,
            user_agent,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub entity_type: Option<String>,
    pub action: Option<String>,
    pub user_id: Option<Uuid>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub async fn list_audit_logs(
    pool: &PgPool,
    company_id: Uuid,
    query: &AuditLogQuery,
) -> AppResult<(Vec<AuditLogWithUser>, i64)> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(25).min(100);
    let offset = (page - 1) * per_page;

    let count = audit_logs::count_filtered(
        pool,
        company_id,
        query.entity_type.as_deref(),
        query.action.as_deref(),
        query.user_id,
        query.start_date,
        query.end_date,
    )
    .await?;

    let logs = audit_reads::list_filtered(
        pool,
        company_id,
        query.entity_type.as_deref(),
        query.action.as_deref(),
        query.user_id,
        query.start_date,
        query.end_date,
        per_page,
        offset,
    )
    .await?;

    Ok((logs, count))
}

#[allow(clippy::too_many_arguments)]
pub async fn log_action_with_metadata(
    pool: &PgPool,
    company_id: Option<Uuid>,
    user_id: Option<Uuid>,
    action: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    old_values: Option<serde_json::Value>,
    new_values: Option<serde_json::Value>,
    description: Option<&str>,
    metadata: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let ip_address = metadata.and_then(|meta| meta.ip_address.as_deref());
    let user_agent = metadata.and_then(|meta| meta.user_agent.as_deref());

    let result = audit_logs::insert(
        pool,
        company_id,
        user_id,
        action,
        entity_type,
        entity_id,
        old_values,
        new_values,
        description,
        ip_address,
        user_agent,
    )
    .await;

    // Call sites intentionally discard the Result (audit logging must never
    // fail the caller), so surface the error here — otherwise insert failures
    // in production (FK violations, column-length overflow, etc.) are invisible.
    if let Err(e) = &result {
        tracing::warn!(
            error = %e,
            action = action,
            entity_type = entity_type,
            company_id = ?company_id,
            user_id = ?user_id,
            "Failed to write audit_logs row"
        );
    }

    result?;
    Ok(())
}
