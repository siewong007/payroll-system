//! Read model for statutory exports (EPF/SOCSO/EIS/PCB files): per-employee
//! contribution rows for an approved/paid period, plus the employer's statutory codes.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::statutory::{CompanyStatutoryInfo, StatutoryRow};

pub async fn company_statutory_info(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<CompanyStatutoryInfo> {
    let company = sqlx::query_as!(
        CompanyStatutoryInfo,
        "SELECT name, epf_number, socso_code, eis_code, tax_number FROM companies WHERE id = $1",
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(company)
}

/// Per-employee statutory contribution rows for an approved/paid period.
pub async fn statutory_rows(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    year: i32,
    month: i32,
) -> AppResult<Vec<StatutoryRow>> {
    let rows = sqlx::query_as!(
        StatutoryRow,
        r#"SELECT
            e.full_name AS employee_name, e.ic_number,
            e.tax_identification_number, e.epf_number, e.socso_number, e.eis_number,
            pi.gross_salary,
            pi.epf_employee, pi.epf_employer,
            pi.socso_employee, pi.socso_employer,
            pi.eis_employee, pi.eis_employer,
            pi.pcb_amount
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        JOIN employees e ON pi.employee_id = e.id
        WHERE pr.company_id = $1 AND pr.period_year = $2 AND pr.period_month = $3
        AND pr.status::text IN ('approved', 'paid')
        ORDER BY e.employee_number"#,
        company_id,
        year,
        month,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}
