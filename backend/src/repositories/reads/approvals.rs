//! Read models for the approval inboxes (leave / claim / overtime): request rows
//! joined to employee identity for the admin review lists, plus the shared
//! employee+company lookup used when emailing approval notices.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::approval::{
    ClaimWithEmployee, EmployeeEmailInfo, LeaveRequestWithEmployee, OvertimeWithEmployee,
};

/// Up to 100 most-recent leave requests for the admin inbox, optionally filtered
/// by status.
pub async fn list_pending_leave(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    status: Option<&str>,
) -> AppResult<Vec<LeaveRequestWithEmployee>> {
    let requests = sqlx::query_as!(
        LeaveRequestWithEmployee,
        r#"SELECT lr.id, lr.employee_id, lr.company_id, lr.leave_type_id,
            lr.start_date, lr.end_date, lr.days, lr.reason, lr.status,
            lr.reviewed_by, lr.reviewed_at, lr.review_notes,
            lr.attachment_url, lr.attachment_name,
            lr.created_at, lr.updated_at,
            lt.name AS "leave_type_name?",
            e.full_name AS "employee_name?",
            e.employee_number AS "employee_number?"
        FROM leave_requests lr
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        JOIN employees e ON lr.employee_id = e.id
        WHERE lr.company_id = $1
        AND ($2::text IS NULL OR lr.status = $2)
        ORDER BY lr.created_at DESC
        LIMIT 100"#,
        company_id,
        status,
    )
    .fetch_all(executor)
    .await?;
    Ok(requests)
}

/// Employee name/email plus company name, for composing approval emails.
pub async fn employee_email_info(
    executor: impl Executor<'_, Database = Postgres>,
    employee_id: Uuid,
) -> AppResult<Option<EmployeeEmailInfo>> {
    let info = sqlx::query_as!(
        EmployeeEmailInfo,
        r#"SELECT e.full_name, e.email AS "email!", COALESCE(c.name, '') AS "company_name!"
        FROM employees e
        JOIN companies c ON e.company_id = c.id
        WHERE e.id = $1"#,
        employee_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(info)
}

/// A single overtime application joined to employee identity, or `None`.
pub async fn overtime_with_employee_by_id(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    overtime_id: Uuid,
) -> AppResult<Option<OvertimeWithEmployee>> {
    let ot = sqlx::query_as!(
        OvertimeWithEmployee,
        r#"SELECT oa.*,
            e.full_name AS "employee_name?",
            e.employee_number AS "employee_number?"
        FROM overtime_applications oa
        JOIN employees e ON oa.employee_id = e.id
        WHERE oa.id = $1 AND oa.company_id = $2"#,
        overtime_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(ot)
}

/// Up to 100 most-recent overtime applications for the admin inbox, optionally
/// filtered by status.
pub async fn list_pending_overtime(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    status: Option<&str>,
) -> AppResult<Vec<OvertimeWithEmployee>> {
    let apps = sqlx::query_as!(
        OvertimeWithEmployee,
        r#"SELECT oa.*,
            e.full_name AS "employee_name?",
            e.employee_number AS "employee_number?"
        FROM overtime_applications oa
        JOIN employees e ON oa.employee_id = e.id
        WHERE oa.company_id = $1
        AND ($2::text IS NULL OR oa.status = $2)
        ORDER BY oa.created_at DESC
        LIMIT 100"#,
        company_id,
        status,
    )
    .fetch_all(executor)
    .await?;
    Ok(apps)
}

/// A single claim joined to employee identity, or `None`.
pub async fn claim_with_employee_by_id(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    claim_id: Uuid,
) -> AppResult<Option<ClaimWithEmployee>> {
    let claim = sqlx::query_as!(
        ClaimWithEmployee,
        r#"SELECT c.*,
            e.full_name AS "employee_name?",
            e.employee_number AS "employee_number?"
        FROM claims c
        JOIN employees e ON c.employee_id = e.id
        WHERE c.id = $1 AND c.company_id = $2"#,
        claim_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(claim)
}

/// Up to 100 most-recent claims for the admin inbox, optionally filtered by status.
pub async fn list_pending_claims(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    status: Option<&str>,
) -> AppResult<Vec<ClaimWithEmployee>> {
    let claims = sqlx::query_as!(
        ClaimWithEmployee,
        r#"SELECT c.*,
            e.full_name AS "employee_name?",
            e.employee_number AS "employee_number?"
        FROM claims c
        JOIN employees e ON c.employee_id = e.id
        WHERE c.company_id = $1
        AND ($2::text IS NULL OR c.status = $2)
        ORDER BY c.created_at DESC
        LIMIT 100"#,
        company_id,
        status,
    )
    .fetch_all(executor)
    .await?;
    Ok(claims)
}
