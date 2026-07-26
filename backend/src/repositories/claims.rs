//! Data access for the `claims` table.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::portal::Claim;

/// Mark exactly the claims a run reimbursed as paid, linking them to the run.
///
/// By id, not by predicate. The engine used to sum a period-bounded predicate
/// and then re-run it to mark rows processed, so a claim approved between the
/// two was marked paid without ever being paid. `status = 'approved' AND
/// payroll_run_id IS NULL` is kept as a compare-and-swap so a row a concurrent
/// run already took is left alone rather than double-claimed.
pub async fn mark_paid(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    claim_ids: &[Uuid],
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE claims
        SET status = 'processed', payroll_run_id = $1, updated_at = NOW()
        WHERE id = ANY($2) AND status = 'approved' AND payroll_run_id IS NULL"#,
        run_id,
        claim_ids,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Revert a run's paid claims back to approved (used when deleting a run).
///
/// By run id, which is exact. A period-bounded revert cannot express this once
/// selection is carry-forward: a run legitimately pays claims incurred before
/// its own period, and un-processing by period would revert claims an *earlier*
/// run already paid. The old employee-set subquery against `payroll_items` also
/// forced an ordering constraint against `payroll_items::delete_for_run`; this
/// has none.
pub async fn revert_for_run(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE claims
        SET status = 'approved', payroll_run_id = NULL, updated_at = NOW()
        WHERE payroll_run_id = $1 AND company_id = $2"#,
        run_id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

// ─── Admin CRUD + approval workflow ───

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    company_id: Uuid,
    title: String,
    description: Option<String>,
    amount: i64,
    category: Option<String>,
    receipt_url: Option<String>,
    receipt_file_name: Option<String>,
    expense_date: NaiveDate,
) -> AppResult<Claim> {
    let claim = sqlx::query_as!(
        Claim,
        r#"INSERT INTO claims
            (employee_id, company_id, title, description, amount, category, receipt_url, receipt_file_name, expense_date, status, submitted_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending', NOW())
        RETURNING *"#,
        employee_id,
        company_id,
        title,
        description,
        amount,
        category,
        receipt_url,
        receipt_file_name,
        expense_date,
    )
    .fetch_one(executor)
    .await?;
    Ok(claim)
}

/// An editable claim (draft/pending/rejected), or `None`.
pub async fn get_editable(
    executor: impl Executor<'_, Database = Postgres>,
    claim_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<Claim>> {
    let claim = sqlx::query_as!(
        Claim,
        r#"SELECT * FROM claims
        WHERE id = $1 AND company_id = $2
        AND status IN ('draft', 'pending', 'rejected')"#,
        claim_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(claim)
}

/// A deletable claim (draft/cancelled), or `None`.
pub async fn get_deletable(
    executor: impl Executor<'_, Database = Postgres>,
    claim_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<Claim>> {
    let claim = sqlx::query_as!(
        Claim,
        r#"SELECT * FROM claims
        WHERE id = $1 AND company_id = $2
        AND status IN ('draft', 'cancelled')"#,
        claim_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(claim)
}

/// A cancellable claim (pending/approved/rejected), or `None`.
pub async fn get_cancellable(
    executor: impl Executor<'_, Database = Postgres>,
    claim_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<Claim>> {
    let claim = sqlx::query_as!(
        Claim,
        r#"SELECT * FROM claims
        WHERE id = $1
          AND company_id = $2
          AND status IN ('pending', 'approved', 'rejected')"#,
        claim_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(claim)
}

#[allow(clippy::too_many_arguments)]
pub async fn update_full(
    executor: impl Executor<'_, Database = Postgres>,
    claim_id: Uuid,
    company_id: Uuid,
    employee_id: Option<Uuid>,
    title: Option<String>,
    description: Option<String>,
    amount: Option<i64>,
    category: Option<String>,
    receipt_url: Option<String>,
    receipt_file_name: Option<String>,
    expense_date: Option<NaiveDate>,
) -> AppResult<Claim> {
    let updated = sqlx::query_as!(
        Claim,
        r#"UPDATE claims
        SET employee_id = COALESCE($3, employee_id),
            title = COALESCE($4, title),
            description = CASE WHEN $5::text IS NULL THEN description ELSE NULLIF($5, '') END,
            amount = COALESCE($6, amount),
            category = CASE WHEN $7::text IS NULL THEN category ELSE NULLIF($7, '') END,
            receipt_url = CASE WHEN $8::text IS NULL THEN receipt_url ELSE NULLIF($8, '') END,
            receipt_file_name = CASE WHEN $9::text IS NULL THEN receipt_file_name ELSE NULLIF($9, '') END,
            expense_date = COALESCE($10, expense_date),
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *"#,
        claim_id,
        company_id,
        employee_id,
        title,
        description,
        amount,
        category,
        receipt_url,
        receipt_file_name,
        expense_date,
    )
    .fetch_one(executor)
    .await?;
    Ok(updated)
}

pub async fn delete(
    executor: impl Executor<'_, Database = Postgres>,
    claim_id: Uuid,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM claims WHERE id = $1 AND company_id = $2",
        claim_id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Cancel a claim that is still cancellable. `None` if it is not.
///
/// A compare-and-swap rather than an unconditional UPDATE: the status was read
/// on an earlier statement, so a run committing in between could otherwise have
/// its just-paid ('processed') claim cancelled out from under it — the money is
/// already in the payslip.
pub async fn set_cancelled(
    executor: impl Executor<'_, Database = Postgres>,
    claim_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<Claim>> {
    let cancelled = sqlx::query_as!(
        Claim,
        r#"UPDATE claims
        SET status = 'cancelled', updated_at = NOW()
        WHERE id = $1 AND company_id = $2
          AND status IN ('pending', 'approved', 'rejected')
        RETURNING *"#,
        claim_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(cancelled)
}

/// Approve a pending claim. `None` if not found or not pending.
pub async fn set_approved(
    executor: impl Executor<'_, Database = Postgres>,
    claim_id: Uuid,
    company_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
) -> AppResult<Option<Claim>> {
    let claim = sqlx::query_as!(
        Claim,
        r#"UPDATE claims SET
            status = 'approved', reviewed_by = $3, reviewed_at = NOW(),
            review_notes = $4, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending'
        RETURNING *"#,
        claim_id,
        company_id,
        reviewer_id,
        notes,
    )
    .fetch_optional(executor)
    .await?;
    Ok(claim)
}

/// Reject a pending claim. `None` if not found or not pending.
pub async fn set_rejected(
    executor: impl Executor<'_, Database = Postgres>,
    claim_id: Uuid,
    company_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
) -> AppResult<Option<Claim>> {
    let claim = sqlx::query_as!(
        Claim,
        r#"UPDATE claims SET
            status = 'rejected', reviewed_by = $3, reviewed_at = NOW(),
            review_notes = $4, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending'
        RETURNING *"#,
        claim_id,
        company_id,
        reviewer_id,
        notes,
    )
    .fetch_optional(executor)
    .await?;
    Ok(claim)
}

/// An employee's own claims, optionally filtered by status, newest first (max 100).
pub async fn list_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    status: Option<&str>,
) -> AppResult<Vec<Claim>> {
    let claims = sqlx::query_as!(
        Claim,
        r#"SELECT * FROM claims
        WHERE employee_id = $1
        AND ($2::text IS NULL OR status = $2)
        ORDER BY created_at DESC
        LIMIT 100"#,
        employee_id,
        status,
    )
    .fetch_all(executor)
    .await?;
    Ok(claims)
}

// ─── Self-service portal operations ───

#[allow(clippy::too_many_arguments)]
pub async fn insert_draft(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    company_id: Uuid,
    title: String,
    description: Option<String>,
    amount: i64,
    category: Option<String>,
    receipt_url: Option<String>,
    receipt_file_name: Option<String>,
    expense_date: NaiveDate,
) -> AppResult<Claim> {
    let claim = sqlx::query_as!(
        Claim,
        r#"INSERT INTO claims (employee_id, company_id, title, description, amount, category, receipt_url, receipt_file_name, expense_date)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING *"#,
        employee_id,
        company_id,
        title,
        description,
        amount,
        category,
        receipt_url,
        receipt_file_name,
        expense_date,
    )
    .fetch_one(executor)
    .await?;
    Ok(claim)
}

pub async fn mark_submitted(
    executor: impl Executor<'_, Database = Postgres>,
    claim_id: Uuid,
    employee_id: Uuid,
) -> AppResult<Option<Claim>> {
    let claim = sqlx::query_as!(
        Claim,
        r#"UPDATE claims SET status = 'pending', submitted_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND employee_id = $2 AND status = 'draft'
        RETURNING *"#,
        claim_id,
        employee_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(claim)
}

pub async fn get_cancellable_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    claim_id: Uuid,
    employee_id: Uuid,
) -> AppResult<Option<Claim>> {
    let claim = sqlx::query_as!(
        Claim,
        r#"SELECT * FROM claims
        WHERE id = $1
          AND employee_id = $2
          AND status IN ('pending', 'approved', 'rejected')"#,
        claim_id,
        employee_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(claim)
}

/// Cancel an employee's own claim. `false` if it is no longer cancellable.
///
/// Same compare-and-swap as `set_cancelled`, for the same reason: a run
/// committing between the read and this write must not have its paid claim
/// cancelled.
pub async fn mark_cancelled(
    executor: impl Executor<'_, Database = Postgres>,
    claim_id: Uuid,
) -> AppResult<bool> {
    let rows = sqlx::query!(
        r#"UPDATE claims SET status = 'cancelled', updated_at = NOW()
        WHERE id = $1 AND status IN ('pending', 'approved', 'rejected')"#,
        claim_id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows > 0)
}

pub async fn delete_draft_or_cancelled(
    executor: impl Executor<'_, Database = Postgres>,
    claim_id: Uuid,
    employee_id: Uuid,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        "DELETE FROM claims WHERE id = $1 AND employee_id = $2 AND status IN ('draft', 'cancelled')",
        claim_id,
        employee_id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}
