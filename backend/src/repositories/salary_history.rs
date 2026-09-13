//! Data access for the `salary_history` table.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::employee::SalaryHistory;

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    company_id: Uuid,
    old_salary: i64,
    new_salary: i64,
    change_type: &str,
    notes: Option<&str>,
    created_by: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO salary_history (id, employee_id, company_id, old_salary, new_salary, effective_date, change_type, notes, created_by)
                VALUES ($1, $2, $3, $4, $5, NOW()::date, $6, $7, $8)"#,
        id,
        employee_id,
        company_id,
        old_salary,
        new_salary,
        change_type,
        notes,
        created_by,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Record the initial salary for a bulk-imported employee (old_salary 0,
/// effective on their join date).
//
// NOTE: the `VALUES` line keeps its original indentation for byte-exact cache match.
pub async fn insert_bulk_import_initial(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    company_id: Uuid,
    new_salary: i64,
    effective_date: NaiveDate,
    created_by: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO salary_history (id, employee_id, company_id, old_salary, new_salary, effective_date, reason, change_type, created_by)
                    VALUES ($1, $2, $3, 0, $4, $5, 'Initial salary (bulk import)', 'new_hire', $6)"#,
        id,
        employee_id,
        company_id,
        new_salary,
        effective_date,
        created_by,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn list_by_employee(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
) -> AppResult<Vec<SalaryHistory>> {
    // Columns are listed explicitly rather than `SELECT *` so that the tenant
    // anchor added in 1009 stays out of `SalaryHistory` and off the
    // `GET /employees/:id/salary-history` payload.
    let history = sqlx::query_as!(
        SalaryHistory,
        r#"SELECT id, employee_id, old_salary, new_salary, effective_date, reason,
               change_type, notes, approved_by, approved_at, created_at, created_by
           FROM salary_history WHERE employee_id = $1 ORDER BY effective_date DESC"#,
        employee_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(history)
}
