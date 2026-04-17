use axum::{Json, extract::State};
use chrono::Datelike;
use serde::Serialize;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};

#[derive(Debug, Serialize)]
pub struct DashboardSummary {
    pub total_employees: i64,
    pub active_employees: i64,
    pub last_payroll_period: Option<String>,
    pub last_payroll_total_net: Option<i64>,
    pub last_payroll_total_gross: Option<i64>,
    pub last_payroll_employee_count: Option<i32>,
    pub ytd_total_gross: i64,
    pub ytd_total_epf_employer: i64,
    pub ytd_total_socso_employer: i64,
    pub ytd_total_eis_employer: i64,
    pub departments: Vec<DepartmentCount>,
}

#[derive(Debug, Serialize)]
pub struct DepartmentCount {
    pub department: String,
    pub count: i64,
}

pub async fn summary(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<DashboardSummary>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let total_employees: Option<i64> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM employees WHERE company_id = $1 AND deleted_at IS NULL",
    )
    .bind(company_id)
    .fetch_one(&state.pool)
    .await?;
    let total_employees = total_employees.unwrap_or(0);

    let active_employees: Option<i64> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM employees WHERE company_id = $1 AND is_active = TRUE AND deleted_at IS NULL",
    )
    .bind(company_id)
    .fetch_one(&state.pool)
    .await?;
    let active_employees = active_employees.unwrap_or(0);

    let last_payroll = sqlx::query_as::<_, (String, i64, i64, i32)>(
        r#"SELECT
            period_year::text || '-' || LPAD(period_month::text, 2, '0'),
            total_net, total_gross, employee_count
        FROM payroll_runs
        WHERE company_id = $1 AND status NOT IN ('cancelled', 'draft')
        ORDER BY period_year DESC, period_month DESC
        LIMIT 1"#,
    )
    .bind(company_id)
    .fetch_optional(&state.pool)
    .await?;

    let current_year = chrono::Utc::now().year();
    let ytd = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        r#"SELECT
            COALESCE(SUM(total_gross), 0)::BIGINT,
            COALESCE(SUM(total_epf_employer), 0)::BIGINT,
            COALESCE(SUM(total_socso_employer), 0)::BIGINT,
            COALESCE(SUM(total_eis_employer), 0)::BIGINT
        FROM payroll_runs
        WHERE company_id = $1 AND period_year = $2
        AND status NOT IN ('cancelled', 'draft')"#,
    )
    .bind(company_id)
    .bind(current_year)
    .fetch_one(&state.pool)
    .await?;

    let departments = sqlx::query_as::<_, (Option<String>, Option<i64>)>(
        r#"SELECT department, COUNT(*)
        FROM employees
        WHERE company_id = $1 AND is_active = TRUE AND deleted_at IS NULL
        GROUP BY department ORDER BY COUNT(*) DESC"#,
    )
    .bind(company_id)
    .fetch_all(&state.pool)
    .await?;

    let is_exec = auth.is_exec();

    Ok(Json(DashboardSummary {
        total_employees,
        active_employees,
        last_payroll_period: if is_exec {
            None
        } else {
            last_payroll.as_ref().map(|p| p.0.clone())
        },
        last_payroll_total_net: if is_exec {
            None
        } else {
            last_payroll.as_ref().map(|p| p.1)
        },
        last_payroll_total_gross: if is_exec {
            None
        } else {
            last_payroll.as_ref().map(|p| p.2)
        },
        last_payroll_employee_count: if is_exec {
            None
        } else {
            last_payroll.as_ref().map(|p| p.3)
        },
        ytd_total_gross: if is_exec { 0 } else { ytd.0 },
        ytd_total_epf_employer: if is_exec { 0 } else { ytd.1 },
        ytd_total_socso_employer: if is_exec { 0 } else { ytd.2 },
        ytd_total_eis_employer: if is_exec { 0 } else { ytd.3 },
        departments: departments
            .into_iter()
            .map(|(dept, count)| DepartmentCount {
                department: dept.unwrap_or_else(|| "Unassigned".into()),
                count: count.unwrap_or(0),
            })
            .collect(),
    }))
}
