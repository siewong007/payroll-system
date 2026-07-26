//! Read model for the audit trail: `audit_logs` left-joined to `users` so each
//! row carries the actor's email/name. Backs both the filtered admin log viewer
//! and the per-payroll-run history.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::audit::AuditLogWithUser;

/// One page of audit rows for a company, with the same optional filters as
/// [`crate::repositories::audit_logs::count_filtered`].
#[allow(clippy::too_many_arguments)]
pub async fn list_filtered(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    entity_type: Option<&str>,
    action: Option<&str>,
    user_id: Option<Uuid>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<AuditLogWithUser>> {
    let logs = sqlx::query_as!(
        AuditLogWithUser,
        r#"SELECT al.id, al.user_id, al.action, al.entity_type, al.entity_id,
            al.old_values, al.new_values, al.ip_address, al.user_agent,
            al.description, al.created_at,
            u.email AS "user_email?", u.full_name AS "user_full_name?"
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
        company_id,
        entity_type,
        action,
        user_id,
        start_date,
        end_date,
        limit,
        offset,
    )
    .fetch_all(executor)
    .await?;
    Ok(logs)
}

/// The distinct `entity_type` values a company has ever written.
///
/// Written as a recursive loose index scan ("skip scan") rather than
/// `SELECT DISTINCT`, because the two have different growth curves and this
/// runs on every load of the audit screen. `audit_logs` is append-only with no
/// retention path, so a plain `DISTINCT` reads every row the company has
/// accumulated — it is O(rows) forever, for an answer that is two dozen values.
/// Skipping from one distinct value to the next is O(distinct × log n): the
/// walk below does one index seek per value returned, regardless of how many
/// rows sit behind each.
///
/// Requires `idx_audit_logs_company_entity_created` (migration 1008); without a
/// `(company_id, entity_type, …)` index this degenerates into a seek per value
/// against no usable index and is *worse* than the DISTINCT it replaces.
pub async fn distinct_entity_types(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<String>> {
    let values = sqlx::query_scalar!(
        r#"WITH RECURSIVE walk AS (
            (SELECT entity_type FROM audit_logs
             WHERE company_id = $1
             ORDER BY entity_type
             LIMIT 1)
            UNION ALL
            SELECT (SELECT al.entity_type FROM audit_logs al
                    WHERE al.company_id = $1 AND al.entity_type > walk.entity_type
                    ORDER BY al.entity_type
                    LIMIT 1)
            FROM walk
            WHERE walk.entity_type IS NOT NULL
        )
        SELECT entity_type AS "entity_type!" FROM walk
        WHERE entity_type IS NOT NULL
        ORDER BY entity_type"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(values)
}

/// The distinct `action` values a company has ever written. Same shape and same
/// reasoning as [`distinct_entity_types`]; requires
/// `idx_audit_logs_company_action_created`.
pub async fn distinct_actions(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<String>> {
    let values = sqlx::query_scalar!(
        r#"WITH RECURSIVE walk AS (
            (SELECT action FROM audit_logs
             WHERE company_id = $1
             ORDER BY action
             LIMIT 1)
            UNION ALL
            SELECT (SELECT al.action FROM audit_logs al
                    WHERE al.company_id = $1 AND al.action > walk.action
                    ORDER BY al.action
                    LIMIT 1)
            FROM walk
            WHERE walk.action IS NOT NULL
        )
        SELECT action AS "action!" FROM walk
        WHERE action IS NOT NULL
        ORDER BY action"#,
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(values)
}

/// All audit rows (up to 100) attributable to one payroll run — the run itself
/// plus item-level edits that reference it in their old/new values.
pub async fn list_for_run(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    run_id: Uuid,
) -> AppResult<Vec<AuditLogWithUser>> {
    let logs = sqlx::query_as!(
        AuditLogWithUser,
        r#"SELECT al.id, al.user_id, al.action, al.entity_type, al.entity_id,
            al.old_values, al.new_values, al.ip_address, al.user_agent,
            al.description, al.created_at,
            u.email AS "user_email?", u.full_name AS "user_full_name?"
        FROM audit_logs al
        LEFT JOIN users u ON al.user_id = u.id
        WHERE al.company_id = $1
          AND (
            (al.entity_type = 'payroll_run' AND al.entity_id = $2)
            OR (
                al.entity_type = 'payroll_item'
                AND (
                    al.old_values->>'payroll_run_id' = $2::text
                    OR al.new_values->>'payroll_run_id' = $2::text
                )
            )
          )
        ORDER BY al.created_at DESC
        LIMIT 100"#,
        company_id,
        run_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(logs)
}
