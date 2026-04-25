use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::payroll::PayrollRun;
use crate::services::audit_service::AuditRequestMeta;

async fn load_run(pool: &PgPool, company_id: Uuid, id: Uuid) -> AppResult<PayrollRun> {
    sqlx::query_as::<_, PayrollRun>(
        r#"SELECT id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text as status,
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by
        FROM payroll_runs
        WHERE id = $1 AND company_id = $2"#,
    )
    .bind(id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))
}

async fn audit_transition(
    pool: &PgPool,
    company_id: Uuid,
    actor_user_id: Uuid,
    action: &str,
    old_run: &PayrollRun,
    run: &PayrollRun,
    new_values: serde_json::Value,
    description: String,
    audit_meta: Option<&AuditRequestMeta>,
) {
    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_user_id),
        action,
        "payroll_run",
        Some(run.id),
        Some(serde_json::to_value(old_run).unwrap_or_default()),
        Some(new_values),
        Some(&description),
        audit_meta,
    )
    .await;
}

pub async fn submit_for_approval(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    actor_user_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollRun> {
    let old_run = load_run(pool, company_id, run_id).await?;
    if old_run.status != "processed" {
        return Err(AppError::BadRequest(
            "Only processed payroll runs can be submitted for approval".into(),
        ));
    }

    let run = sqlx::query_as::<_, PayrollRun>(
        r#"UPDATE payroll_runs SET
            status = 'pending_approval', updated_by = $3, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'processed'
        RETURNING id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text as status,
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by"#,
    )
    .bind(run_id)
    .bind(company_id)
    .bind(actor_user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Payroll run could not be submitted".into()))?;

    audit_transition(
        pool,
        company_id,
        actor_user_id,
        "submit_approval",
        &old_run,
        &run,
        serde_json::to_value(&run).unwrap_or_default(),
        format!(
            "Submitted payroll run for approval for {:02}/{}",
            run.period_month, run.period_year
        ),
        audit_meta,
    )
    .await;

    Ok(run)
}

pub async fn approve(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    actor_user_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollRun> {
    let old_run = load_run(pool, company_id, run_id).await?;
    if old_run.status != "pending_approval" {
        return Err(AppError::BadRequest(
            "Only submitted payroll runs can be approved".into(),
        ));
    }

    let run = sqlx::query_as::<_, PayrollRun>(
        r#"UPDATE payroll_runs SET
            status = 'approved', approved_by = $3, approved_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending_approval'
        RETURNING id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text as status,
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by"#,
    )
    .bind(run_id)
    .bind(company_id)
    .bind(actor_user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Payroll run could not be approved".into()))?;

    audit_transition(
        pool,
        company_id,
        actor_user_id,
        "approve",
        &old_run,
        &run,
        serde_json::to_value(&run).unwrap_or_default(),
        format!(
            "Approved payroll run for {:02}/{}",
            run.period_month, run.period_year
        ),
        audit_meta,
    )
    .await;

    Ok(run)
}

pub async fn return_for_changes(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    actor_user_id: Uuid,
    reason: Option<String>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollRun> {
    let old_run = load_run(pool, company_id, run_id).await?;
    if old_run.status != "pending_approval" {
        return Err(AppError::BadRequest(
            "Only submitted payroll runs can be returned for changes".into(),
        ));
    }

    let reason = reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(500).collect::<String>());

    let run = sqlx::query_as::<_, PayrollRun>(
        r#"UPDATE payroll_runs SET
            status = 'processed', updated_by = $3, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending_approval'
        RETURNING id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text as status,
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by"#,
    )
    .bind(run_id)
    .bind(company_id)
    .bind(actor_user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Payroll run could not be returned".into()))?;

    audit_transition(
        pool,
        company_id,
        actor_user_id,
        "return_changes",
        &old_run,
        &run,
        serde_json::json!({
            "payroll_run": run,
            "reason": reason,
        }),
        format!(
            "Returned payroll run for changes for {:02}/{}",
            old_run.period_month, old_run.period_year
        ),
        audit_meta,
    )
    .await;

    Ok(run)
}

pub async fn lock_as_paid(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    actor_user_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollRun> {
    let old_run = load_run(pool, company_id, run_id).await?;
    if old_run.status != "approved" {
        return Err(AppError::BadRequest(
            "Only approved payroll runs can be marked paid and locked".into(),
        ));
    }

    let run = sqlx::query_as::<_, PayrollRun>(
        r#"UPDATE payroll_runs SET
            status = 'paid', locked_by = $3, locked_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'approved'
        RETURNING id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text as status,
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by"#,
    )
    .bind(run_id)
    .bind(company_id)
    .bind(actor_user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Payroll run could not be locked".into()))?;

    audit_transition(
        pool,
        company_id,
        actor_user_id,
        "lock",
        &old_run,
        &run,
        serde_json::to_value(&run).unwrap_or_default(),
        format!(
            "Marked payroll run as paid and locked for {:02}/{}",
            run.period_month, run.period_year
        ),
        audit_meta,
    )
    .await;

    Ok(run)
}
