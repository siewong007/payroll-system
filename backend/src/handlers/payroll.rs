use axum::{
    Json,
    extract::{Path, Query, State},
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, Permission};
use crate::core::error::{AppError, AppResult};
use crate::models::payroll::{
    CreatePayrollEntryRequest, PayrollEntry, PayrollEntryQuery, PayrollEntryWithEmployee,
    PayrollGroup, PayrollItem, PayrollPreview, PayrollRun, PayrollSummary, PayslipBreakdown,
    ProcessPayrollRequest, ReturnPayrollRunRequest, UpdatePayrollEntryRequest,
    UpdatePayrollPcbRequest,
};
use crate::services::audit_service::{AuditLogWithUser, AuditRequestMeta};
use crate::services::{payroll_engine, payroll_entry_service, payroll_service};

/// Pay date for a request that does not name one: the 28th, falling back to the
/// 1st only if that day does not exist in the month.
///
/// Shared by `process` and `preview` so the preview is dated exactly as the run
/// it is previewing.
fn default_pay_date(req: &ProcessPayrollRequest) -> chrono::NaiveDate {
    req.pay_date.unwrap_or_else(|| {
        chrono::NaiveDate::from_ymd_opt(req.period_year, req.period_month as u32, 28)
            .unwrap_or_else(|| {
                chrono::NaiveDate::from_ymd_opt(req.period_year, req.period_month as u32, 1)
                    .unwrap()
            })
    })
}

pub async fn process(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Json(req): Json<ProcessPayrollRequest>,
) -> AppResult<Json<PayrollRun>> {
    auth.require_permission(Permission::ManagePayrollDraft)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;
    let pay_date = default_pay_date(&req);

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

/// Dry-run the same calculation `process` would commit.
///
/// Gated on `ManagePayrollDraft` like `process` itself: the response carries
/// every employee's projected net pay, which is payroll data.
pub async fn preview(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<ProcessPayrollRequest>,
) -> AppResult<Json<PayrollPreview>> {
    auth.require_permission(Permission::ManagePayrollDraft)?;
    let company_id = auth.company_id()?;

    let preview = payroll_engine::preview_payroll(
        &state.pool,
        company_id,
        req.payroll_group_id,
        req.period_year,
        req.period_month,
        default_pay_date(&req),
    )
    .await?;

    Ok(Json(preview))
}

pub async fn get_payslip_breakdown(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((run_id, employee_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<PayslipBreakdown>> {
    auth.require_permission(Permission::ViewPayroll)?;
    let company_id = auth.company_id()?;

    Ok(Json(
        payroll_service::get_payslip_breakdown(&state.pool, company_id, run_id, employee_id)
            .await?,
    ))
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

    let logs = payroll_service::list_run_audit_logs(&state.pool, company_id, id).await?;

    Ok(Json(logs))
}

pub async fn delete_run(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_permission(Permission::ManagePayrollDraft)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    payroll_service::delete_run(&state.pool, company_id, id, auth.0.sub, Some(&audit_meta)).await?;

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn update_item_pcb(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Path((run_id, employee_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdatePayrollPcbRequest>,
) -> AppResult<Json<PayrollSummary>> {
    auth.require_permission(Permission::ManagePayrollDraft)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

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
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PayrollRun>> {
    auth.require_permission(Permission::SubmitPayroll)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;
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
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PayrollRun>> {
    auth.require_permission(Permission::ApprovePayroll)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;
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
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
    Json(req): Json<ReturnPayrollRunRequest>,
) -> AppResult<Json<PayrollRun>> {
    auth.require_permission(Permission::ApprovePayroll)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;
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
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PayrollRun>> {
    auth.require_permission(Permission::MarkPayrollPaid)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;
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
    audit_meta: AuditRequestMeta,
    Json(req): Json<CreatePayrollEntryRequest>,
) -> AppResult<Json<PayrollEntry>> {
    auth.require_permission(Permission::ManagePayrollDraft)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

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
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePayrollEntryRequest>,
) -> AppResult<Json<PayrollEntry>> {
    auth.require_permission(Permission::ManagePayrollDraft)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

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
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_permission(Permission::ManagePayrollDraft)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

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
    let company_id = auth.company_id()?;
    let items = payroll_service::list_items(&state.pool, company_id, id).await?;

    Ok(Json(items))
}
