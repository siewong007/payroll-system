use chrono::NaiveTime;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::work_schedule::{
    CreateWorkScheduleRequest, UpdateWorkScheduleRequest, WorkSchedule,
};
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
    let schedule = sqlx::query_as::<_, WorkSchedule>(
        "SELECT * FROM company_work_schedules WHERE company_id = $1 AND is_default = TRUE",
    )
    .bind(company_id)
    .fetch_optional(pool)
    .await?;
    Ok(schedule)
}

/// Get all work schedules for a company
pub async fn list_schedules(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<WorkSchedule>> {
    let schedules = sqlx::query_as::<_, WorkSchedule>(
        "SELECT * FROM company_work_schedules WHERE company_id = $1 ORDER BY is_default DESC, name",
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;
    Ok(schedules)
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

    let schedule = sqlx::query_as::<_, WorkSchedule>(
        r#"INSERT INTO company_work_schedules
           (company_id, name, start_time, end_time, grace_minutes, half_day_hours, timezone, is_default)
           VALUES ($1, $2, $3, $4, $5, $6, $7, TRUE)
           ON CONFLICT (company_id) WHERE is_default = TRUE
           DO UPDATE SET
               name = EXCLUDED.name,
               start_time = EXCLUDED.start_time,
               end_time = EXCLUDED.end_time,
               grace_minutes = EXCLUDED.grace_minutes,
               half_day_hours = EXCLUDED.half_day_hours,
               timezone = EXCLUDED.timezone,
               updated_at = NOW()
           RETURNING *"#,
    )
    .bind(company_id)
    .bind(name)
    .bind(start)
    .bind(end)
    .bind(grace)
    .bind(half_day)
    .bind(tz)
    .fetch_one(pool)
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
    let existing = sqlx::query_as::<_, WorkSchedule>(
        "SELECT * FROM company_work_schedules WHERE id = $1 AND company_id = $2",
    )
    .bind(schedule_id)
    .bind(company_id)
    .fetch_optional(pool)
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

    let schedule = sqlx::query_as::<_, WorkSchedule>(
        r#"UPDATE company_work_schedules
           SET name = $3, start_time = $4, end_time = $5,
               grace_minutes = $6, half_day_hours = $7, timezone = $8,
               updated_at = NOW()
           WHERE id = $1 AND company_id = $2
           RETURNING *"#,
    )
    .bind(schedule_id)
    .bind(company_id)
    .bind(name)
    .bind(start)
    .bind(end)
    .bind(grace)
    .bind(half_day)
    .bind(tz)
    .fetch_one(pool)
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
