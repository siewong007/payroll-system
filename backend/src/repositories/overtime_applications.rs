//! Data access for the `overtime_applications` table.

use chrono::{NaiveDate, NaiveTime};
use rust_decimal::Decimal;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::portal::OvertimeApplication;

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    company_id: Uuid,
    ot_date: NaiveDate,
    start_time: NaiveTime,
    end_time: NaiveTime,
    hours: Decimal,
    ot_type: &str,
    reason: Option<String>,
    created_by: Uuid,
) -> AppResult<OvertimeApplication> {
    let overtime = sqlx::query_as!(
        OvertimeApplication,
        r#"INSERT INTO overtime_applications
            (employee_id, company_id, ot_date, start_time, end_time, hours, ot_type, reason, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING *"#,
        employee_id,
        company_id,
        ot_date,
        start_time,
        end_time,
        hours,
        ot_type,
        reason,
        created_by,
    )
    .fetch_one(executor)
    .await?;
    Ok(overtime)
}

pub async fn lock_payable(
    executor: impl Executor<'_, Database = Postgres>,
    employee_ids: &[Uuid],
    company_id: Uuid,
    period_end: NaiveDate,
) -> AppResult<()> {
    if employee_ids.is_empty() {
        return Ok(());
    }
    sqlx::query!(
        r#"SELECT id FROM overtime_applications
        WHERE employee_id = ANY($1)
          AND company_id = $2
          AND status = 'approved'
          AND payroll_run_id IS NULL
          AND ot_date <= $3
        ORDER BY id
        FOR UPDATE"#,
        employee_ids,
        company_id,
        period_end,
    )
    .fetch_all(executor)
    .await?;
    Ok(())
}

/// Mark exactly the approved overtime applications a run paid, linking them to the run.
pub async fn mark_paid(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    ot_ids: &[Uuid],
) -> AppResult<u64> {
    if ot_ids.is_empty() {
        return Ok(0);
    }
    let res = sqlx::query!(
        r#"UPDATE overtime_applications
        SET status = 'processed', payroll_run_id = $1, updated_at = NOW()
        WHERE id = ANY($2) AND status = 'approved' AND payroll_run_id IS NULL"#,
        run_id,
        ot_ids,
    )
    .execute(executor)
    .await?;
    Ok(res.rows_affected())
}

/// Revert a run's paid overtime applications back to approved (used on delete/cancel/reverse).
pub async fn revert_for_run(
    executor: impl Executor<'_, Database = Postgres>,
    run_id: Uuid,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE overtime_applications
        SET status = 'approved', payroll_run_id = NULL, updated_at = NOW()
        WHERE payroll_run_id = $1 AND company_id = $2"#,
        run_id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Checks whether another non-rejected, non-cancelled overtime application for
/// the same employee overlaps the half-open `[start_time, end_time)` window on
/// `ot_date` (treating `end_time <= start_time` as crossing midnight, so yesterday's
/// overnight application overlaps early-morning applications today and vice versa).
pub async fn has_overlapping_window(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    ot_date: NaiveDate,
    start_time: NaiveTime,
    end_time: NaiveTime,
    exclude_id: Option<Uuid>,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar!(
        r#"WITH req AS (
            SELECT
                (EXTRACT(EPOCH FROM $3::time) / 60.0)::double precision AS s1,
                CASE
                    WHEN $4::time <= $3::time THEN (EXTRACT(EPOCH FROM $4::time) / 60.0)::double precision + 1440.0
                    ELSE (EXTRACT(EPOCH FROM $4::time) / 60.0)::double precision
                END AS e1
        )
        SELECT EXISTS(
            SELECT 1
            FROM overtime_applications oa
            CROSS JOIN req
            CROSS JOIN LATERAL (
                SELECT
                    ((oa.ot_date - $2::date) * 1440.0)::double precision
                        + (EXTRACT(EPOCH FROM oa.start_time) / 60.0)::double precision AS oa_start,
                    ((oa.ot_date - $2::date) * 1440.0)::double precision
                        + CASE
                            WHEN oa.end_time <= oa.start_time THEN (EXTRACT(EPOCH FROM oa.end_time) / 60.0)::double precision + 1440.0
                            ELSE (EXTRACT(EPOCH FROM oa.end_time) / 60.0)::double precision
                        END AS oa_end
            ) w
            WHERE oa.employee_id = $1
              AND oa.ot_date BETWEEN ($2::date - 1) AND ($2::date + 1)
              AND oa.status NOT IN ('rejected', 'cancelled')
              AND ($5::uuid IS NULL OR oa.id <> $5)
              AND req.s1 < w.oa_end
              AND w.oa_start < req.e1
        ) AS "exists!""#,
        employee_id,
        ot_date,
        start_time,
        end_time,
        exclude_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

pub async fn exists_replacement_granted_for_date(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    ot_date: NaiveDate,
    exclude_id: Uuid,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(
            SELECT 1 FROM overtime_applications
            WHERE employee_id = $1
              AND ot_date = $2
              AND ot_type = 'public_holiday'
              AND status = 'approved'
              AND id <> $3
        ) AS "exists!""#,
        employee_id,
        ot_date,
        exclude_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// A pending application, or `None` if missing/not editable.
pub async fn get_pending(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<OvertimeApplication>> {
    let ot = sqlx::query_as!(
        OvertimeApplication,
        r#"SELECT * FROM overtime_applications
        WHERE id = $1 AND company_id = $2 AND status = 'pending'"#,
        ot_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(ot)
}

/// Lock and load a pending application inside a write transaction.
pub async fn get_pending_locked(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<OvertimeApplication>> {
    let ot = sqlx::query_as!(
        OvertimeApplication,
        r#"SELECT * FROM overtime_applications
        WHERE id = $1 AND company_id = $2 AND status = 'pending'
        FOR UPDATE"#,
        ot_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(ot)
}

/// A cancelled application, or `None` if missing/not deletable.
pub async fn get_cancelled(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<OvertimeApplication>> {
    let ot = sqlx::query_as!(
        OvertimeApplication,
        r#"SELECT * FROM overtime_applications
        WHERE id = $1 AND company_id = $2
        AND status = 'cancelled'"#,
        ot_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(ot)
}

/// A cancellable application (pending/approved/rejected).
pub async fn get_cancellable(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<OvertimeApplication>> {
    let ot = sqlx::query_as!(
        OvertimeApplication,
        r#"SELECT * FROM overtime_applications
        WHERE id = $1
          AND company_id = $2
          AND status IN ('pending', 'approved', 'rejected')
          AND payroll_run_id IS NULL
        FOR UPDATE"#,
        ot_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(ot)
}

#[allow(clippy::too_many_arguments)]
pub async fn update_full(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    company_id: Uuid,
    employee_id: Option<Uuid>,
    ot_date: Option<NaiveDate>,
    start_time: NaiveTime,
    end_time: NaiveTime,
    hours: Option<Decimal>,
    ot_type: &str,
    reason: Option<String>,
    updated_by: Uuid,
) -> AppResult<Option<OvertimeApplication>> {
    let updated = sqlx::query_as!(
        OvertimeApplication,
        r#"UPDATE overtime_applications
        SET employee_id = COALESCE($3, employee_id),
            ot_date = COALESCE($4, ot_date),
            start_time = $5,
            end_time = $6,
            hours = COALESCE($7, hours),
            ot_type = $8,
            reason = CASE WHEN $9::text IS NULL THEN reason ELSE NULLIF($9, '') END,
            updated_by = $10,
            editor_ids = CASE
                WHEN $10 = ANY(editor_ids) THEN editor_ids
                ELSE array_append(editor_ids, $10)
            END,
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending'
        RETURNING *"#,
        ot_id,
        company_id,
        employee_id,
        ot_date,
        start_time,
        end_time,
        hours,
        ot_type,
        reason,
        updated_by,
    )
    .fetch_optional(executor)
    .await?;
    Ok(updated)
}

pub async fn delete(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    company_id: Uuid,
) -> AppResult<u64> {
    let res = sqlx::query!(
        "DELETE FROM overtime_applications WHERE id = $1 AND company_id = $2 AND status = 'cancelled'",
        ot_id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(res.rows_affected())
}

pub async fn set_cancelled(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    company_id: Uuid,
    expected_status: &str,
) -> AppResult<Option<OvertimeApplication>> {
    let cancelled = sqlx::query_as!(
        OvertimeApplication,
        r#"UPDATE overtime_applications
        SET status = 'cancelled', updated_at = NOW()
        WHERE id = $1 AND company_id = $2
          AND status = $3
          AND status <> 'processed'
          AND payroll_run_id IS NULL
        RETURNING *"#,
        ot_id,
        company_id,
        expected_status,
    )
    .fetch_optional(executor)
    .await?;
    Ok(cancelled)
}

/// Approve a pending application. `None` if not found or not pending.
pub async fn set_approved(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    company_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
) -> AppResult<Option<OvertimeApplication>> {
    let ot = sqlx::query_as!(
        OvertimeApplication,
        r#"UPDATE overtime_applications oa SET
            status = 'approved', reviewed_by = $3, reviewed_at = NOW(),
            review_notes = $4, updated_at = NOW()
        WHERE oa.id = $1 AND oa.company_id = $2 AND oa.status = 'pending'
          AND EXISTS (
              SELECT 1 FROM employees e
              WHERE e.id = oa.employee_id AND e.company_id = $2 AND e.deleted_at IS NULL
          )
        RETURNING oa.*"#,
        ot_id,
        company_id,
        reviewer_id,
        notes,
    )
    .fetch_optional(executor)
    .await?;
    Ok(ot)
}

/// Reject a pending application. `None` if not found or not pending.
pub async fn set_rejected(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    company_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
) -> AppResult<Option<OvertimeApplication>> {
    let ot = sqlx::query_as!(
        OvertimeApplication,
        r#"UPDATE overtime_applications SET
            status = 'rejected', reviewed_by = $3, reviewed_at = NOW(),
            review_notes = $4, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending'
        RETURNING *"#,
        ot_id,
        company_id,
        reviewer_id,
        notes,
    )
    .fetch_optional(executor)
    .await?;
    Ok(ot)
}

/// An employee's own overtime applications, newest first (max 50).
pub async fn list_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
) -> AppResult<Vec<OvertimeApplication>> {
    let apps = sqlx::query_as!(
        OvertimeApplication,
        r#"SELECT * FROM overtime_applications
        WHERE employee_id = $1
        ORDER BY created_at DESC
        LIMIT 50"#,
        employee_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(apps)
}

// ─── Self-service portal operations ───

pub async fn get_cancellable_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    employee_id: Uuid,
) -> AppResult<Option<OvertimeApplication>> {
    let ot = sqlx::query_as!(
        OvertimeApplication,
        r#"SELECT * FROM overtime_applications
        WHERE id = $1
          AND employee_id = $2
          AND status IN ('pending', 'approved', 'rejected')
          AND payroll_run_id IS NULL
        FOR UPDATE"#,
        ot_id,
        employee_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(ot)
}

pub async fn get_employee_id_for_company(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<Uuid>> {
    let employee_id = sqlx::query_scalar!(
        "SELECT employee_id FROM overtime_applications WHERE id = $1 AND company_id = $2",
        ot_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(employee_id)
}

pub async fn get_company_id_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    employee_id: Uuid,
) -> AppResult<Option<Uuid>> {
    let company_id = sqlx::query_scalar!(
        "SELECT company_id FROM overtime_applications WHERE id = $1 AND employee_id = $2",
        ot_id,
        employee_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(company_id)
}

pub async fn mark_cancelled_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    employee_id: Uuid,
    expected_status: &str,
) -> AppResult<bool> {
    let res = sqlx::query!(
        "UPDATE overtime_applications SET status = 'cancelled', updated_at = NOW()
         WHERE id = $1 AND employee_id = $2 AND status = $3
           AND status <> 'processed' AND payroll_run_id IS NULL",
        ot_id,
        employee_id,
        expected_status,
    )
    .execute(executor)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn delete_cancelled_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    employee_id: Uuid,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        "DELETE FROM overtime_applications WHERE id = $1 AND employee_id = $2 AND status = 'cancelled' AND reviewed_at IS NULL",
        ot_id,
        employee_id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}
