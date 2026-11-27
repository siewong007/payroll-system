use std::collections::HashSet;

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
    list_audit_logs_scoped(pool, company_id, query, false).await
}

pub async fn list_audit_logs_scoped(
    pool: &PgPool,
    company_id: Uuid,
    query: &AuditLogQuery,
    hide_privileged: bool,
) -> AppResult<(Vec<AuditLogWithUser>, i64)> {
    let page = query.page.unwrap_or(1).clamp(1, 100_000);
    let per_page = query.per_page.unwrap_or(25).clamp(1, 100);
    let offset = (page - 1).saturating_mul(per_page);

    let count = audit_logs::count_filtered_scoped(
        pool,
        company_id,
        query.entity_type.as_deref(),
        query.entity_id,
        query.action.as_deref(),
        query.user_id,
        start_date_or_none(query),
        query.end_date,
        hide_privileged,
    )
    .await?;

    let logs = audit_reads::list_filtered_scoped(
        pool,
        company_id,
        query.entity_type.as_deref(),
        query.entity_id,
        query.action.as_deref(),
        query.user_id,
        start_date_or_none(query),
        query.end_date,
        hide_privileged,
        per_page,
        offset,
    )
    .await?;

    Ok((logs, count))
}

fn start_date_or_none(query: &AuditLogQuery) -> Option<chrono::NaiveDate> {
    query.start_date
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
pub async fn list_filter_options(
    pool: &PgPool,
    company_id: Uuid,
    hide_privileged: bool,
) -> AppResult<AuditFilterOptions> {
    let entity_types =
        audit_reads::distinct_entity_types(pool, company_id, hide_privileged).await?;
    let actions = audit_reads::distinct_actions(pool, company_id, hide_privileged).await?;

    Ok(AuditFilterOptions {
        entity_types: entity_types.into_iter().map(FilterOption::new).collect(),
        actions: actions.into_iter().map(FilterOption::new).collect(),
    })
}

fn is_payroll_audit_key(key: &str, entity_type: &str) -> bool {
    let k = key.to_ascii_lowercase();
    let is_payroll_entity = matches!(
        entity_type,
        "payroll_entry" | "payroll_run" | "payroll_item" | "salary_history"
    );
    if is_payroll_entity
        && matches!(
            k.as_str(),
            "amount"
                | "rate"
                | "quantity"
                | "description"
                | "notes"
                | "reason"
                | "cancel_reason"
                | "category"
                | "item_type"
                | "is_taxable"
        )
    {
        return true;
    }
    if entity_type == "tp3_record"
        && !matches!(
            k.as_str(),
            "id" | "employee_id"
                | "company_id"
                | "tax_year"
                | "created_at"
                | "updated_at"
                | "previous_employer_name"
        )
    {
        return true;
    }
    matches!(
        k.as_str(),
        "basic_salary"
            | "base_salary"
            | "old_salary"
            | "new_salary"
            | "previous_salary"
            | "salary"
            | "gross_salary"
            | "net_salary"
            | "hourly_rate"
            | "daily_rate"
            | "overtime_pay"
            | "pcb_amount"
            | "pcb"
            | "tax_number"
            | "tax_identification_number"
            | "bank_name"
            | "bank_account_number"
            | "bank_account_type"
            | "allowances"
            | "deductions"
            | "bonus"
            | "commission"
            | "salary_group"
            | "payroll_group_id"
            | "unpaid_leave_divisor"
            | "working_spouse"
            | "is_muslim"
    ) || k.starts_with("bank_")
        || k.starts_with("epf_")
        || k.starts_with("socso_")
        || k.starts_with("eis_")
        || k.starts_with("hrdf_")
        || k.starts_with("zakat")
        || k.starts_with("ptptn")
        || k.starts_with("tabung_haji")
        || k.starts_with("total_")
        || k.starts_with("ytd_")
        || k.starts_with("previous_")
        || k.ends_with("_ytd")
}

fn is_personal_audit_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    matches!(
        k.as_str(),
        "ic_number"
            | "passport_number"
            | "date_of_birth"
            | "phone"
            | "emergency_contact_phone"
            | "address"
            | "address_line1"
            | "address_line2"
            | "city"
            | "state"
            | "postcode"
            | "bank_name"
            | "bank_account_number"
            | "bank_account_type"
            | "working_spouse"
            | "is_muslim"
    )
}

