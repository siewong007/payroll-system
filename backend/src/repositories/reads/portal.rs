//! Read models for the employee self-service portal: payslip summaries (payroll
//! items joined to their run) and leave balances joined to their type.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::portal::{LeaveBalanceWithType, LeaveRequest, MyPayslip, TeamLeaveEntry};

/// An employee's payslips for approved/paid runs, newest period first.
pub async fn my_payslips(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
) -> AppResult<Vec<MyPayslip>> {
    let payslips = sqlx::query_as!(
        MyPayslip,
        r#"SELECT pi.id, pi.payroll_run_id,
            pr.period_year, pr.period_month, pr.period_start, pr.period_end, pr.pay_date,
            pi.basic_salary, pi.gross_salary, pi.total_allowances, pi.total_overtime,
            pi.total_bonus, pi.total_commission, pi.total_claims,
            pi.epf_employee, pi.epf_employer, pi.socso_employee, pi.socso_employer,
            pi.eis_employee, pi.eis_employer, pi.pcb_amount, pi.zakat_amount,
            pi.ptptn_amount, pi.tabung_haji_amount, pi.total_loan_deductions,
            pi.total_other_deductions, pi.unpaid_leave_deduction,
            pi.total_deductions, pi.net_salary, pi.employer_cost,
            pi.ytd_gross, pi.ytd_epf_employee, pi.ytd_pcb,
            pi.ytd_socso_employee, pi.ytd_eis_employee, pi.ytd_zakat, pi.ytd_net
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        WHERE pi.employee_id = $1
        AND pr.status::text IN ('approved', 'paid')
        ORDER BY pr.period_year DESC, pr.period_month DESC"#,
        employee_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(payslips)
}

/// An employee's leave balances for a year, with the type name/paid flag.
pub async fn leave_balances_with_type(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    year: i32,
) -> AppResult<Vec<LeaveBalanceWithType>> {
    let balances = sqlx::query_as!(
        LeaveBalanceWithType,
        r#"SELECT lb.id, lb.leave_type_id, lt.name AS leave_type_name, lt.is_paid,
            lb.year, lb.entitled_days, lb.taken_days, lb.pending_days, lb.carried_forward
        FROM leave_balances lb
        JOIN leave_types lt ON lb.leave_type_id = lt.id
        WHERE lb.employee_id = $1 AND lb.year = $2 AND lt.is_active = TRUE
        ORDER BY lt.name"#,
        employee_id,
        year,
    )
    .fetch_all(executor)
    .await?;
    Ok(balances)
}

/// Approved leave requests for a year for an employee.
pub async fn approved_leaves_for_year(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    year: i32,
) -> AppResult<Vec<LeaveRequest>> {
    let leaves = sqlx::query_as!(
        LeaveRequest,
        r#"SELECT lr.id, lr.employee_id, lr.company_id, lr.leave_type_id,
            lr.start_date, lr.end_date, lr.days, lr.reason, lr.status,
            lr.reviewed_by, lr.reviewed_at, lr.review_notes,
            lr.attachment_url, lr.attachment_name,
            lr.created_at, lr.updated_at,
            lt.name AS "leave_type_name?"
        FROM leave_requests lr
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        WHERE lr.employee_id = $1 AND lr.status = 'approved'
        AND EXTRACT(YEAR FROM lr.start_date)::int = $2
        ORDER BY lr.start_date"#,
        employee_id,
        year,
    )
    .fetch_all(executor)
    .await?;
    Ok(leaves)
}

/// Team calendar for a given month, showing approved leaves for teammates.
pub async fn team_calendar(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    company_id: Uuid,
    period_start: chrono::NaiveDate,
    period_end: chrono::NaiveDate,
) -> AppResult<Vec<TeamLeaveEntry>> {
    let entries = sqlx::query_as!(
        TeamLeaveEntry,
        r#"SELECT DISTINCT lr.id, lr.employee_id, e.full_name AS employee_name,
            e.department, lt.name AS leave_type_name,
            lr.start_date, lr.end_date, lr.days, lr.status
        FROM leave_requests lr
        JOIN employees e ON lr.employee_id = e.id
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        WHERE e.company_id = $1
          AND lr.status = 'approved'
          AND lr.start_date <= $4
          AND lr.end_date >= $3
          AND (lr.employee_id = $2
               OR lr.employee_id IN (
                  SELECT tm2.employee_id FROM team_members tm2
                  WHERE tm2.team_id IN (
                      SELECT tm1.team_id FROM team_members tm1
                      WHERE tm1.employee_id = $2
                  )
              ))
        ORDER BY lr.start_date, e.full_name"#,
        company_id,
        employee_id,
        period_start,
        period_end,
    )
    .fetch_all(executor)
    .await?;
    Ok(entries)
}
