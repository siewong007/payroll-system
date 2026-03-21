use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Holiday {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub date: NaiveDate,
    pub holiday_type: String,
    pub description: Option<String>,
    pub is_recurring: bool,
    pub state: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateHolidayRequest {
    pub name: String,
    pub date: NaiveDate,
    pub holiday_type: Option<String>,
    pub description: Option<String>,
    pub is_recurring: Option<bool>,
    pub state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateHolidayRequest {
    pub name: Option<String>,
    pub date: Option<NaiveDate>,
    pub holiday_type: Option<String>,
    pub description: Option<String>,
    pub is_recurring: Option<bool>,
    pub state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkingDayConfig {
    pub id: Uuid,
    pub company_id: Uuid,
    pub day_of_week: i16,
    pub is_working_day: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkingDaysRequest {
    pub days: Vec<WorkingDayEntry>,
}

#[derive(Debug, Deserialize)]
pub struct WorkingDayEntry {
    pub day_of_week: i16,
    pub is_working_day: bool,
}

/// Calendar summary for a given month
#[derive(Debug, Serialize)]
pub struct MonthCalendar {
    pub year: i32,
    pub month: u32,
    pub working_days: i32,
    pub holidays: Vec<Holiday>,
    pub working_day_config: Vec<WorkingDayConfig>,
}