fn is_privileged_actor_ref(k: &str, v: &serde_json::Value, priv_ids: &HashSet<Uuid>) -> bool {
    let lower = k.to_ascii_lowercase();
    let is_actor_key = lower == "user_id" || lower == "reviewed_by" || lower.ends_with("_by");
    if !is_actor_key {
        return false;
    }
    v.as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .is_some_and(|id| priv_ids.contains(&id))
}

fn is_email_log_audit_key(k: &str, entity_type: &str) -> bool {
    matches!(
        entity_type,
        "letter" | "email" | "email_log" | "email_outbox"
    ) && matches!(
        k.to_ascii_lowercase().as_str(),
        "recipient_email" | "subject" | "body_html"
    )
}

fn redact_audit_json(
    value: &mut serde_json::Value,
    entity_type: &str,
    hide_payroll: bool,
    hide_personal: bool,
    hide_email_logs: bool,
    privileged_actor_ids: Option<&HashSet<Uuid>>,
) {
    match value {
        serde_json::Value::Object(map) => {
            let is_sensitive_setting = hide_payroll
                && entity_type == "company_setting"
                && map
                    .get("category")
                    .and_then(|v| v.as_str())
                    .is_some_and(|c| c == "payroll" || c == "statutory");
            for (k, v) in map.iter_mut() {
                if k == "editor_ids"
                    && let Some(priv_ids) = privileged_actor_ids
                    && let Some(arr) = v.as_array_mut()
                {
                    for elem in arr.iter_mut() {
                        if elem
                            .as_str()
                            .and_then(|s| Uuid::parse_str(s).ok())
                            .is_some_and(|id| priv_ids.contains(&id))
                        {
                            *elem = serde_json::Value::String("[REDACTED]".to_string());
                        }
                    }
                    continue;
                }
                let should_redact = (is_sensitive_setting && k == "value")
                    || (hide_payroll && is_payroll_audit_key(k, entity_type))
                    || (hide_personal && is_personal_audit_key(k))
                    || (hide_email_logs && is_email_log_audit_key(k, entity_type))
                    || privileged_actor_ids
                        .is_some_and(|priv_ids| is_privileged_actor_ref(k, v, priv_ids));
                if should_redact {
                    *v = serde_json::Value::String("[REDACTED]".to_string());
                } else {
                    redact_audit_json(
                        v,
                        entity_type,
                        hide_payroll,
                        hide_personal,
                        hide_email_logs,
                        privileged_actor_ids,
                    );
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_audit_json(
                    item,
                    entity_type,
                    hide_payroll,
                    hide_personal,
                    hide_email_logs,
                    privileged_actor_ids,
                );
            }
        }
        _ => {}
    }
}

/// Redacts sensitive payroll, personal-identity, email-log, and privileged-actor
/// fields on returned audit rows according to the caller's privileges.
pub fn redact_audit_logs(
    logs: &mut [AuditLogWithUser],
    hide_payroll: bool,
    hide_personal: bool,
    hide_email_logs: bool,
    privileged_actor_ids: Option<&HashSet<Uuid>>,
) {
    if !hide_payroll && !hide_personal && !hide_email_logs && privileged_actor_ids.is_none() {
        return;
    }
    for row in logs {
        let entity_type = row.entity_type.clone();
        if let Some(priv_ids) = privileged_actor_ids
            && row.user_id.is_some_and(|uid| priv_ids.contains(&uid))
        {
            row.user_id = None;
            row.user_email = None;
            row.user_full_name = None;
        }
        if hide_payroll
            && matches!(
                entity_type.as_str(),
                "payroll_run" | "payroll_item" | "payroll_entry" | "tp3_record"
            )
            && row.description.is_some()
        {
            row.description = Some("[REDACTED]".to_string());
        }
        if let Some(ref mut old_val) = row.old_values {
            redact_audit_json(
                old_val,
                &entity_type,
                hide_payroll,
                hide_personal,
                hide_email_logs,
                privileged_actor_ids,
            );
        }
        if let Some(ref mut new_val) = row.new_values {
            redact_audit_json(
                new_val,
                &entity_type,
                hide_payroll,
                hide_personal,
                hide_email_logs,
                privileged_actor_ids,
            );
        }
    }
}
