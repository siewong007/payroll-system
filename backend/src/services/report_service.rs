//! Admin reports. Query bodies live in `repositories::reads::reports`; this
//! module exposes thin wrappers plus the report-period picker assembly.

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::report::YearMonthsOption;
use crate::repositories::reads::reports as report_reads;

pub use crate::models::report::ReportPeriodsResponse;
pub use crate::models::report::{
    ClaimsReportRow, DepartmentPayrollRow, LeaveReportRow, PayrollSummaryRow, StatutoryReportRow,
};

pub async fn payroll_summary(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
) -> AppResult<Vec<PayrollSummaryRow>> {
    report_reads::payroll_summary(pool, company_id, year).await
}

pub async fn payroll_by_department(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
    month: i32,
) -> AppResult<Vec<DepartmentPayrollRow>> {
    report_reads::payroll_by_department(pool, company_id, year, month).await
}

pub async fn leave_report(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
) -> AppResult<Vec<LeaveReportRow>> {
    report_reads::leave_report(pool, company_id, year).await
}

pub async fn claims_report(
    pool: &PgPool,
    company_id: Uuid,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> AppResult<Vec<ClaimsReportRow>> {
    report_reads::claims_report(pool, company_id, start_date, end_date).await
}

pub async fn statutory_report(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
    month: i32,
) -> AppResult<Vec<StatutoryReportRow>> {
    report_reads::statutory_report(pool, company_id, year, month).await
}

// ─── Report Period Options ───

pub fn current_report_year_month() -> (i32, i32) {
    let now = Utc::now().date_naive();
    (now.year(), now.month() as i32)
}

pub async fn report_periods(pool: &PgPool, company_id: Uuid) -> AppResult<ReportPeriodsResponse> {
    let payroll_periods = report_reads::distinct_payroll_periods(pool, company_id).await?;

    let mut payroll_months_map = BTreeMap::<i32, Vec<i32>>::new();
    for row in payroll_periods {
        payroll_months_map
            .entry(row.period_year)
            .or_default()
            .push(row.period_month);
    }

    let payroll_years = payroll_months_map.keys().copied().collect::<Vec<_>>();
    let payroll_months = payroll_months_map
        .into_iter()
        .map(|(year, months)| YearMonthsOption { year, months })
        .collect::<Vec<_>>();

    let leave_years = report_reads::distinct_leave_years(pool, company_id).await?;

    let claims_years = report_reads::distinct_claims_years(pool, company_id).await?;

    let (default_year, default_month) = payroll_months
        .last()
        .and_then(|period| period.months.last().map(|month| (period.year, *month)))
        .unwrap_or_else(current_report_year_month);

    Ok(ReportPeriodsResponse {
        default_year,
        default_month,
        payroll_years: payroll_years.clone(),
        payroll_months,
        leave_years,
        claims_years,
        ea_form_years: payroll_years,
    })
}
