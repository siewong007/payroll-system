//! Read model for EA-form (annual remuneration statement) generation: YTD
//! aggregations over payroll_items plus employee/company details.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::ea_form::{EaCompanyRow, EaEmployeeRow, EaEmployeeSummary, EaYtdTotals};

/// Per-employee YTD gross + months-worked for the EA employee picker.
pub async fn list_employee_summaries(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    year: i32,
) -> AppResult<Vec<EaEmployeeSummary>> {
    let rows = sqlx::query_as!(
        EaEmployeeSummary,
        r#"SELECT
            pi.employee_id,
            e.full_name AS employee_name,
            e.employee_number,
            e.ic_number,
            SUM(pi.gross_salary)::bigint AS "ytd_gross!",
            COUNT(DISTINCT pr.period_month) AS "months_worked!"
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        JOIN employees e ON pi.employee_id = e.id
        WHERE pr.company_id = $1 AND pr.period_year = $2
        AND pr.status::text IN ('approved', 'paid')
        AND e.deleted_at IS NULL
        GROUP BY pi.employee_id, e.full_name, e.employee_number, e.ic_number
        ORDER BY e.employee_number"#,
        company_id,
        year,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn company_for_ea(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<EaCompanyRow> {
    let company = sqlx::query_as!(
        EaCompanyRow,
        "SELECT name, registration_number, tax_number, epf_number, address_line1, address_line2, city, state, postcode FROM companies WHERE id = $1",
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(company)
}

pub async fn employee_for_ea(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<EaEmployeeRow>> {
    let emp = sqlx::query_as!(
        EaEmployeeRow,
        r#"SELECT full_name, employee_number, ic_number, tax_identification_number,
            epf_number, socso_number, address_line1, address_line2, city, state, postcode, date_joined
        FROM employees WHERE id = $1 AND company_id = $2"#,
        employee_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(emp)
}

/// YTD employment-income and statutory totals for one employee in a tax year.
pub async fn ytd_totals(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    company_id: Uuid,
    year: i32,
) -> AppResult<EaYtdTotals> {
    let agg = sqlx::query_as!(
        EaYtdTotals,
        r#"SELECT
            COALESCE(SUM(pi.basic_salary), 0)::bigint AS "ytd_basic!",
            COALESCE(SUM(pi.total_allowances), 0)::bigint AS "ytd_allowances!",
            COALESCE(SUM(pi.total_bonus), 0)::bigint AS "ytd_bonus!",
            COALESCE(SUM(pi.total_commission), 0)::bigint AS "ytd_commission!",
            COALESCE(SUM(pi.total_overtime), 0)::bigint AS "ytd_overtime!",
            COALESCE(SUM(pi.gross_salary), 0)::bigint AS "ytd_gross!",
            COALESCE(SUM(pi.epf_employee), 0)::bigint AS "ytd_epf_employee!",
            COALESCE(SUM(pi.socso_employee), 0)::bigint AS "ytd_socso_employee!",
            COALESCE(SUM(pi.eis_employee), 0)::bigint AS "ytd_eis_employee!",
            COALESCE(SUM(pi.pcb_amount), 0)::bigint AS "ytd_pcb!",
            COALESCE(SUM(pi.zakat_amount), 0)::bigint AS "ytd_zakat!",
            COUNT(DISTINCT pr.period_month) AS "months_worked!"
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        WHERE pi.employee_id = $1 AND pr.company_id = $2 AND pr.period_year = $3
        AND pr.status::text IN ('approved', 'paid')"#,
        employee_id,
        company_id,
        year,
    )
    .fetch_one(executor)
    .await?;
    Ok(agg)
}
