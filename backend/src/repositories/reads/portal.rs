//! Read models for the employee self-service portal: payslip summaries (payroll
//! items joined to their run) and leave balances joined to their type.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::portal::{LeaveBalanceWithType, MyPayslip};

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
