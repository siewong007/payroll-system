//! Admin dashboard summary: head-counts, the latest payroll snapshot, YTD
//! employer cost, and department breakdown. Payroll figures are gated by the
//! caller's privilege (the `exec` role must not see them).

use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::core::timezone;
use crate::models::dashboard::{
    DashboardSummary, DepartmentCount, NeedsAttention, PendingApprovalCounts, YtdEmployerTotals,
};
use crate::repositories::reads::dashboard as dashboard_reads;
use crate::services::attendance_service;

/// Assemble the dashboard summary. When `can_access_payroll` is false, payroll
/// figures are blanked (None / 0) so the `exec` role never sees them.
/// When `can_view_approvals` is false (e.g. `finance`), pending approval counts
/// are zeroed so `all_clear` does not flag queues the caller cannot open.
///
/// `window_days` sizes the attendance half of `needs_attention`; the approval
/// queue depths it carries are deliberately unwindowed.
pub async fn summary(
    pool: &PgPool,
    company_id: Uuid,
    can_access_payroll: bool,
    can_view_approvals: bool,
    window_days: i64,
) -> AppResult<DashboardSummary> {
    let current_year = chrono::Utc::now().year();

    let payroll_fut = async {
        if can_access_payroll {
            tokio::try_join!(
                dashboard_reads::last_payroll(pool, company_id),
                dashboard_reads::ytd_employer_totals(pool, company_id, current_year),
            )
        } else {
            Ok((
                None,
                YtdEmployerTotals {
                    total_gross: 0,
                    total_epf_employer: 0,
                    total_socso_employer: 0,
                    total_eis_employer: 0,
                },
            ))
        }
    };

    let exceptions_fut = async {
        // Bucket the exception window on the company's own calendar, so "last 7
        // days" here means the same 7 days the attendance list shows.
        let tz = attendance_service::get_company_timezone(pool, company_id).await;
        let parsed_tz = timezone::parse(&tz).unwrap_or(chrono_tz::Asia::Kuala_Lumpur);
        let today = chrono::Utc::now().with_timezone(&parsed_tz).date_naive();
        let date_from = today - chrono::Duration::days(window_days - 1);
        dashboard_reads::attendance_exception_totals(pool, company_id, &tz, date_from, today).await
    };

    let pending_fut = async {
        if can_view_approvals {
            dashboard_reads::pending_approval_counts(pool, company_id).await
        } else {
            Ok(PendingApprovalCounts {
                pending_leave: 0,
                pending_claims: 0,
                pending_overtime: 0,
            })
        }
    };

    let (
        (total_employees, active_employees),
        (last_payroll, ytd),
        departments,
        pending,
        exceptions,
    ) = tokio::try_join!(
        dashboard_reads::employee_headcount(pool, company_id),
        payroll_fut,
        dashboard_reads::department_counts(pool, company_id),
        pending_fut,
        exceptions_fut,
    )?;

    let needs_attention = NeedsAttention::new(window_days, exceptions, pending);

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
        needs_attention,
    })
}
