//! Data access for the `notifications` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::notification::Notification;

/// A user's notifications for one active company, newest first; `unread_only` filters to unread.
pub async fn list(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    company_id: Uuid,
    unread_only: bool,
    limit: i64,
) -> AppResult<Vec<Notification>> {
    let notifications = sqlx::query_as!(
        Notification,
        r#"SELECT * FROM notifications
        WHERE user_id = $1 AND company_id = $2 AND ($3 = FALSE OR is_read = FALSE)
        ORDER BY created_at DESC
        LIMIT $4"#,
        user_id,
        company_id,
        unread_only,
        limit,
    )
    .fetch_all(executor)
    .await?;
    Ok(notifications)
}

pub async fn count_summary(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    company_id: Uuid,
) -> AppResult<(i64, i64)> {
    let row = sqlx::query!(
        r#"SELECT
               COUNT(*) FILTER (WHERE is_read = FALSE) AS "unread!",
               COUNT(*) AS "total!"
           FROM notifications
           WHERE user_id = $1 AND company_id = $2"#,
        user_id,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok((row.unread, row.total))
}

pub async fn count_unread(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    company_id: Uuid,
) -> AppResult<i64> {
    let unread = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM notifications WHERE user_id = $1 AND company_id = $2 AND is_read = FALSE"#,
        user_id,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(unread)
}

pub async fn count_total(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    company_id: Uuid,
) -> AppResult<i64> {
    let total = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM notifications WHERE user_id = $1 AND company_id = $2"#,
        user_id,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(total)
}

pub async fn mark_read(
    executor: impl Executor<'_, Database = Postgres>,
    notification_id: Uuid,
    user_id: Uuid,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE notifications SET is_read = TRUE, read_at = NOW() WHERE id = $1 AND user_id = $2 AND company_id = $3",
        notification_id,
        user_id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn mark_all_read(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE notifications SET is_read = TRUE, read_at = NOW() WHERE user_id = $1 AND company_id = $2 AND is_read = FALSE",
        user_id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    company_id: Uuid,
    notification_type: &str,
    title: &str,
    message: &str,
    entity_type: Option<&str>,
    entity_id: Option<Uuid>,
) -> AppResult<Notification> {
    let notification = sqlx::query_as!(
        Notification,
        r#"INSERT INTO notifications (user_id, company_id, notification_type, title, message, entity_type, entity_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *"#,
        user_id,
        company_id,
        notification_type,
        title,
        message,
        entity_type,
        entity_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(notification)
}

/// Fan out one notification to every active approver/HR user in the company.
pub async fn insert_for_admins(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    notification_type: &str,
    title: &str,
    message: &str,
    entity_type: Option<&str>,
    entity_id: Option<Uuid>,
) -> AppResult<()> {
    let _ = insert_for_admins_returning_ids(
        executor,
        company_id,
        notification_type,
        title,
        message,
        entity_type,
        entity_id,
    )
    .await?;
    Ok(())
}

/// Fan out one notification to every active approver/HR user in the company and return inserted IDs.
pub async fn insert_for_admins_returning_ids(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    notification_type: &str,
    title: &str,
    message: &str,
    entity_type: Option<&str>,
    entity_id: Option<Uuid>,
) -> AppResult<Vec<Uuid>> {
    let ids = sqlx::query_scalar!(
        r#"INSERT INTO notifications (user_id, company_id, notification_type, title, message, entity_type, entity_id)
        SELECT DISTINCT u.id, $1::uuid, $2::varchar, $3::varchar, $4::text, $5::varchar, $6::uuid
        FROM users u
        JOIN user_companies uc ON uc.user_id = u.id AND uc.company_id = $1
        WHERE u.is_active = TRUE
            AND u.deleted_at IS NULL
            AND (
                u.roles && ARRAY['super_admin', 'admin', 'payroll_admin', 'hr_manager', 'manager']::VARCHAR(50)[]
                OR EXISTS (
                    SELECT 1
                    FROM user_group_members ugm
                    JOIN user_groups ug ON ug.id = ugm.group_id
                    JOIN user_group_permissions ugp ON ugp.group_id = ug.id
                    WHERE ugm.user_id = u.id
                      AND ug.company_id = $1
                      AND ug.is_active = TRUE
                      AND ugp.permission = 'manage_approvals'
                )
            )
        RETURNING id"#,
        company_id,
        notification_type,
        title,
        message,
        entity_type,
        entity_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(ids)
}

/// Fan out one payslip-availability notification to every active user linked to an employee in a payroll run.
pub async fn insert_for_payroll_run(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    payroll_run_id: Uuid,
    title: &str,
    message: &str,
) -> AppResult<Vec<Uuid>> {
    let ids = sqlx::query_scalar!(
        r#"INSERT INTO notifications (user_id, company_id, notification_type, title, message, entity_type, entity_id)
        SELECT DISTINCT u.id, $1::uuid, 'payroll_processed'::varchar, $3::varchar, $4::text, 'payroll_run'::varchar, $2::uuid
        FROM payroll_items pi
        JOIN users u ON u.employee_id = pi.employee_id
        JOIN user_companies uc ON uc.user_id = u.id AND uc.company_id = $1
        WHERE pi.payroll_run_id = $2
          AND u.is_active = TRUE
          AND u.deleted_at IS NULL
        RETURNING id"#,
        company_id,
        payroll_run_id,
        title,
        message,
    )
    .fetch_all(executor)
    .await?;
    Ok(ids)
}

/// Fan out one notification to active users linked to the given employees in a company.
#[allow(clippy::too_many_arguments)]
pub async fn insert_for_employees(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_ids: &[Uuid],
    notification_type: &str,
    title: &str,
    message: &str,
    entity_type: Option<&str>,
    entity_id: Option<Uuid>,
) -> AppResult<Vec<Uuid>> {
    if employee_ids.is_empty() {
        return Ok(Vec::new());
    }
    let ids = sqlx::query_scalar!(
        r#"INSERT INTO notifications (user_id, company_id, notification_type, title, message, entity_type, entity_id)
        SELECT DISTINCT u.id, $1::uuid, $3::varchar, $4::varchar, $5::text, $6::varchar, $7::uuid
        FROM users u
        JOIN user_companies uc ON uc.user_id = u.id AND uc.company_id = $1
        WHERE u.employee_id = ANY($2)
          AND u.is_active = TRUE
          AND u.deleted_at IS NULL
        RETURNING id"#,
        company_id,
        employee_ids,
        notification_type,
        title,
        message,
        entity_type,
        entity_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(ids)
}

/// Delete handled notifications past the retention window and stale unread
/// ones past a longer one (plan item 26). A notification nobody acted on for
/// a year is not actionable any more either.
pub async fn purge_stale(
    executor: impl Executor<'_, Database = Postgres>,
    read_retention_days: i32,
    unread_retention_days: i32,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"DELETE FROM notifications
           WHERE (read_at IS NOT NULL AND created_at < NOW() - ($1 || ' days')::interval)
              OR created_at < NOW() - ($2 || ' days')::interval"#,
    )
    .bind(read_retention_days.to_string())
    .bind(unread_retention_days.to_string())
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}
