//! Admin dashboard summary: head-counts, the latest payroll snapshot, YTD
//! employer cost, and department breakdown. Payroll figures are gated by the
//! caller's privilege (the `exec` role must not see them).

use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::dashboard::{DashboardSummary, DepartmentCount};
use crate::repositories::employees as employee_repo;
use crate::repositories::reads::dashboard as dashboard_reads;

/// Assemble the dashboard summary. When `can_access_payroll` is false, payroll
/// figures are blanked (None / 0) so the `exec` role never sees them.
pub async fn summary(
    pool: &PgPool,
    company_id: Uuid,
    can_access_payroll: bool,
) -> AppResult<DashboardSummary> {
    let total_employees = employee_repo::count(pool, company_id, None, None, None).await?;
    let active_employees = employee_repo::count(pool, company_id, None, None, Some(true)).await?;

    let last_payroll = dashboard_reads::last_payroll(pool, company_id).await?;

    let current_year = chrono::Utc::now().year();
    let ytd = dashboard_reads::ytd_employer_totals(pool, company_id, current_year).await?;

    let departments = dashboard_reads::department_counts(pool, company_id).await?;

    Ok(DashboardSummary {
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
    })
}
