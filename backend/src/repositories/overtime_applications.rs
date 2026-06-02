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
) -> AppResult<OvertimeApplication> {
    let overtime = sqlx::query_as!(
        OvertimeApplication,
        r#"INSERT INTO overtime_applications
            (employee_id, company_id, ot_date, start_time, end_time, hours, ot_type, reason)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING *"#,
        employee_id,
        company_id,
        ot_date,
        start_time,
        end_time,
        hours,
        ot_type,
        reason,
    )
    .fetch_one(executor)
    .await?;
    Ok(overtime)
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
          AND status IN ('pending', 'approved', 'rejected')"#,
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
) -> AppResult<OvertimeApplication> {
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
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2
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
    )
    .fetch_one(executor)
    .await?;
    Ok(updated)
}

pub async fn delete(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM overtime_applications WHERE id = $1 AND company_id = $2",
        ot_id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn set_cancelled(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    company_id: Uuid,
) -> AppResult<OvertimeApplication> {
    let cancelled = sqlx::query_as!(
        OvertimeApplication,
        r#"UPDATE overtime_applications
        SET status = 'cancelled', updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *"#,
        ot_id,
        company_id,
    )
    .fetch_one(executor)
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
        r#"UPDATE overtime_applications SET
            status = 'approved', reviewed_by = $3, reviewed_at = NOW(),
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
          AND status IN ('pending', 'approved', 'rejected')"#,
        ot_id,
        employee_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(ot)
}

pub async fn mark_cancelled_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE overtime_applications SET status = 'cancelled', updated_at = NOW() WHERE id = $1",
        ot_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn delete_cancelled_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    ot_id: Uuid,
    employee_id: Uuid,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        "DELETE FROM overtime_applications WHERE id = $1 AND employee_id = $2 AND status = 'cancelled'",
        ot_id,
        employee_id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}
