//! Data access for the `leave_requests` table. Single-row reads here join
//! `leave_types` only to denormalize `leave_type_name` onto the `LeaveRequest`
//! model; the employee-facing list view lives in `reads::approvals`.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::portal::LeaveRequest;

#[allow(clippy::too_many_arguments)]
pub async fn insert_with_type(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
    company_id: Uuid,
    leave_type_id: Uuid,
    start_date: NaiveDate,
    end_date: NaiveDate,
    days: Decimal,
    reason: Option<String>,
    attachment_url: Option<String>,
    attachment_name: Option<String>,
) -> AppResult<LeaveRequest> {
    let leave = sqlx::query_as!(
        LeaveRequest,
        r#"WITH new_lr AS (
            INSERT INTO leave_requests
                (employee_id, company_id, leave_type_id, start_date, end_date, days, reason, attachment_url, attachment_name)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
        )
        SELECT nlr.id, nlr.employee_id, nlr.company_id, nlr.leave_type_id,
            nlr.start_date, nlr.end_date, nlr.days, nlr.reason, nlr.status,
            nlr.reviewed_by, nlr.reviewed_at, nlr.review_notes,
            nlr.attachment_url, nlr.attachment_name,
            nlr.created_at, nlr.updated_at,
            lt.name AS "leave_type_name?"
        FROM new_lr nlr
        JOIN leave_types lt ON nlr.leave_type_id = lt.id"#,
        employee_id,
        company_id,
        leave_type_id,
        start_date,
        end_date,
        days,
        reason,
        attachment_url,
        attachment_name,
    )
    .fetch_one(executor)
    .await?;
    Ok(leave)
}

/// A pending request (with its type name), or `None` if missing/not editable.
pub async fn get_pending_with_type(
    executor: impl Executor<'_, Database = Postgres>,
    request_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<LeaveRequest>> {
    let leave = sqlx::query_as!(
        LeaveRequest,
        r#"SELECT lr.id, lr.employee_id, lr.company_id, lr.leave_type_id,
            lr.start_date, lr.end_date, lr.days, lr.reason, lr.status,
            lr.reviewed_by, lr.reviewed_at, lr.review_notes,
            lr.attachment_url, lr.attachment_name,
            lr.created_at, lr.updated_at,
            lt.name AS "leave_type_name?"
        FROM leave_requests lr
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        WHERE lr.id = $1 AND lr.company_id = $2 AND lr.status = 'pending'"#,
        request_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(leave)
}

/// A cancelled request (with its type name), or `None` if missing/not deletable.
pub async fn get_cancelled_with_type(
    executor: impl Executor<'_, Database = Postgres>,
    request_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<LeaveRequest>> {
    let leave = sqlx::query_as!(
        LeaveRequest,
        r#"SELECT lr.id, lr.employee_id, lr.company_id, lr.leave_type_id,
            lr.start_date, lr.end_date, lr.days, lr.reason, lr.status,
            lr.reviewed_by, lr.reviewed_at, lr.review_notes,
            lr.attachment_url, lr.attachment_name,
            lr.created_at, lr.updated_at,
            lt.name AS "leave_type_name?"
        FROM leave_requests lr
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        WHERE lr.id = $1 AND lr.company_id = $2 AND lr.status = 'cancelled'"#,
        request_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(leave)
}

/// A cancellable request (pending/approved/rejected) with its type name.
pub async fn get_cancellable_with_type(
    executor: impl Executor<'_, Database = Postgres>,
    request_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<LeaveRequest>> {
    let leave = sqlx::query_as!(
        LeaveRequest,
        r#"SELECT lr.id, lr.employee_id, lr.company_id, lr.leave_type_id,
            lr.start_date, lr.end_date, lr.days, lr.reason, lr.status,
            lr.reviewed_by, lr.reviewed_at, lr.review_notes,
            lr.attachment_url, lr.attachment_name,
            lr.created_at, lr.updated_at,
            lt.name AS "leave_type_name?"
        FROM leave_requests lr
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        WHERE lr.id = $1
          AND lr.company_id = $2
          AND lr.status IN ('pending', 'approved', 'rejected')"#,
        request_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(leave)
}

#[allow(clippy::too_many_arguments)]
pub async fn update_full(
    executor: impl Executor<'_, Database = Postgres>,
    request_id: Uuid,
    company_id: Uuid,
    employee_id: Uuid,
    leave_type_id: Uuid,
    start_date: NaiveDate,
    end_date: NaiveDate,
    days: Decimal,
    reason: Option<String>,
    attachment_url: Option<String>,
    attachment_name: Option<String>,
) -> AppResult<LeaveRequest> {
    let updated = sqlx::query_as!(
        LeaveRequest,
        r#"UPDATE leave_requests
        SET employee_id = $3,
            leave_type_id = $4,
            start_date = $5,
            end_date = $6,
            days = $7,
            reason = CASE WHEN $8::text IS NULL THEN reason ELSE NULLIF($8, '') END,
            attachment_url = CASE WHEN $9::text IS NULL THEN attachment_url ELSE NULLIF($9, '') END,
            attachment_name = CASE WHEN $10::text IS NULL THEN attachment_name ELSE NULLIF($10, '') END,
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *,
            (SELECT name FROM leave_types WHERE id = leave_type_id) AS "leave_type_name?""#,
        request_id,
        company_id,
        employee_id,
        leave_type_id,
        start_date,
        end_date,
        days,
        reason,
        attachment_url,
        attachment_name,
    )
    .fetch_one(executor)
    .await?;
    Ok(updated)
}

pub async fn delete(
    executor: impl Executor<'_, Database = Postgres>,
    request_id: Uuid,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM leave_requests WHERE id = $1 AND company_id = $2",
        request_id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn set_cancelled(
    executor: impl Executor<'_, Database = Postgres>,
    request_id: Uuid,
    company_id: Uuid,
) -> AppResult<LeaveRequest> {
    let cancelled = sqlx::query_as!(
        LeaveRequest,
        r#"UPDATE leave_requests
        SET status = 'cancelled', updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *, (SELECT name FROM leave_types WHERE id = leave_type_id) AS "leave_type_name?""#,
        request_id,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(cancelled)
}

/// Approve a pending request. `None` if it was not found or not pending.
pub async fn set_approved(
    executor: impl Executor<'_, Database = Postgres>,
    request_id: Uuid,
    company_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
) -> AppResult<Option<LeaveRequest>> {
    let lr = sqlx::query_as!(
        LeaveRequest,
        r#"UPDATE leave_requests SET
            status = 'approved', reviewed_by = $3, reviewed_at = NOW(),
            review_notes = $4, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending'
        RETURNING *, (SELECT name FROM leave_types WHERE id = leave_type_id) AS "leave_type_name?""#,
        request_id,
        company_id,
        reviewer_id,
        notes,
    )
    .fetch_optional(executor)
    .await?;
    Ok(lr)
}

/// Reject a pending request. `None` if it was not found or not pending.
pub async fn set_rejected(
    executor: impl Executor<'_, Database = Postgres>,
    request_id: Uuid,
    company_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
) -> AppResult<Option<LeaveRequest>> {
    let lr = sqlx::query_as!(
        LeaveRequest,
        r#"UPDATE leave_requests SET
            status = 'rejected', reviewed_by = $3, reviewed_at = NOW(),
            review_notes = $4, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending'
        RETURNING *, (SELECT name FROM leave_types WHERE id = leave_type_id) AS "leave_type_name?""#,
        request_id,
        company_id,
        reviewer_id,
        notes,
    )
    .fetch_optional(executor)
    .await?;
    Ok(lr)
}
