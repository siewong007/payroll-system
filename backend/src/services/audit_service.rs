use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::AppResult;

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

    let count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM audit_logs al
        LEFT JOIN users u ON al.user_id = u.id
        WHERE (u.company_id = $1 OR al.user_id IN (SELECT id FROM users WHERE company_id = $1))
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
        WHERE (u.company_id = $1 OR al.user_id IN (SELECT id FROM users WHERE company_id = $1))
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
    sqlx::query(
        r#"INSERT INTO audit_logs (user_id, action, entity_type, entity_id, old_values, new_values, description)
        VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(user_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(old_values)
    .bind(new_values)
    .bind(description)
    .execute(pool)
    .await?;
    Ok(())
}
