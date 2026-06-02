//! Data access for the `payroll_entries` table (staged variable earnings/deductions).

use rust_decimal::Decimal;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::payroll::PayrollEntry;

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    company_id: Uuid,
    period_year: i32,
    period_month: i32,
    category: &str,
    item_type: &str,
    description: &str,
    amount: i64,
    quantity: Option<Decimal>,
    rate: Option<i64>,
    is_taxable: Option<bool>,
    created_by: Uuid,
) -> AppResult<PayrollEntry> {
    let entry = sqlx::query_as!(
        PayrollEntry,
        r#"INSERT INTO payroll_entries
            (employee_id, company_id, period_year, period_month, category, item_type,
             description, amount, quantity, rate, is_taxable, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, COALESCE($11, TRUE), $12)
        RETURNING *"#,
        employee_id,
        company_id,
        period_year,
        period_month,
        category,
        item_type,
        description,
        amount,
        quantity,
        rate,
        is_taxable,
        created_by,
    )
    .fetch_one(executor)
    .await?;
    Ok(entry)
}

/// An unprocessed entry for a company, by id (returns `None` if missing or already processed).
pub async fn get_unprocessed(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<PayrollEntry>> {
    let entry = sqlx::query_as!(
        PayrollEntry,
        "SELECT * FROM payroll_entries WHERE id = $1 AND company_id = $2 AND is_processed = FALSE",
        id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(entry)
}

#[allow(clippy::too_many_arguments)]
pub async fn update(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    employee_id: Uuid,
    period_year: i32,
    period_month: i32,
    category: &str,
    item_type: &str,
    description: &str,
    amount: i64,
    quantity: Option<Decimal>,
    rate: Option<i64>,
    is_taxable: Option<bool>,
    updated_by: Uuid,
) -> AppResult<PayrollEntry> {
    let updated = sqlx::query_as!(
        PayrollEntry,
        r#"UPDATE payroll_entries
        SET employee_id = $3,
            period_year = $4,
            period_month = $5,
            category = $6,
            item_type = $7,
            description = $8,
            amount = $9,
            quantity = $10,
            rate = $11,
            is_taxable = COALESCE($12, is_taxable),
            updated_by = $13,
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND is_processed = FALSE
        RETURNING *"#,
        id,
        company_id,
        employee_id,
        period_year,
        period_month,
        category,
        item_type,
        description,
        amount,
        quantity,
        rate,
        is_taxable,
        updated_by,
    )
    .fetch_one(executor)
    .await?;
    Ok(updated)
}

pub async fn delete_unprocessed(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM payroll_entries WHERE id = $1 AND company_id = $2 AND is_processed = FALSE",
        id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Mark an employee's staged entries for a period as processed, attaching the run id.
pub async fn mark_processed(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    employee_id: Uuid,
    year: i32,
    month: i32,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE payroll_entries SET is_processed = TRUE, payroll_run_id = $1
        WHERE employee_id = $2 AND period_year = $3 AND period_month = $4 AND is_processed = FALSE"#,
        run_id,
        employee_id,
        year,
        month,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Revert a run's staged entries back to unprocessed (used when deleting a run).
pub async fn revert_for_run(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    company_id: Uuid,
    updated_by: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE payroll_entries
        SET is_processed = FALSE, payroll_run_id = NULL, updated_at = NOW(), updated_by = $3
        WHERE payroll_run_id = $1 AND company_id = $2"#,
        run_id,
        company_id,
        updated_by,
    )
    .execute(executor)
    .await?;
    Ok(())
}

// ─── Unpaid-leave deduction entries (staged on leave approval) ───

/// Whether a *processed* unpaid-leave deduction matching `description` already
/// exists for an employee (blocks cancelling already-paid leave).
//
// NOTE: indentation matches the byte-exact SQL in the offline `.sqlx` cache
// (this query was originally nested several blocks deep).
pub async fn exists_processed_unpaid_leave(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    company_id: Uuid,
    description: &str,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(
                    SELECT 1 FROM payroll_entries
                    WHERE employee_id = $1
                      AND company_id = $2
                      AND item_type = 'unpaid_leave'
                      AND description LIKE $3
                      AND is_processed = TRUE
                ) AS "exists!""#,
        employee_id,
        company_id,
        description,
    )
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// Delete the *unprocessed* unpaid-leave deduction matching `description`.
//
// NOTE: indentation matches the byte-exact SQL in the offline `.sqlx` cache.
pub async fn delete_unprocessed_unpaid_leave(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    company_id: Uuid,
    description: &str,
) -> AppResult<()> {
    sqlx::query!(
        r#"DELETE FROM payroll_entries
                WHERE employee_id = $1
                  AND company_id = $2
                  AND item_type = 'unpaid_leave'
                  AND description LIKE $3
                  AND is_processed = FALSE"#,
        employee_id,
        company_id,
        description,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Stage an unpaid-leave salary deduction.
//
// NOTE: indentation matches the byte-exact SQL in the offline `.sqlx` cache.
#[allow(clippy::too_many_arguments)]
pub async fn insert_unpaid_leave_deduction(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    company_id: Uuid,
    period_year: i32,
    period_month: i32,
    description: &str,
    amount: i64,
    created_by: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO payroll_entries
                            (id, employee_id, company_id, period_year, period_month, category, item_type, description, amount, created_by)
                        VALUES ($1, $2, $3, $4, $5, 'deduction', 'unpaid_leave', $6, $7, $8)"#,
        id,
        employee_id,
        company_id,
        period_year,
        period_month,
        description,
        amount,
        created_by,
    )
    .execute(executor)
    .await?;
    Ok(())
}
