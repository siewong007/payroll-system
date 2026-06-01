use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, Permission};
use crate::core::error::{AppError, AppResult};
use crate::models::payroll::{
    CreatePayrollEntryRequest, PayrollEntry, PayrollEntryWithEmployee, PayrollGroup, PayrollItem,
    PayrollRun, PayrollSummary, ProcessPayrollRequest, UpdatePayrollEntryRequest,
    UpdatePayrollPcbRequest,
};
use crate::services::audit_service::{AuditLogWithUser, AuditRequestMeta};
use crate::services::{payroll_engine, payroll_entry_service, payroll_service};

#[derive(Debug, Deserialize)]
pub struct ReturnPayrollRunRequest {
    pub reason: Option<String>,
}

pub async fn process(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(req): Json<ProcessPayrollRequest>,
) -> AppResult<Json<PayrollRun>> {
    auth.require_permission(Permission::ManagePayrollDraft)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);

    let pay_date = req.pay_date.unwrap_or_else(|| {
        chrono::NaiveDate::from_ymd_opt(req.period_year, req.period_month as u32, 28)
            .unwrap_or_else(|| {
                chrono::NaiveDate::from_ymd_opt(req.period_year, req.period_month as u32, 1)
                    .unwrap()
            })
    });

    let run = payroll_engine::process_payroll(
        &state.pool,
        company_id,
        req.payroll_group_id,
        req.period_year,
        req.period_month,
        pay_date,
        auth.0.sub,
        req.notes,
        Some(&audit_meta),
    )
    .await?;

    Ok(Json(run))
}

