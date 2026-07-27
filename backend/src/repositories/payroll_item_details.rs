//! Data access for the `payroll_item_details` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::payroll::{PayrollItemDetail, PayslipLine};

/// Insert a payslip's breakdown lines in one statement.
///
/// One round trip regardless of line count, because a run writes these for every
/// employee inside the payroll transaction — a per-line insert would reintroduce
/// exactly the per-employee chatter the statutory snapshot removed.
pub async fn insert_lines(
    executor: impl Executor<'_, Database = Postgres>,
    payroll_item_id: Uuid,
    lines: &[PayslipLine],
) -> AppResult<()> {
    if lines.is_empty() {
        return Ok(());
    }

    let categories: Vec<String> = lines.iter().map(|l| l.category.clone()).collect();
    let item_types: Vec<String> = lines.iter().map(|l| l.item_type.clone()).collect();
    let descriptions: Vec<String> = lines.iter().map(|l| l.description.clone()).collect();
    let amounts: Vec<i64> = lines.iter().map(|l| l.amount).collect();
    let taxable: Vec<bool> = lines.iter().map(|l| l.is_taxable).collect();
    let statutory: Vec<bool> = lines.iter().map(|l| l.is_statutory).collect();

    sqlx::query!(
        r#"INSERT INTO payroll_item_details
            (payroll_item_id, category, item_type, description, amount, is_taxable, is_statutory)
        SELECT $1, category, item_type, description, amount, is_taxable, is_statutory
        FROM UNNEST($2::varchar[], $3::varchar[], $4::varchar[], $5::bigint[], $6::bool[], $7::bool[])
            AS t(category, item_type, description, amount, is_taxable, is_statutory)"#,
        payroll_item_id,
        &categories,
        &item_types,
        &descriptions,
        &amounts,
        &taxable,
        &statutory,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Breakdown lines for one payslip, earnings before deductions.
pub async fn list_for_item(
    executor: impl Executor<'_, Database = Postgres>,
    payroll_item_id: Uuid,
) -> AppResult<Vec<PayrollItemDetail>> {
    let rows = sqlx::query_as!(
        PayrollItemDetail,
        r#"SELECT id, payroll_item_id, category, item_type, description, amount,
                  is_taxable, is_statutory, created_at
        FROM payroll_item_details
        WHERE payroll_item_id = $1
        ORDER BY category DESC, is_statutory, id"#,
        payroll_item_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Replace one payslip's PCB line so the stored breakdown matches an edited
/// `payroll_items.pcb_amount`.
///
/// Delete-then-insert rather than UPDATE: the engine drops zero-valued lines, so
/// a payslip whose PCB was 0 has no row to update, and an edit back to 0 must
/// remove the row again. Both halves are one statement so the breakdown is never
/// momentarily missing its PCB line, and the description matches the one
/// `build_payslip_lines` writes — a payslip should not read differently
/// depending on whether its PCB was edited.
pub async fn replace_pcb_line(
    executor: impl Executor<'_, Database = Postgres>,
    payroll_item_id: Uuid,
    amount: i64,
) -> AppResult<()> {
    sqlx::query!(
        r#"WITH removed AS (
            DELETE FROM payroll_item_details
            WHERE payroll_item_id = $1 AND item_type = 'pcb'
        )
        INSERT INTO payroll_item_details
            (payroll_item_id, category, item_type, description, amount, is_taxable, is_statutory)
        SELECT $1, 'deduction', 'pcb', 'PCB (monthly tax deduction)', $2::bigint, FALSE, TRUE
        WHERE $2::bigint <> 0"#,
        payroll_item_id,
        amount,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Delete all detail rows belonging to a run's payslip items.
pub async fn delete_for_run(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"DELETE FROM payroll_item_details pid
        USING payroll_items pi
        WHERE pid.payroll_item_id = pi.id
          AND pi.payroll_run_id = $1"#,
        run_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}
