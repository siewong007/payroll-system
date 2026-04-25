use axum::http::{HeaderMap, header};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::AppResult;

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

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AuditLogWithUser {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub old_values: Option<serde_json::Value>,
    pub new_values: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub user_email: Option<String>,
    pub user_full_name: Option<String>,
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

    // Filter by al.company_id so NULL-user rows (public kiosk endpoints etc.)
    // remain visible. Legacy rows missing company_id are excluded; migration
    // 024 backfills them from the associated user where possible.
    let count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM audit_logs al
        WHERE al.company_id = $1
        AND ($2::text IS NULL OR al.entity_type = $2)
        AND ($3::text IS NULL OR al.action = $3)
        AND ($4::uuid IS NULL OR al.user_id = $4)
        AND ($5::date IS NULL OR al.created_at >= $5::date)
        AND ($6::date IS NULL OR al.created_at < ($6::date + INTERVAL '1 day'))"#,
    )
    .bind(company_id)
    .bind(&query.entity_type)
    .bind(&query.action)
    .bind(query.user_id)
    .bind(query.start_date)
    .bind(query.end_date)
    .fetch_one(pool)
    .await?;

    let logs = sqlx::query_as::<_, AuditLogWithUser>(
        r#"SELECT al.id, al.user_id, al.action, al.entity_type, al.entity_id,
            al.old_values, al.new_values, al.ip_address, al.user_agent,
            al.description, al.created_at,
            u.email as user_email, u.full_name as user_full_name
        FROM audit_logs al
        LEFT JOIN users u ON al.user_id = u.id
        WHERE al.company_id = $1
        AND ($2::text IS NULL OR al.entity_type = $2)
        AND ($3::text IS NULL OR al.action = $3)
        AND ($4::uuid IS NULL OR al.user_id = $4)
        AND ($5::date IS NULL OR al.created_at >= $5::date)
        AND ($6::date IS NULL OR al.created_at < ($6::date + INTERVAL '1 day'))
        ORDER BY al.created_at DESC
        LIMIT $7 OFFSET $8"#,
    )
    .bind(company_id)
    .bind(&query.entity_type)
    .bind(&query.action)
    .bind(query.user_id)
    .bind(query.start_date)
    .bind(query.end_date)
    .bind(per_page)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok((logs, count))
}
#[allow(clippy::too_many_arguments)]
pub async fn log_action(
    pool: &PgPool,
    user_id: Option<Uuid>,
    action: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    old_values: Option<serde_json::Value>,
    new_values: Option<serde_json::Value>,
    description: Option<&str>,
) -> AppResult<()> {
    log_action_with_metadata(
        pool,
        None,
        user_id,
        action,
        entity_type,
        entity_id,
        old_values,
        new_values,
        description,
        None,
    )
    .await
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

    let result = sqlx::query(
        r#"INSERT INTO audit_logs
        (company_id, user_id, action, entity_type, entity_id, old_values, new_values, description, ip_address, user_agent)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
    )
    .bind(company_id)
    .bind(user_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(old_values)
    .bind(new_values)
    .bind(description)
    .bind(ip_address)
    .bind(user_agent)
    .execute(pool)
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