pub async fn list_runs(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<PayrollRun>>> {
    auth.require_permission(Permission::ViewPayroll)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let runs = payroll_service::list_runs(&state.pool, company_id).await?;

    Ok(Json(runs))
}

pub async fn get_run(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PayrollSummary>> {
    auth.require_permission(Permission::ViewPayroll)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    Ok(Json(
        payroll_service::get_summary(&state.pool, company_id, id).await?,
    ))
}

pub async fn list_run_audit_logs(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<AuditLogWithUser>>> {
    auth.require_permission(Permission::ViewPayroll)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let run_exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(
            SELECT 1 FROM payroll_runs WHERE id = $1 AND company_id = $2
        ) AS "exists!""#,
        id,
        company_id,
    )
    .fetch_one(&state.pool)
    .await?;

    if !run_exists {
        return Err(AppError::NotFound("Payroll run not found".into()));
    }

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
        id,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(logs))
}

pub async fn delete_run(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_permission(Permission::ManagePayrollDraft)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let run = sqlx::query_as!(
        PayrollRun,
        r#"SELECT id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text AS "status!",
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by
        FROM payroll_runs
        WHERE id = $1 AND company_id = $2"#,
        id,
        company_id,
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))?;

    if run.status == "processing" {
        return Err(AppError::BadRequest(
            "Payroll run is currently processing and cannot be deleted".into(),
        ));
    }

    if matches!(
        run.status.as_str(),
        "pending_approval" | "approved" | "paid"
    ) || run.locked_at.is_some()
    {
        return Err(AppError::BadRequest(
            "Submitted, approved, or paid payroll runs are locked and cannot be deleted".into(),
        ));
    }

    let mut tx = state.pool.begin().await?;

    sqlx::query!(
        r#"UPDATE payroll_entries
        SET is_processed = FALSE, payroll_run_id = NULL, updated_at = NOW(), updated_by = $3
        WHERE payroll_run_id = $1 AND company_id = $2"#,
        id,
        company_id,
        auth.0.sub,
    )
    .execute(&mut *tx)
    .await?;

    // Revert processed claims for this period back to approved
    sqlx::query!(
        r#"UPDATE claims SET status = 'approved', updated_at = NOW()
        WHERE company_id = $1 AND status = 'processed'
          AND expense_date >= $2 AND expense_date <= $3"#,
        company_id,
        run.period_start,
        run.period_end,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        r#"DELETE FROM payroll_item_details pid
        USING payroll_items pi
        WHERE pid.payroll_item_id = pi.id
          AND pi.payroll_run_id = $1"#,
        id,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!("DELETE FROM payroll_items WHERE payroll_run_id = $1", id)
        .execute(&mut *tx)
        .await?;

    sqlx::query!(
        "DELETE FROM payroll_runs WHERE id = $1 AND company_id = $2",
        id,
        company_id,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let _ = crate::services::audit_service::log_action_with_metadata(
        &state.pool,
        Some(company_id),
        Some(auth.0.sub),
        "delete",
        "payroll_run",
        Some(id),
        Some(serde_json::to_value(&run).unwrap_or_default()),
        None,
        Some(&format!(
            "Deleted payroll run {} for {:02}/{}",
            id, run.period_month, run.period_year
        )),
        Some(&audit_meta),
    )
    .await;

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn update_item_pcb(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path((run_id, employee_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdatePayrollPcbRequest>,
) -> AppResult<Json<PayrollSummary>> {
    auth.require_permission(Permission::ManagePayrollDraft)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    if req.pcb_amount < 0 {
        return Err(AppError::BadRequest("PCB amount cannot be negative".into()));
    }

    let mut tx = state.pool.begin().await?;

    let run_row = sqlx::query!(
        r#"SELECT status::text AS "status!", period_year, period_month
        FROM payroll_runs
        WHERE id = $1 AND company_id = $2
        FOR UPDATE"#,
        run_id,
        company_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))?;

    let (run_status, period_year, period_month) =
        (run_row.status, run_row.period_year, run_row.period_month);

    if run_status != "processed" {
        return Err(AppError::BadRequest(
            "PCB can only be edited while the payroll run is processed and not yet approved".into(),
        ));
    }

    let has_later_run = sqlx::query_scalar!(
        r#"SELECT EXISTS(
            SELECT 1
            FROM payroll_items pi
            JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
            WHERE pi.employee_id = $1
              AND pr.company_id = $2
              AND pr.status::text IN ('processed', 'pending_approval', 'approved', 'paid')
              AND (pr.period_year > $3 OR (pr.period_year = $3 AND pr.period_month > $4))
        ) AS "exists!""#,
        employee_id,
        company_id,
        period_year,
        period_month,
    )
    .fetch_one(&mut *tx)
    .await?;

    if has_later_run {
        return Err(AppError::BadRequest(
            "PCB cannot be edited because a later payroll run already exists for this employee"
                .into(),
        ));
    }

    let current = sqlx::query!(
        r#"SELECT pcb_amount, total_deductions, net_salary, ytd_pcb
        FROM payroll_items
        WHERE payroll_run_id = $1 AND employee_id = $2
        FOR UPDATE"#,
        run_id,
        employee_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Payroll item not found".into()))?;

    let old_pcb = current.pcb_amount;
    let delta = req.pcb_amount - old_pcb;
    let new_total_deductions = current.total_deductions + delta;
    let new_net_salary = current.net_salary - delta;
    let new_ytd_pcb = current.ytd_pcb + delta;

    sqlx::query!(
        r#"UPDATE payroll_items
        SET pcb_amount = $3,
            total_deductions = $4,
            net_salary = $5,
            ytd_pcb = $6,
            updated_at = NOW()
        WHERE payroll_run_id = $1 AND employee_id = $2"#,
        run_id,
        employee_id,
        req.pcb_amount,
        new_total_deductions,
        new_net_salary,
        new_ytd_pcb,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        r#"UPDATE payroll_runs
        SET total_pcb = total_pcb + $3,
            total_net = total_net - $3,
            updated_at = NOW(),
            updated_by = $4
        WHERE id = $1 AND company_id = $2"#,
        run_id,
        company_id,
        delta,
        auth.0.sub,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let summary = payroll_service::get_summary(&state.pool, company_id, run_id).await?;

    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let _ = crate::services::audit_service::log_action_with_metadata(
        &state.pool,
        Some(company_id),
        Some(auth.0.sub),
        "update",
        "payroll_item",
        None,
        Some(serde_json::json!({
            "payroll_run_id": run_id,
            "employee_id": employee_id,
            "pcb_amount": old_pcb
        })),
        Some(serde_json::json!({
            "payroll_run_id": run_id,
            "employee_id": employee_id,
            "pcb_amount": req.pcb_amount
        })),
        Some("Updated payroll item PCB amount"),
        Some(&audit_meta),
    )
    .await;

    Ok(Json(summary))
}

pub async fn submit_run_for_approval(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PayrollRun>> {
    auth.require_permission(Permission::SubmitPayroll)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let run = crate::services::payroll_lifecycle_service::submit_for_approval(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;

    Ok(Json(run))
}

pub async fn approve_run(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PayrollRun>> {
    auth.require_permission(Permission::ApprovePayroll)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let run = crate::services::payroll_lifecycle_service::approve(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;

    Ok(Json(run))
}

pub async fn return_run_for_changes(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ReturnPayrollRunRequest>,
) -> AppResult<Json<PayrollRun>> {
    auth.require_permission(Permission::ApprovePayroll)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let run = crate::services::payroll_lifecycle_service::return_for_changes(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        req.reason,
        Some(&audit_meta),
    )
    .await?;

    Ok(Json(run))
}

pub async fn lock_run(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PayrollRun>> {
    auth.require_permission(Permission::MarkPayrollPaid)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let run = crate::services::payroll_lifecycle_service::lock_as_paid(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;

    Ok(Json(run))
}

pub async fn list_groups(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<PayrollGroup>>> {
    auth.require_permission(Permission::ViewPayroll)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let groups = payroll_service::list_groups(&state.pool, company_id).await?;

    Ok(Json(groups))
}

#[derive(Debug, Deserialize)]
pub struct PayrollEntryQuery {
    pub period_year: Option<i32>,
    pub period_month: Option<i32>,
    pub employee_id: Option<Uuid>,
    pub item_type: Option<String>,
    pub include_processed: Option<bool>,
}

pub async fn list_entries(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<PayrollEntryQuery>,
) -> AppResult<Json<Vec<PayrollEntryWithEmployee>>> {
    auth.require_permission(Permission::ManagePayrollDraft)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;
    let entries = payroll_entry_service::list_entries(
        &state.pool,
        company_id,
        q.period_year,
        q.period_month,
        q.employee_id,
        q.item_type.as_deref(),
        q.include_processed.unwrap_or(false),
    )
    .await?;

    Ok(Json(entries))
}

pub async fn create_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(req): Json<CreatePayrollEntryRequest>,
) -> AppResult<Json<PayrollEntry>> {
    auth.require_permission(Permission::ManagePayrollDraft)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let entry = payroll_entry_service::create_entry(
        &state.pool,
        company_id,
        req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;

    Ok(Json(entry))
}

pub async fn update_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePayrollEntryRequest>,
) -> AppResult<Json<PayrollEntry>> {
    auth.require_permission(Permission::ManagePayrollDraft)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let updated = payroll_entry_service::update_entry(
        &state.pool,
        company_id,
        id,
        req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;

    Ok(Json(updated))
}

pub async fn delete_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_permission(Permission::ManagePayrollDraft)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let audit_meta = AuditRequestMeta::from_headers(&headers);
    payroll_entry_service::delete_entry(&state.pool, company_id, id, auth.0.sub, Some(&audit_meta))
        .await?;

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn download_run_payslips_pdf(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(run_id): Path<Uuid>,
) -> Result<axum::response::Response, crate::core::error::AppError> {
    use axum::body::Body;
    use axum::http::{Response, StatusCode, header};

    auth.require_permission(Permission::ViewPayroll)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let bytes = crate::services::payslip_pdf_service::generate_bulk_payslips(
        &state.pool,
        run_id,
        company_id,
    )
    .await?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"payslips.pdf\"",
        )
        .body(Body::from(bytes))
        .unwrap())
}

pub async fn get_items(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<PayrollItem>>> {
    auth.require_permission(Permission::ViewPayroll)?;
    let items = payroll_service::list_items(&state.pool, id).await?;

    Ok(Json(items))
}
