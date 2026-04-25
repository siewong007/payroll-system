use axum::{
    Json,
    extract::{Query, State},
};
use chrono::{Datelike, NaiveDate, Utc};
use serde::Deserialize;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::services::report_service::{self, *};

fn require_admin(auth: &AuthUser) -> AppResult<uuid::Uuid> {
    if auth.has_any_role(&[
        "super_admin",
        "admin",
        "payroll_admin",
        "hr_manager",
        "finance",
    ]) {
        return auth.company_id();
    }
    Err(AppError::Forbidden("Admin access required".into()))
}

fn require_payroll_access(auth: &AuthUser) -> AppResult<uuid::Uuid> {
    auth.require_payroll_privileged()?;
    auth.company_id()
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

fn current_year_month() -> (i32, i32) {
    let now = Utc::now().date_naive();
    (now.year(), now.month() as i32)
}

fn current_year() -> i32 {
    current_year_month().0
}

pub async fn report_periods(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<ReportPeriodsResponse>> {
    let company_id = require_admin(&auth)?;
    let mut periods = report_service::report_periods(&state.pool, company_id).await?;
    if !auth.is_payroll_privileged() {
        periods.payroll_years.clear();
        periods.payroll_months.clear();
        periods.ea_form_years.clear();
    }
    Ok(Json(periods))
}

pub async fn payroll_summary(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<YearQuery>,
) -> AppResult<Json<Vec<PayrollSummaryRow>>> {
    let company_id = require_payroll_access(&auth)?;
    let year = q.year.unwrap_or_else(current_year);
    let rows = report_service::payroll_summary(&state.pool, company_id, year).await?;
    Ok(Json(rows))
}

pub async fn payroll_by_department(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<YearMonthQuery>,
) -> AppResult<Json<Vec<DepartmentPayrollRow>>> {
    let company_id = require_payroll_access(&auth)?;
    let (default_year, default_month) = current_year_month();
    let year = q.year.unwrap_or(default_year);
    let month = q.month.unwrap_or(default_month);
    let rows = report_service::payroll_by_department(&state.pool, company_id, year, month).await?;
    Ok(Json(rows))
}

pub async fn leave_report(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<YearQuery>,
) -> AppResult<Json<Vec<LeaveReportRow>>> {
    let company_id = require_admin(&auth)?;
    let year = q.year.unwrap_or_else(current_year);
    let rows = report_service::leave_report(&state.pool, company_id, year).await?;
    Ok(Json(rows))
}

pub async fn claims_report(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<DateRangeQuery>,
) -> AppResult<Json<Vec<ClaimsReportRow>>> {
    let company_id = require_admin(&auth)?;
    let current_year = current_year();
    let start = q
        .start_date
        .unwrap_or(NaiveDate::from_ymd_opt(current_year, 1, 1).unwrap());
    let end = q
        .end_date
        .unwrap_or(NaiveDate::from_ymd_opt(current_year, 12, 31).unwrap());
    let rows = report_service::claims_report(&state.pool, company_id, start, end).await?;
    Ok(Json(rows))
}

pub async fn statutory_report(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<YearMonthQuery>,
) -> AppResult<Json<Vec<StatutoryReportRow>>> {
    let company_id = require_payroll_access(&auth)?;
    let (default_year, default_month) = current_year_month();
    let year = q.year.unwrap_or(default_year);
    let month = q.month.unwrap_or(default_month);
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

    let company_id = require_payroll_access(&auth)?;
    let (default_year, default_month) = current_year_month();
    let year = q.year.unwrap_or(default_year);
    let month = q.month.unwrap_or(default_month);
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

    let company_id = require_payroll_access(&auth)?;
    let (default_year, default_month) = current_year_month();
    let year = q.year.unwrap_or(default_year);
    let month = q.month.unwrap_or(default_month);
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

    let company_id = require_payroll_access(&auth)?;
    let (default_year, default_month) = current_year_month();
    let year = q.year.unwrap_or(default_year);
    let month = q.month.unwrap_or(default_month);
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

    let company_id = require_payroll_access(&auth)?;
    let (default_year, default_month) = current_year_month();
    let year = q.year.unwrap_or(default_year);
    let month = q.month.unwrap_or(default_month);
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
    let company_id = require_payroll_access(&auth)?;
    let year = q.year.unwrap_or_else(current_year);
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

    let company_id = require_payroll_access(&auth)?;
    let year = q.year.unwrap_or_else(current_year);
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
