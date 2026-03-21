use axum::{
    extract::{Query, State},
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::services::report_service::{self, *};

fn require_admin(auth: &AuthUser) -> AppResult<uuid::Uuid> {
    match auth.0.role.as_str() {
        "super_admin" | "admin" | "payroll_admin" | "hr_manager" | "finance" | "exec" => Ok(auth
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
    let start = q.start_date.unwrap_or(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    let end = q.end_date.unwrap_or(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
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
