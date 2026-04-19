use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::AppResult;

// ─── Payroll Summary Report ───

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PayrollSummaryRow {
    pub period: String,
    pub employee_count: i32,
    pub total_gross: i64,
    pub total_net: i64,
    pub total_epf_employee: i64,
    pub total_epf_employer: i64,
    pub total_socso_employee: i64,
    pub total_socso_employer: i64,
    pub total_eis_employee: i64,
    pub total_eis_employer: i64,
    pub total_pcb: i64,
    pub total_zakat: i64,
    pub total_employer_cost: i64,
}

pub async fn payroll_summary(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
) -> AppResult<Vec<PayrollSummaryRow>> {
    let rows = sqlx::query_as::<_, PayrollSummaryRow>(
        r#"SELECT
            TO_CHAR(period_start, 'YYYY-MM') as period,
            employee_count, total_gross, total_net,
            total_epf_employee, total_epf_employer,
            total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer,
            total_pcb, total_zakat, total_employer_cost
        FROM payroll_runs
        WHERE company_id = $1 AND period_year = $2
        AND status::text IN ('processed', 'approved', 'paid')
        ORDER BY period_month ASC"#,
    )
    .bind(company_id)
    .bind(year)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ─── Payroll by Department ───

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DepartmentPayrollRow {
    pub department: Option<String>,
    pub employee_count: i64,
    pub total_gross: i64,
    pub total_net: i64,
    pub total_employer_cost: i64,
}

pub async fn payroll_by_department(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
    month: i32,
) -> AppResult<Vec<DepartmentPayrollRow>> {
    let rows = sqlx::query_as::<_, DepartmentPayrollRow>(
        r#"SELECT
            e.department,
            COUNT(DISTINCT pi.employee_id) as employee_count,
            COALESCE(SUM(pi.gross_salary), 0) as total_gross,
            COALESCE(SUM(pi.net_salary), 0) as total_net,
            COALESCE(SUM(pi.employer_cost), 0) as total_employer_cost
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        JOIN employees e ON pi.employee_id = e.id
        WHERE pr.company_id = $1 AND pr.period_year = $2 AND pr.period_month = $3
        AND pr.status::text IN ('processed', 'approved', 'paid')
        GROUP BY e.department
        ORDER BY total_gross DESC"#,
    )
    .bind(company_id)
    .bind(year)
    .bind(month)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ─── Leave Report ───

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LeaveReportRow {
    pub employee_name: String,
    pub employee_number: String,
    pub department: Option<String>,
    pub gender: Option<String>,
    pub marital_status: Option<String>,
    pub num_children: Option<i32>,
    pub leave_type_name: String,
    pub entitled_days: rust_decimal::Decimal,
    pub taken_days: rust_decimal::Decimal,
    pub pending_days: rust_decimal::Decimal,
    pub balance: rust_decimal::Decimal,
}

pub async fn leave_report(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
) -> AppResult<Vec<LeaveReportRow>> {
    let rows = sqlx::query_as::<_, LeaveReportRow>(
        r#"SELECT
            e.full_name as employee_name,
            e.employee_number,
            e.department,
            e.gender::text as gender,
            e.marital_status::text as marital_status,
            e.num_children,
            lt.name as leave_type_name,
            lb.entitled_days + lb.carried_forward as entitled_days,
            lb.taken_days,
            lb.pending_days,
            (lb.entitled_days + lb.carried_forward - lb.taken_days - lb.pending_days) as balance
        FROM leave_balances lb
        JOIN employees e ON lb.employee_id = e.id
        JOIN leave_types lt ON lb.leave_type_id = lt.id
        WHERE e.company_id = $1 AND lb.year = $2 AND e.is_active = TRUE
        ORDER BY e.full_name, lt.name"#,
    )
    .bind(company_id)
    .bind(year)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ─── Claims Report ───

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ClaimsReportRow {
    pub employee_name: String,
    pub employee_number: String,
    pub department: Option<String>,
    pub total_claims: i64,
    pub total_amount: i64,
    pub approved_count: i64,
    pub approved_amount: i64,
    pub pending_count: i64,
    pub pending_amount: i64,
    pub rejected_count: i64,
}

pub async fn claims_report(
    pool: &PgPool,
    company_id: Uuid,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> AppResult<Vec<ClaimsReportRow>> {
    let rows = sqlx::query_as::<_, ClaimsReportRow>(
        r#"SELECT
            e.full_name as employee_name,
            e.employee_number,
            e.department,
            COUNT(*) as total_claims,
            COALESCE(SUM(c.amount), 0) as total_amount,
            COUNT(*) FILTER (WHERE c.status = 'approved') as approved_count,
            COALESCE(SUM(c.amount) FILTER (WHERE c.status = 'approved'), 0) as approved_amount,
            COUNT(*) FILTER (WHERE c.status = 'pending') as pending_count,
            COALESCE(SUM(c.amount) FILTER (WHERE c.status = 'pending'), 0) as pending_amount,
            COUNT(*) FILTER (WHERE c.status = 'rejected') as rejected_count
        FROM claims c
        JOIN employees e ON c.employee_id = e.id
        WHERE c.company_id = $1 AND c.expense_date BETWEEN $2 AND $3
        GROUP BY e.id, e.full_name, e.employee_number, e.department
        ORDER BY total_amount DESC"#,
    )
    .bind(company_id)
    .bind(start_date)
    .bind(end_date)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ─── Statutory Report (EPF/SOCSO/EIS/PCB) ───

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct StatutoryReportRow {
    pub employee_name: String,
    pub employee_number: String,
    pub ic_number: Option<String>,
    pub epf_number: Option<String>,
    pub socso_number: Option<String>,
    pub basic_salary: i64,
    pub gross_salary: i64,
    pub epf_employee: i64,
    pub epf_employer: i64,
    pub socso_employee: i64,
    pub socso_employer: i64,
    pub eis_employee: i64,
    pub eis_employer: i64,
    pub pcb_amount: i64,
    pub zakat_amount: i64,
}

pub async fn statutory_report(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
    month: i32,
) -> AppResult<Vec<StatutoryReportRow>> {
    let rows = sqlx::query_as::<_, StatutoryReportRow>(
        r#"SELECT
            e.full_name as employee_name,
            e.employee_number,
            e.ic_number,
            e.epf_number,
            e.socso_number,
            pi.basic_salary,
            pi.gross_salary,
            pi.epf_employee, pi.epf_employer,
            pi.socso_employee, pi.socso_employer,
            pi.eis_employee, pi.eis_employer,
            pi.pcb_amount, pi.zakat_amount
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        JOIN employees e ON pi.employee_id = e.id
        WHERE pr.company_id = $1 AND pr.period_year = $2 AND pr.period_month = $3
        AND pr.status::text IN ('processed', 'approved', 'paid')
        ORDER BY e.employee_number"#,
    )
    .bind(company_id)
    .bind(year)
    .bind(month)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ─── Report Period Options ───

#[derive(Debug, Serialize)]
pub struct YearMonthsOption {
    pub year: i32,
    pub months: Vec<i32>,
}

#[derive(Debug, Serialize)]
pub struct ReportPeriodsResponse {
    pub default_year: i32,
    pub default_month: i32,
    pub payroll_years: Vec<i32>,
    pub payroll_months: Vec<YearMonthsOption>,
    pub leave_years: Vec<i32>,
    pub claims_years: Vec<i32>,
    pub ea_form_years: Vec<i32>,
}

#[derive(Debug, sqlx::FromRow)]
struct PayrollPeriodRow {
    period_year: i32,
    period_month: i32,
}

pub fn current_report_year_month() -> (i32, i32) {
    let now = Utc::now().date_naive();
    (now.year(), now.month() as i32)
}

pub async fn report_periods(pool: &PgPool, company_id: Uuid) -> AppResult<ReportPeriodsResponse> {
    let payroll_periods = sqlx::query_as::<_, PayrollPeriodRow>(
        r#"SELECT DISTINCT period_year, period_month
        FROM payroll_runs
        WHERE company_id = $1
        AND status::text IN ('processed', 'approved', 'paid')
        ORDER BY period_year ASC, period_month ASC"#,
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;

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

    let leave_years = sqlx::query_scalar::<_, i32>(
        r#"SELECT DISTINCT lb.year
        FROM leave_balances lb
        JOIN employees e ON lb.employee_id = e.id
        WHERE e.company_id = $1
        ORDER BY lb.year ASC"#,
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;

    let claims_years = sqlx::query_scalar::<_, i32>(
        r#"SELECT DISTINCT EXTRACT(YEAR FROM expense_date)::INT
        FROM claims
        WHERE company_id = $1
        ORDER BY EXTRACT(YEAR FROM expense_date)::INT ASC"#,
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;

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
