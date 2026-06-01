//! Data access for the `audit_logs` table.
//!
//! Partial: seeded with the attendance-correction insert. The audit_service
//! migration will accrete the general logging queries here.

use uuid::Uuid;

use crate::core::error::AppResult;
use sqlx::{Executor, Postgres};

/// Record an attendance-record correction in the audit trail.
pub async fn insert_attendance_correction(
    executor: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    record_id: Uuid,
    old_values: &serde_json::Value,
    new_values: &serde_json::Value,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO audit_logs (user_id, action, entity_type, entity_id, old_values, new_values, description)
           VALUES ($1, 'update', 'attendance_record', $2, $3, $4, 'Attendance record corrected')"#,
        user_id,
        record_id,
        old_values,
        new_values,
    )
    .execute(executor)
    .await?;
    Ok(())
}
