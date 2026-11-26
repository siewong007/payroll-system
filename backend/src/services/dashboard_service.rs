//! Admin dashboard summary: head-counts, the latest payroll snapshot, YTD
//! employer cost, and department breakdown. Payroll figures are gated by the
//! caller's privilege (the `exec` role must not see them).

use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::core::timezone;
use crate::models::dashboard::{
    AttendanceExceptionTotals, DashboardSummary, DepartmentCount, NeedsAttention,
    PendingApprovalCounts, YtdEmployerTotals,
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
    can_view_attendance: bool,
    window_days: i64,
) -> AppResult<DashboardSummary> {
    // Resolve company timezone once so both YTD employer totals and the attendance
    // exception window use the tenant's local calendar (e.g. MYT on Jan 1 00:00–07:59).
    let tz = attendance_service::get_company_timezone(pool, company_id).await;
    let parsed_tz = timezone::parse(&tz).unwrap_or(chrono_tz::Asia::Kuala_Lumpur);
    let today = chrono::Utc::now().with_timezone(&parsed_tz).date_naive();
    let current_year = today.year();

    // Group queries into at most 2 concurrent pool branches (instead of 6 simultaneous
    // acquires) so concurrent dashboard loads do not saturate the 10-connection pool.
    let org_and_approvals_fut = async {
        let (total_employees, active_employees, departments) =
            dashboard_reads::employee_department_summary(pool, company_id).await?;
        let pending = if can_view_approvals {
            dashboard_reads::pending_approval_counts(pool, company_id).await?
        } else {
            PendingApprovalCounts {
                pending_leave: 0,
                pending_claims: 0,
                pending_overtime: 0,
            }
        };
        Ok::<_, crate::core::error::AppError>((
            total_employees,
            active_employees,
            departments,
            pending,
        ))
    };

    let payroll_and_exceptions_fut = async {
        let (last_payroll, ytd) = if can_access_payroll {
            let last = dashboard_reads::last_payroll(pool, company_id).await?;
            let ytd = dashboard_reads::ytd_employer_totals(pool, company_id, current_year).await?;
            (last, ytd)
        } else {
            (
                None,
                YtdEmployerTotals {
                    total_gross: 0,
                    total_epf_employer: 0,
                    total_socso_employer: 0,
                    total_eis_employer: 0,
                },
            )
        };
        let exceptions = if can_view_attendance {
            let date_from = today - chrono::Duration::days(window_days - 1);
            dashboard_reads::attendance_exception_totals(pool, company_id, &tz, date_from, today)
                .await?
        } else {
            AttendanceExceptionTotals {
                late_count: 0,
                absent_count: 0,
                open_session_count: 0,
                outside_geofence_count: 0,
                manual_entry_count: 0,
            }
        };
        Ok::<_, crate::core::error::AppError>((last_payroll, ytd, exceptions))
    };

    let (
        (total_employees, active_employees, departments, pending),
        (last_payroll, ytd, exceptions),
    ) = tokio::try_join!(org_and_approvals_fut, payroll_and_exceptions_fut)?;

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
