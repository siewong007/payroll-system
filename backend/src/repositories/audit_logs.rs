//! Data access for the `audit_logs` table. Single-table writes/counts live here;
//! the user-joined read model is in `reads::audit`.

use chrono::NaiveDate;
use uuid::Uuid;

use crate::core::error::AppResult;
use sqlx::{Executor, Postgres};

/// Count audit rows for a company matching the optional filters (entity type,
/// action, actor, and an inclusive created-at date range).
pub async fn count_filtered(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    entity_type: Option<&str>,
    action: Option<&str>,
    user_id: Option<Uuid>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> AppResult<i64> {
    // Filter by al.company_id so NULL-user rows (public kiosk endpoints etc.)
    // remain visible. Legacy rows missing company_id are excluded; migration
    // 024 backfills them from the associated user where possible.
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM audit_logs al
        WHERE al.company_id = $1
        AND ($2::text IS NULL OR al.entity_type = $2)
        AND ($3::text IS NULL OR al.action = $3)
        AND ($4::uuid IS NULL OR al.user_id = $4)
        AND ($5::date IS NULL OR al.created_at >= $5::date)
        AND ($6::date IS NULL OR al.created_at < ($6::date + INTERVAL '1 day'))"#,
        company_id,
        entity_type,
        action,
        user_id,
        start_date,
        end_date,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}

/// Insert one audit row. Returns the raw result so callers can log-and-swallow
/// (audit writes must never fail the request they describe).
#[allow(clippy::too_many_arguments)]
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Option<Uuid>,
    user_id: Option<Uuid>,
    action: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    old_values: Option<serde_json::Value>,
    new_values: Option<serde_json::Value>,
    description: Option<&str>,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO audit_logs
        (company_id, user_id, action, entity_type, entity_id, old_values, new_values, description, ip_address, user_agent)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
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
    .execute(executor)
    .await?;
    Ok(())
}

// `insert_bulk_import` used to live here. It was the only writer whose INSERT
// omitted `company_id`, and every read path filters `WHERE al.company_id = $1`
// (`count_filtered` above, `reads::audit::list_filtered`) — so the highest-value
// employee-creating operation in the product wrote a row no API could ever
// return. The import now goes through `audit_service::log_action_with_metadata`
// like every other mutation, which makes that class of drift impossible rather
// than merely fixed once.
