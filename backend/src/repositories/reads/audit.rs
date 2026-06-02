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
