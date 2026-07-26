use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::repositories::audit_logs;
use crate::repositories::reads::audit as audit_reads;

pub use crate::models::audit::{AuditLogQuery, AuditLogWithUser, AuditRequestMeta};
pub use crate::models::audit_filter::{AuditFilterOptions, FilterOption};

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
        query.entity_id,
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
        query.entity_id,
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

/// Write one audit row.
///
/// Generic over the executor so a caller can pass `&pool` (best-effort, the
/// common case) *or* `&mut *tx` to put the audit row in the same transaction as
/// the change it describes. The second form matters wherever the trail is
/// evidence: a row written on a separate connection survives a rollback of the
/// change, and is lost if the change commits and the audit write then fails.
#[allow(clippy::too_many_arguments)]
pub async fn log_action_with_metadata(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
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
        executor,
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

/// The filter vocabulary for one company's audit trail.
///
/// Both facets are read from the company's own rows, so the dropdowns offer
/// exactly what exists — no option that matches nothing, and nothing written
/// that the UI cannot reach. The two walks are independent and could run
/// concurrently, but they are sub-millisecond against the 1008 indexes and
/// sharing one `&PgPool` connection sequentially keeps the pool pressure of an
/// admin screen at one connection.
pub async fn list_filter_options(pool: &PgPool, company_id: Uuid) -> AppResult<AuditFilterOptions> {
    let entity_types = audit_reads::distinct_entity_types(pool, company_id).await?;
    let actions = audit_reads::distinct_actions(pool, company_id).await?;

    Ok(AuditFilterOptions {
        entity_types: entity_types.into_iter().map(FilterOption::new).collect(),
        actions: actions.into_iter().map(FilterOption::new).collect(),
    })
}
