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

    let total_employees = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM employees WHERE company_id = $1 AND deleted_at IS NULL"#,
        company_id,
    )
    .fetch_one(&state.pool)
    .await?;

    let active_employees = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM employees WHERE company_id = $1 AND is_active = TRUE AND deleted_at IS NULL"#,
        company_id,
    )
    .fetch_one(&state.pool)
    .await?;

    let last_payroll = sqlx::query!(
        r#"SELECT
            period_year::text || '-' || LPAD(period_month::text, 2, '0') AS "period!",
            total_net, total_gross, employee_count
        FROM payroll_runs
        WHERE company_id = $1 AND status NOT IN ('cancelled', 'draft')
        ORDER BY period_year DESC, period_month DESC
        LIMIT 1"#,
        company_id,
    )
    .fetch_optional(&state.pool)
    .await?;

    let current_year = chrono::Utc::now().year();
    let ytd = sqlx::query!(
        r#"SELECT
            COALESCE(SUM(total_gross), 0)::BIGINT AS "total_gross!",
            COALESCE(SUM(total_epf_employer), 0)::BIGINT AS "total_epf_employer!",
            COALESCE(SUM(total_socso_employer), 0)::BIGINT AS "total_socso_employer!",
            COALESCE(SUM(total_eis_employer), 0)::BIGINT AS "total_eis_employer!"
        FROM payroll_runs
        WHERE company_id = $1 AND period_year = $2
        AND status NOT IN ('cancelled', 'draft')"#,
        company_id,
        current_year,
    )
    .fetch_one(&state.pool)
    .await?;

    let departments = sqlx::query!(
        r#"SELECT department, COUNT(*) AS "count!"
        FROM employees
        WHERE company_id = $1 AND is_active = TRUE AND deleted_at IS NULL
        GROUP BY department ORDER BY COUNT(*) DESC"#,
        company_id,
    )
    .fetch_all(&state.pool)
    .await?;

    let can_access_payroll = auth.is_payroll_privileged();

    Ok(Json(DashboardSummary {
        total_employees,
        active_employees,
        last_payroll_period: if can_access_payroll {
            last_payroll.as_ref().map(|p| p.period.clone())
        } else {
            None
        },
        last_payroll_total_net: if can_access_payroll {
            last_payroll.as_ref().map(|p| p.total_net)
        } else {
            None
        },
        last_payroll_total_gross: if can_access_payroll {
            last_payroll.as_ref().map(|p| p.total_gross)
        } else {
            None
        },
        last_payroll_employee_count: if can_access_payroll {
            last_payroll.as_ref().map(|p| p.employee_count)
        } else {
            None
        },
        ytd_total_gross: if can_access_payroll {
            ytd.total_gross
        } else {
            0
        },
        ytd_total_epf_employer: if can_access_payroll {
            ytd.total_epf_employer
        } else {
            0
        },
        ytd_total_socso_employer: if can_access_payroll {
            ytd.total_socso_employer
        } else {
            0
        },
        ytd_total_eis_employer: if can_access_payroll {
            ytd.total_eis_employer
        } else {
            0
        },
        departments: departments
            .into_iter()
            .map(|row| DepartmentCount {
                department: row.department.unwrap_or_else(|| "Unassigned".into()),
                count: row.count,
            })
            .collect(),
    }))
}
