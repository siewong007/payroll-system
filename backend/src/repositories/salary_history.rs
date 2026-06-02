//! Data access for the `salary_history` table.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::employee::SalaryHistory;

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    old_salary: i64,
    new_salary: i64,
    created_by: Uuid,
) -> AppResult<()> {
    // NOTE: the `VALUES` line is indented to match the byte-exact SQL stored in the
    // offline `.sqlx` cache (query hashing is whitespace-sensitive). Do not reflow it.
    sqlx::query!(
        r#"INSERT INTO salary_history (id, employee_id, old_salary, new_salary, effective_date, created_by)
                VALUES ($1, $2, $3, $4, NOW()::date, $5)"#,
        id,
        employee_id,
        old_salary,
        new_salary,
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
    new_salary: i64,
    effective_date: NaiveDate,
    created_by: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO salary_history (id, employee_id, old_salary, new_salary, effective_date, reason, created_by)
                    VALUES ($1, $2, 0, $3, $4, 'Initial salary (bulk import)', $5)"#,
        id,
        employee_id,
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
    let history = sqlx::query_as!(
        SalaryHistory,
        "SELECT * FROM salary_history WHERE employee_id = $1 ORDER BY effective_date DESC",
        employee_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(history)
}
