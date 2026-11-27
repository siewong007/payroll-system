//! Data access for `employee_attendance_devices` (WebCrypto P-256 bound devices).

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::attendance::EmployeeAttendanceDevice;

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Uuid,
    user_id: Uuid,
    public_key_raw: &str,
    public_key_fingerprint: &str,
    device_label: &str,
    user_agent: Option<&str>,
    cookie_device_id: Option<Uuid>,
    status: &str,
    approved_by: Option<Uuid>,
) -> AppResult<EmployeeAttendanceDevice> {
    let is_approved = status == "approved";
    let row = sqlx::query_as!(
        EmployeeAttendanceDevice,
        r#"INSERT INTO employee_attendance_devices
           (company_id, employee_id, user_id, public_key_raw, public_key_fingerprint,
            device_label, user_agent, cookie_device_id, status, approved_by, approved_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                   CASE WHEN $11 THEN NOW() ELSE NULL END)
           RETURNING id, company_id, employee_id, user_id, public_key_raw, public_key_fingerprint,
                     device_label, user_agent, cookie_device_id, status, approved_by, approved_at,
                     revoked_by, revoked_at, revoke_reason, last_used_at, created_at"#,
        company_id,
        employee_id,
        user_id,
        public_key_raw,
        public_key_fingerprint,
        device_label,
        user_agent,
        cookie_device_id,
        status,
        approved_by,
        is_approved,
    )
    .fetch_one(executor)
    .await?;
    Ok(row)
}

