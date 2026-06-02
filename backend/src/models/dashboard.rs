use serde::Serialize;

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
