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

    let audit_meta = AuditRequestMeta::from_headers(&headers);

    payroll_service::delete_run(&state.pool, company_id, id, auth.0.sub, Some(&audit_meta)).await?;

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

    let audit_meta = AuditRequestMeta::from_headers(&headers);
    let summary = payroll_service::update_item_pcb(
        &state.pool,
        company_id,
        run_id,
        employee_id,
        req.pcb_amount,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;

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
