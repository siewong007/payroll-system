use serde::{Deserialize, Serialize};

/// Query for `GET /api/dashboard/summary`.
#[derive(Debug, Deserialize)]
pub struct DashboardQuery {
    /// Rolling window, in local days, for the attendance exception counts.
    pub window_days: Option<i64>,
}

impl DashboardQuery {
    /// Resolved window length. Defaults to a week and is clamped to a quarter —
    /// past that the counts stop being a to-do list and become a report.
    pub fn window_days(&self) -> i64 {
        self.window_days.unwrap_or(7).clamp(1, 90)
    }
}

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
    /// What a manager needs to act on, for the window the dashboard requests.
    pub needs_attention: NeedsAttention,
}

/// Exception counts and queue depths — the "what went wrong and who do I chase"
/// half of the dashboard.
///
/// Attendance figures cover a rolling window (default: the last 7 local days);
/// approval queues are absolute, since a request pending since March is exactly
/// the thing that must not fall off a window.
#[derive(Debug, Serialize)]
pub struct NeedsAttention {
    pub window_days: i64,
    pub late_count: i64,
    pub absent_count: i64,
    /// Sessions never checked out — someone is still "at work" days later.
    pub open_session_count: i64,
    pub outside_geofence_count: i64,
    /// Records an administrator created or edited by hand.
    pub manual_entry_count: i64,
    pub pending_leave: i64,
    pub pending_claims: i64,
    pub pending_overtime: i64,
    /// Serialized form of [`NeedsAttention::is_clear`], so the client can render
    /// an all-clear without re-deriving the predicate and drifting from it.
    pub all_clear: bool,
}

impl NeedsAttention {
    /// Assemble the block from the two aggregate reads.
    pub fn new(
        window_days: i64,
        totals: AttendanceExceptionTotals,
        counts: PendingApprovalCounts,
    ) -> Self {
        let mut block = Self {
            window_days,
            late_count: totals.late_count,
            absent_count: totals.absent_count,
            open_session_count: totals.open_session_count,
            outside_geofence_count: totals.outside_geofence_count,
            manual_entry_count: totals.manual_entry_count,
            pending_leave: counts.pending_leave,
            pending_claims: counts.pending_claims,
            pending_overtime: counts.pending_overtime,
            all_clear: false,
        };
        block.all_clear = block.is_clear();
        block
    }

    /// True when there is nothing for the manager to act on, so the UI can show
    /// an all-clear rather than a row of zeroes.
    pub fn is_clear(&self) -> bool {
        self.late_count == 0
            && self.absent_count == 0
            && self.open_session_count == 0
            && self.outside_geofence_count == 0
            && self.manual_entry_count == 0
            && self.pending_leave == 0
            && self.pending_claims == 0
            && self.pending_overtime == 0
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct AttendanceExceptionTotals {
    pub late_count: i64,
    pub absent_count: i64,
    pub open_session_count: i64,
    pub outside_geofence_count: i64,
    pub manual_entry_count: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct PendingApprovalCounts {
    pub pending_leave: i64,
    pub pending_claims: i64,
    pub pending_overtime: i64,
}

#[derive(Debug, Serialize)]
pub struct DepartmentCount {
    pub department: String,
    pub count: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct LastPayrollRow {
    pub period: String,
    pub total_net: i64,
    pub total_gross: i64,
    pub employee_count: i32,
}

#[derive(Debug, sqlx::FromRow)]
pub struct YtdEmployerTotals {
    pub total_gross: i64,
    pub total_epf_employer: i64,
    pub total_socso_employer: i64,
    pub total_eis_employer: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct DepartmentCountRow {
    pub department: Option<String>,
    pub count: i64,
}
