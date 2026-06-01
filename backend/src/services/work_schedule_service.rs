use chrono::NaiveTime;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::work_schedule::{
    CreateWorkScheduleRequest, UpdateWorkScheduleRequest, WorkSchedule,
};
use crate::repositories::company_work_schedules;
use crate::services::audit_service::{self, AuditRequestMeta};

fn parse_time(s: &str) -> AppResult<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M:%S"))
        .map_err(|_| AppError::BadRequest(format!("Invalid time format '{}'. Use HH:MM", s)))
}

/// Get the default work schedule for a company
pub async fn get_default_schedule(
    pool: &PgPool,
    company_id: Uuid,
) -> AppResult<Option<WorkSchedule>> {
    company_work_schedules::get_default(pool, company_id).await
}

/// Get all work schedules for a company
pub async fn list_schedules(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<WorkSchedule>> {
    company_work_schedules::list_for_company(pool, company_id).await
}

/// Create or replace the default work schedule for a company
pub async fn upsert_default_schedule(
    pool: &PgPool,
    company_id: Uuid,
    req: &CreateWorkScheduleRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<WorkSchedule> {
    let start = parse_time(&req.start_time)?;
    let end = parse_time(&req.end_time)?;
    let name = req.name.as_deref().unwrap_or("Default");
    let grace = req.grace_minutes.unwrap_or(15);
    let half_day = req.half_day_hours.unwrap_or(4.0);
    let tz = req.timezone.as_deref().unwrap_or("Asia/Kuala_Lumpur");

    if !(0..=120).contains(&grace) {
        return Err(AppError::BadRequest(
            "Grace minutes must be between 0 and 120".into(),
        ));
    }

    let existing = get_default_schedule(pool, company_id).await?;

    let schedule = company_work_schedules::upsert_default(
        pool, company_id, name, start, end, grace, half_day, tz,
    )
    .await?;

    let action = if existing.is_some() {
        "update"
    } else {
        "create"
    };
    let description = if existing.is_some() {
        "Default work schedule updated"
    } else {
        "Default work schedule created"
    };

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        action,
        "work_schedule",
        Some(schedule.id),
        existing.and_then(|value| serde_json::to_value(value).ok()),
        Some(serde_json::to_value(&schedule).unwrap_or_default()),
        Some(description),
        audit_meta,
    )
    .await;

    Ok(schedule)
}

/// Update an existing work schedule
pub async fn update_schedule(
    pool: &PgPool,
    company_id: Uuid,
    schedule_id: Uuid,
    req: &UpdateWorkScheduleRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<WorkSchedule> {
    // Verify it belongs to this company
    let existing = company_work_schedules::get_by_id(pool, schedule_id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Work schedule not found".into()))?;

    let start = match &req.start_time {
        Some(s) => parse_time(s)?,
        None => existing.start_time,
    };
    let end = match &req.end_time {
        Some(s) => parse_time(s)?,
        None => existing.end_time,
    };
    let name = req.name.as_deref().unwrap_or(&existing.name);
    let grace = req.grace_minutes.unwrap_or(existing.grace_minutes);
    let half_day = req.half_day_hours.unwrap_or_else(|| {
        use rust_decimal::prelude::ToPrimitive;
        existing.half_day_hours.to_f64().unwrap_or(4.0)
    });
    let tz = req.timezone.as_deref().unwrap_or(&existing.timezone);

    if !(0..=120).contains(&grace) {
        return Err(AppError::BadRequest(
            "Grace minutes must be between 0 and 120".into(),
        ));
    }

    let schedule = company_work_schedules::update(
        pool,
        schedule_id,
        company_id,
        name,
        start,
        end,
        grace,
        half_day,
        tz,
    )
    .await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "update",
        "work_schedule",
        Some(schedule.id),
        Some(serde_json::to_value(&existing).unwrap_or_default()),
        Some(serde_json::to_value(&schedule).unwrap_or_default()),
        Some("Work schedule updated"),
        audit_meta,
    )
    .await;

    Ok(schedule)
}
