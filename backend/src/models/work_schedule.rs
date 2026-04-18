use chrono::{DateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkSchedule {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub grace_minutes: i32,
    pub half_day_hours: rust_decimal::Decimal,
    pub timezone: String,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWorkScheduleRequest {
    pub name: Option<String>,
    /// HH:MM format, e.g. "09:00"
    pub start_time: String,
    /// HH:MM format, e.g. "18:00"
    pub end_time: String,
    /// Minutes of grace before marking late (default 15)
    pub grace_minutes: Option<i32>,
    /// Hours threshold for half-day (default 4.0)
    pub half_day_hours: Option<f64>,
    /// IANA timezone, e.g. "Asia/Kuala_Lumpur"
    pub timezone: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkScheduleRequest {
    pub name: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub grace_minutes: Option<i32>,
    pub half_day_hours: Option<f64>,
    pub timezone: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmployeeWorkSchedule {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub company_id: Uuid,
    pub day_of_week: i16,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub grace_minutes: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateEmployeeWorkScheduleRequest {
    pub day_of_week: i16,
    pub start_time: String,
    pub end_time: String,
    pub grace_minutes: Option<i32>,
}
