use axum::{
    Json,
    extract::{Query, State},
};
use chrono::NaiveDate;
use serde::Deserialize;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::services::report_service::{self, *};

fn require_admin(auth: &AuthUser) -> AppResult<uuid::Uuid> {
    match auth.0.role.as_str() {
        "super_admin" | "admin" | "payroll_admin" | "hr_manager" | "finance" => Ok(auth
            .0
            .company_id
            .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?),
        _ => Err(AppError::Forbidden("Admin access required".into())),
    }
}

#[derive(Debug, Deserialize)]
pub struct YearQuery {
    pub year: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct YearMonthQuery {
    pub year: Option<i32>,
    pub month: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct DateRangeQuery {
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

pub async fn payroll_summary(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<YearQuery>,
) -> AppResult<Json<Vec<PayrollSummaryRow>>> {
    auth.deny_exec()?;
    let company_id = require_admin(&auth)?;
    let year = q.year.unwrap_or(2026);
    let rows = report_service::payroll_summary(&state.pool, company_id, year).await?;
    Ok(Json(rows))
}

pub async fn payroll_by_department(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<YearMonthQuery>,
) -> AppResult<Json<Vec<DepartmentPayrollRow>>> {
    auth.deny_exec()?;
    let company_id = require_admin(&auth)?;
    let year = q.year.unwrap_or(2026);
    let month = q.month.unwrap_or(1);
    let rows = report_service::payroll_by_department(&state.pool, company_id, year, month).await?;
    Ok(Json(rows))
}

pub async fn leave_report(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<YearQuery>,
) -> AppResult<Json<Vec<LeaveReportRow>>> {
    let company_id = require_admin(&auth)?;
    let year = q.year.unwrap_or(2026);
    let rows = report_service::leave_report(&state.pool, company_id, year).await?;
    Ok(Json(rows))
}

pub async fn claims_report(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<DateRangeQuery>,
) -> AppResult<Json<Vec<ClaimsReportRow>>> {
    let company_id = require_admin(&auth)?;
    let start = q
        .start_date
        .unwrap_or(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    let end = q
        .end_date
        .unwrap_or(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
    let rows = report_service::claims_report(&state.pool, company_id, start, end).await?;
    Ok(Json(rows))
}

pub async fn statutory_report(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<YearMonthQuery>,
) -> AppResult<Json<Vec<StatutoryReportRow>>> {
    auth.deny_exec()?;
    let company_id = require_admin(&auth)?;
    let year = q.year.unwrap_or(2026);
    let month = q.month.unwrap_or(1);
    let rows = report_service::statutory_report(&state.pool, company_id, year, month).await?;
    Ok(Json(rows))
}

// ─── Statutory File Exports ───

use crate::services::ea_form_service;
use crate::services::statutory_export_service;

pub async fn export_epf(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<YearMonthQuery>,
) -> Result<axum::response::Response, crate::core::error::AppError> {
    use axum::body::Body;
    use axum::http::{Response, StatusCode, header};

    auth.deny_exec()?;
    let company_id = require_admin(&auth)?;
    let year = q.year.unwrap_or(2026);
    let month = q.month.unwrap_or(1);
    let bytes = statutory_export_service::export_epf(&state.pool, company_id, year, month).await?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"EPF_Export_{}_{:02}.csv\"",
                year, month
            ),
        )
        .body(Body::from(bytes))
        .unwrap())
}

pub async fn export_socso(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<YearMonthQuery>,
) -> Result<axum::response::Response, crate::core::error::AppError> {
    use axum::body::Body;
    use axum::http::{Response, StatusCode, header};

    auth.deny_exec()?;
    let company_id = require_admin(&auth)?;
    let year = q.year.unwrap_or(2026);
    let month = q.month.unwrap_or(1);
    let bytes =
        statutory_export_service::export_socso(&state.pool, company_id, year, month).await?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"SOCSO_Export_{}_{:02}.csv\"",
                year, month
            ),
        )
        .body(Body::from(bytes))
        .unwrap())
}

pub async fn export_eis(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<YearMonthQuery>,
) -> Result<axum::response::Response, crate::core::error::AppError> {
    use axum::body::Body;
    use axum::http::{Response, StatusCode, header};

    auth.deny_exec()?;
    let company_id = require_admin(&auth)?;
    let year = q.year.unwrap_or(2026);
    let month = q.month.unwrap_or(1);
    let bytes = statutory_export_service::export_eis(&state.pool, company_id, year, month).await?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"EIS_Export_{}_{:02}.csv\"",
                year, month
            ),
        )
        .body(Body::from(bytes))
        .unwrap())
}

pub async fn export_pcb(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<YearMonthQuery>,
) -> Result<axum::response::Response, crate::core::error::AppError> {
    use axum::body::Body;
    use axum::http::{Response, StatusCode, header};

    auth.deny_exec()?;
    let company_id = require_admin(&auth)?;
    let year = q.year.unwrap_or(2026);
    let month = q.month.unwrap_or(1);
    let bytes =
        statutory_export_service::export_pcb_cp39(&state.pool, company_id, year, month).await?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"CP39_PCB_{}_{:02}.txt\"",
                year, month
            ),
        )
        .body(Body::from(bytes))
        .unwrap())
}

// ─── EA Form ───

#[derive(Debug, Deserialize)]
pub struct EaFormQuery {
    pub year: Option<i32>,
    pub employee_id: Option<uuid::Uuid>,
}

pub async fn list_ea_employees(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<YearQuery>,
) -> AppResult<Json<Vec<ea_form_service::EaEmployeeSummary>>> {
    auth.deny_exec()?;
    let company_id = require_admin(&auth)?;
    let year = q.year.unwrap_or(2026);
    let rows = ea_form_service::list_employees_for_ea(&state.pool, company_id, year).await?;
    Ok(Json(rows))
}

pub async fn get_ea_form(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<EaFormQuery>,
) -> Result<axum::response::Response, crate::core::error::AppError> {
    use axum::body::Body;
    use axum::http::{Response, StatusCode, header};

    auth.deny_exec()?;
    let company_id = require_admin(&auth)?;
    let year = q.year.unwrap_or(2026);
    let employee_id = q.employee_id.ok_or_else(|| {
        crate::core::error::AppError::BadRequest("employee_id is required".into())
    })?;

    let data =
        ea_form_service::get_ea_form_data(&state.pool, company_id, employee_id, year).await?;
    let bytes = ea_form_service::generate_ea_form_pdf(&data)?;

    let filename = format!("EA_Form_{}_{}.pdf", year, data.employee_number);
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(Body::from(bytes))
        .unwrap())
}
