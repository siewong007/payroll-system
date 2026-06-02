//! Read models for the approval inboxes (leave / claim / overtime): request rows
//! joined to employee identity for the admin review lists, plus the shared
//! employee+company lookup used when emailing approval notices.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct LeaveRequestWithEmployee {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub company_id: Uuid,
    pub leave_type_id: Uuid,
    pub start_date: chrono::NaiveDate,
    pub end_date: chrono::NaiveDate,
    pub days: rust_decimal::Decimal,
    pub reason: Option<String>,
    pub status: String,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub review_notes: Option<String>,
    pub attachment_url: Option<String>,
    pub attachment_name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub leave_type_name: Option<String>,
    pub employee_name: Option<String>,
    pub employee_number: Option<String>,
}

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

#[derive(Debug, sqlx::FromRow)]
pub struct EmployeeEmailInfo {
    pub full_name: String,
    pub email: String,
    pub company_name: String,
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
