use axum::{
    Json,
    extract::{Multipart, Path, Query, State},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::calendar::{
    CreateHolidayRequest, Holiday, MonthCalendar, UpdateHolidayRequest, UpdateWorkingDaysRequest,
    WorkingDayConfig,
};
use crate::services::calendar_service;

fn require_admin(auth: &AuthUser) -> AppResult<Uuid> {
    match auth.0.role.as_str() {
        "super_admin" | "admin" | "payroll_admin" | "hr_manager" | "exec" => Ok(auth
            .0
            .company_id
            .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?),
        _ => Err(AppError::Forbidden("Admin access required".into())),
    }
}

// ─── Holidays ───

#[derive(Debug, Deserialize)]
pub struct YearQuery {
    pub year: Option<i32>,
}

pub async fn list_holidays(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<YearQuery>,
) -> AppResult<Json<Vec<Holiday>>> {
    let company_id = require_admin(&auth)?;
    let year = q.year.unwrap_or_else(|| chrono::Utc::now().year());
    let holidays = calendar_service::get_holidays(&state.pool, company_id, year).await?;
    Ok(Json(holidays))
}

pub async fn get_holiday(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Holiday>> {
    let company_id = require_admin(&auth)?;
    let holiday = calendar_service::get_holiday(&state.pool, company_id, id).await?;
    Ok(Json(holiday))
}

pub async fn create_holiday(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateHolidayRequest>,
) -> AppResult<Json<Holiday>> {
    let company_id = require_admin(&auth)?;
    let holiday = calendar_service::create_holiday(
        &state.pool,
        company_id,
        &req.name,
        req.date,
        req.holiday_type.as_deref().unwrap_or("public_holiday"),
        req.description.as_deref(),
        req.is_recurring.unwrap_or(false),
        req.state.as_deref(),
        auth.0.sub,
    )
    .await?;
    Ok(Json(holiday))
}

pub async fn update_holiday(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateHolidayRequest>,
) -> AppResult<Json<Holiday>> {
    let company_id = require_admin(&auth)?;
    let holiday = calendar_service::update_holiday(
        &state.pool,
        company_id,
        id,
        req.name.as_deref(),
        req.date,
        req.holiday_type.as_deref(),
        req.description.as_deref(),
        req.is_recurring,
        req.state.as_deref(),
        auth.0.sub,
    )
    .await?;
    Ok(Json(holiday))
}

pub async fn delete_holiday(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = require_admin(&auth)?;
    calendar_service::delete_holiday(&state.pool, company_id, id).await?;
    Ok(Json(serde_json::json!({"success": true})))
}

// ─── Working Days ───

pub async fn get_working_days(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<WorkingDayConfig>>> {
    let company_id = require_admin(&auth)?;
    let config = calendar_service::get_working_days(&state.pool, company_id).await?;
    Ok(Json(config))
}

pub async fn update_working_days(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<UpdateWorkingDaysRequest>,
) -> AppResult<Json<Vec<WorkingDayConfig>>> {
    let company_id = require_admin(&auth)?;
    let days: Vec<(i16, bool)> = req
        .days
        .iter()
        .map(|d| (d.day_of_week, d.is_working_day))
        .collect();
    let config = calendar_service::update_working_days(&state.pool, company_id, &days).await?;
    Ok(Json(config))
}

// ─── Month Calendar ───

#[derive(Debug, Deserialize)]
pub struct MonthQuery {
    pub year: i32,
    pub month: u32,
}

pub async fn get_month_calendar(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<MonthQuery>,
) -> AppResult<Json<MonthCalendar>> {
    let company_id = require_admin(&auth)?;
    let cal =
        calendar_service::get_month_calendar(&state.pool, company_id, q.year, q.month).await?;
    Ok(Json(cal))
}

use chrono::Datelike;

// ─── ICS Import ───

#[derive(Debug, Deserialize)]
pub struct ImportIcsRequest {
    pub url: String,
}

pub async fn import_ics(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<ImportIcsRequest>,
) -> AppResult<Json<Vec<Holiday>>> {
    let company_id = require_admin(&auth)?;
    let holidays =
        calendar_service::import_from_ics(&state.pool, company_id, &req.url, auth.0.sub).await?;
    Ok(Json(holidays))
}

// ─── ICS File Upload ───

pub async fn import_ics_file(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<Vec<Holiday>>> {
    let company_id = require_admin(&auth)?;

    let field = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart data: {}", e)))?
        .ok_or_else(|| AppError::BadRequest("No file provided".into()))?;

    let file_name = field.file_name().map(|s| s.to_string()).unwrap_or_default();

    if !file_name.ends_with(".ics") && !file_name.ends_with(".ical") {
        return Err(AppError::BadRequest(
            "Only .ics or .ical files are accepted".into(),
        ));
    }

    let data = field
        .bytes()
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to read file: {}", e)))?;

    let ics_text = String::from_utf8(data.to_vec())
        .map_err(|_| AppError::BadRequest("File is not valid UTF-8 text".into()))?;

    let holidays =
        calendar_service::import_from_ics_text(&state.pool, company_id, &ics_text, auth.0.sub)
            .await?;
    Ok(Json(holidays))
}