pub async fn list_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Uuid,
) -> AppResult<Vec<EmployeeAttendanceDevice>> {
    let rows = sqlx::query_as!(
        EmployeeAttendanceDevice,
        r#"SELECT id, company_id, employee_id, user_id, public_key_raw, public_key_fingerprint,
                  device_label, user_agent, cookie_device_id, status, approved_by, approved_at,
                  revoked_by, revoked_at, revoke_reason, last_used_at, created_at
           FROM employee_attendance_devices
           WHERE company_id = $1 AND employee_id = $2
           ORDER BY created_at DESC"#,
        company_id,
        employee_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn get_by_id_in_company(
    executor: impl Executor<'_, Database = Postgres>,
    device_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<EmployeeAttendanceDevice>> {
    let row = sqlx::query_as!(
        EmployeeAttendanceDevice,
        r#"SELECT id, company_id, employee_id, user_id, public_key_raw, public_key_fingerprint,
                  device_label, user_agent, cookie_device_id, status, approved_by, approved_at,
                  revoked_by, revoked_at, revoke_reason, last_used_at, created_at
           FROM employee_attendance_devices
           WHERE id = $1 AND company_id = $2"#,
        device_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

pub async fn get_by_id_in_company_for_update(
    executor: impl Executor<'_, Database = Postgres>,
    device_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<EmployeeAttendanceDevice>> {
    let row = sqlx::query_as!(
        EmployeeAttendanceDevice,
        r#"SELECT id, company_id, employee_id, user_id, public_key_raw, public_key_fingerprint,
                  device_label, user_agent, cookie_device_id, status, approved_by, approved_at,
                  revoked_by, revoked_at, revoke_reason, last_used_at, created_at
           FROM employee_attendance_devices
           WHERE id = $1 AND company_id = $2
           FOR UPDATE"#,
        device_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

pub async fn find_for_employee_by_id(
    executor: impl Executor<'_, Database = Postgres>,
    device_id: Uuid,
    company_id: Uuid,
    employee_id: Uuid,
) -> AppResult<Option<EmployeeAttendanceDevice>> {
    let row = sqlx::query_as!(
        EmployeeAttendanceDevice,
        r#"SELECT id, company_id, employee_id, user_id, public_key_raw, public_key_fingerprint,
                  device_label, user_agent, cookie_device_id, status, approved_by, approved_at,
                  revoked_by, revoked_at, revoke_reason, last_used_at, created_at
           FROM employee_attendance_devices
           WHERE id = $1 AND company_id = $2 AND employee_id = $3"#,
        device_id,
        company_id,
        employee_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

pub async fn find_approved_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Uuid,
) -> AppResult<Option<EmployeeAttendanceDevice>> {
    let row = sqlx::query_as!(
        EmployeeAttendanceDevice,
        r#"SELECT id, company_id, employee_id, user_id, public_key_raw, public_key_fingerprint,
                  device_label, user_agent, cookie_device_id, status, approved_by, approved_at,
                  revoked_by, revoked_at, revoke_reason, last_used_at, created_at
           FROM employee_attendance_devices
           WHERE company_id = $1 AND employee_id = $2 AND status = 'approved'
           LIMIT 1"#,
        company_id,
        employee_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

pub async fn has_any_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Uuid,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(
               SELECT 1 FROM employee_attendance_devices
               WHERE company_id = $1 AND employee_id = $2
           ) AS "exists!""#,
        company_id,
        employee_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

pub async fn find_non_revoked_by_fingerprint(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Uuid,
    fingerprint: &str,
) -> AppResult<Option<EmployeeAttendanceDevice>> {
    let row = sqlx::query_as!(
        EmployeeAttendanceDevice,
        r#"SELECT id, company_id, employee_id, user_id, public_key_raw, public_key_fingerprint,
                  device_label, user_agent, cookie_device_id, status, approved_by, approved_at,
                  revoked_by, revoked_at, revoke_reason, last_used_at, created_at
           FROM employee_attendance_devices
           WHERE company_id = $1 AND employee_id = $2
             AND public_key_fingerprint = $3
             AND status <> 'revoked'
           ORDER BY created_at DESC
           LIMIT 1"#,
        company_id,
        employee_id,
        fingerprint,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

pub async fn exists_approved_fingerprint_for_other_employee(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    exclude_employee_id: Uuid,
    fingerprint: &str,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(
               SELECT 1 FROM employee_attendance_devices
               WHERE company_id = $1
                 AND employee_id <> $2
                 AND public_key_fingerprint = $3
                 AND status = 'approved'
           ) AS "exists!""#,
        company_id,
        exclude_employee_id,
        fingerprint,
    )
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// Revoke any currently-approved device for `employee_id` before approving a replacement.
pub async fn revoke_approved_for_employee(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Uuid,
    revoked_by: Uuid,
    reason: &str,
    exclude_device_id: Uuid,
) -> AppResult<u64> {
    let res = sqlx::query!(
        r#"UPDATE employee_attendance_devices
           SET status = 'revoked',
               revoked_by = $4,
               revoked_at = NOW(),
               revoke_reason = $5
           WHERE company_id = $1
             AND employee_id = $2
             AND id <> $3
             AND status = 'approved'"#,
        company_id,
        employee_id,
        exclude_device_id,
        revoked_by,
        reason,
    )
    .execute(executor)
    .await?;
    Ok(res.rows_affected())
}

pub async fn mark_approved(
    executor: impl Executor<'_, Database = Postgres>,
    device_id: Uuid,
    company_id: Uuid,
    approved_by: Uuid,
) -> AppResult<Option<EmployeeAttendanceDevice>> {
    let row = sqlx::query_as!(
        EmployeeAttendanceDevice,
        r#"UPDATE employee_attendance_devices
           SET status = 'approved',
               approved_by = $3,
               approved_at = NOW(),
               revoked_by = NULL,
               revoked_at = NULL,
               revoke_reason = NULL
           WHERE id = $1 AND company_id = $2 AND status <> 'revoked'
           RETURNING id, company_id, employee_id, user_id, public_key_raw, public_key_fingerprint,
                     device_label, user_agent, cookie_device_id, status, approved_by, approved_at,
                     revoked_by, revoked_at, revoke_reason, last_used_at, created_at"#,
        device_id,
        company_id,
        approved_by,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

pub async fn revoke(
    executor: impl Executor<'_, Database = Postgres>,
    device_id: Uuid,
    company_id: Uuid,
    revoked_by: Uuid,
    reason: Option<&str>,
) -> AppResult<Option<EmployeeAttendanceDevice>> {
    let row = sqlx::query_as!(
        EmployeeAttendanceDevice,
        r#"UPDATE employee_attendance_devices
           SET status = 'revoked',
               revoked_by = $3,
               revoked_at = NOW(),
               revoke_reason = $4
           WHERE id = $1 AND company_id = $2 AND status <> 'revoked'
           RETURNING id, company_id, employee_id, user_id, public_key_raw, public_key_fingerprint,
                     device_label, user_agent, cookie_device_id, status, approved_by, approved_at,
                     revoked_by, revoked_at, revoke_reason, last_used_at, created_at"#,
        device_id,
        company_id,
        revoked_by,
        reason,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

pub async fn touch_last_used(
    executor: impl Executor<'_, Database = Postgres>,
    device_id: Uuid,
    cookie_device_id: Option<Uuid>,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE employee_attendance_devices
           SET last_used_at = NOW(),
               cookie_device_id = COALESCE($2, cookie_device_id)
           WHERE id = $1"#,
        device_id,
        cookie_device_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn count_pending_for_company(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!"
           FROM employee_attendance_devices
           WHERE company_id = $1 AND status = 'pending'"#,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}
