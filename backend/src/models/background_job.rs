use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// A unit of work running outside the request that submitted it.
///
/// `payload` is deliberately not serialized back to clients — it is the
/// request body verbatim, which for `employee_import` carries a session id
/// and for future types could carry secrets. `result` is written by the
/// executor and is safe to return: `payroll_run` stores `{run_id}` and
/// `employee_import` stores the confirm response.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct BackgroundJob {
    pub id: Uuid,
    pub company_id: Uuid,
    pub job_type: String,
    pub status: String,
    #[serde(skip_serializing)]
    pub payload: serde_json::Value,
    pub progress_done: i32,
    pub progress_total: i32,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

pub mod job_type {
    pub const PAYROLL_RUN: &str = "payroll_run";
    pub const EMPLOYEE_IMPORT: &str = "employee_import";
}
