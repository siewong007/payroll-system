use chrono::{Datelike, NaiveDate};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::calendar::{Holiday, MonthCalendar, WorkingDayConfig};
use crate::repositories::{holidays as holiday_repo, working_day_config as working_day_repo};

/// Get all holidays for a company in a given year
pub async fn get_holidays(pool: &PgPool, company_id: Uuid, year: i32) -> AppResult<Vec<Holiday>> {
    holiday_repo::list_for_year(pool, company_id, year).await
}

/// Get a single holiday
pub async fn get_holiday(pool: &PgPool, company_id: Uuid, id: Uuid) -> AppResult<Holiday> {
    holiday_repo::get_by_id(pool, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Holiday not found".into()))
}

/// Create a holiday
#[allow(clippy::too_many_arguments)]
pub async fn create_holiday(
    pool: &PgPool,
    company_id: Uuid,
    name: &str,
    date: NaiveDate,
    holiday_type: &str,
    description: Option<&str>,
    is_recurring: bool,
    state: Option<&str>,
    created_by: Uuid,
) -> AppResult<Holiday> {
    holiday_repo::insert(
        pool,
        company_id,
        name,
        date,
        holiday_type,
        description,
        is_recurring,
        state,
        created_by,
    )
    .await
}

/// Update a holiday
#[allow(clippy::too_many_arguments)]
pub async fn update_holiday(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
    name: Option<&str>,
    date: Option<NaiveDate>,
    holiday_type: Option<&str>,
    description: Option<&str>,
    is_recurring: Option<bool>,
    state: Option<&str>,
    updated_by: Uuid,
) -> AppResult<Holiday> {
    holiday_repo::update(
        pool,
        id,
        company_id,
        name,
        date,
        holiday_type,
        description,
        is_recurring,
        state,
        updated_by,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Holiday not found".into()))
}

/// Delete a holiday
pub async fn delete_holiday(pool: &PgPool, company_id: Uuid, id: Uuid) -> AppResult<()> {
    let rows = holiday_repo::delete(pool, id, company_id).await?;
    if rows == 0 {
        return Err(AppError::NotFound("Holiday not found".into()));
    }
    Ok(())
}

/// Get working day configuration for a company
pub async fn get_working_days(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<WorkingDayConfig>> {
    working_day_repo::list_for_company(pool, company_id).await
}

/// Update working day configuration
pub async fn update_working_days(
    pool: &PgPool,
    company_id: Uuid,
    days: &[(i16, bool)],
) -> AppResult<Vec<WorkingDayConfig>> {
    for &(day, is_working) in days {
        if !(0..=6).contains(&day) {
            return Err(AppError::BadRequest(format!(
                "Invalid day_of_week: {}. Must be 0-6.",
                day
            )));
        }
        working_day_repo::upsert(pool, company_id, day, is_working).await?;
    }
    get_working_days(pool, company_id).await
}

/// Get calendar summary for a month (working days count, holidays, config)
pub async fn get_month_calendar(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
    month: u32,
) -> AppResult<MonthCalendar> {
    let working_day_config = get_working_days(pool, company_id).await?;
    let holidays = get_holidays(pool, company_id, year).await?;

    // Filter holidays to this month
    let month_holidays: Vec<Holiday> = holidays
        .into_iter()
        .filter(|h| h.date.month() == month)
        .collect();

    let working_days =
        count_working_days_in_month(year, month, &working_day_config, &month_holidays);

    Ok(MonthCalendar {
        year,
        month,
        working_days,
        holidays: month_holidays,
        working_day_config,
    })
}

/// Count working days in a month, excluding holidays
fn count_working_days_in_month(
    year: i32,
    month: u32,
    config: &[WorkingDayConfig],
    holidays: &[Holiday],
) -> i32 {
    let first_day = match NaiveDate::from_ymd_opt(year, month, 1) {
        Some(d) => d,
        None => return 0,
    };
    let last_day = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .and_then(|d| d.pred_opt())
    .unwrap_or(first_day);

    let holiday_dates: std::collections::HashSet<NaiveDate> =
        holidays.iter().map(|h| h.date).collect();

    // Build working day lookup (default: Mon-Fri)
    let mut working_days_of_week = [false; 7];
    if config.is_empty() {
        working_days_of_week[1..=5].fill(true);
    } else {
        for c in config {
            if (0..=6).contains(&c.day_of_week) {
                working_days_of_week[c.day_of_week as usize] = c.is_working_day;
            }
        }
    }

    let mut count = 0;
    let mut d = first_day;
    while d <= last_day {
        let dow = d.weekday().num_days_from_sunday() as usize;
        if working_days_of_week[dow] && !holiday_dates.contains(&d) {
            count += 1;
        }
        match d.succ_opt() {
            Some(next) => d = next,
            None => break,
        }
    }

    count
}

/// Count working days between two dates (inclusive), respecting company calendar
pub async fn count_working_days_between(
    pool: &PgPool,
    company_id: Uuid,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> AppResult<i32> {
    let config = get_working_days(pool, company_id).await?;

    // Get holidays for the range (may span years)
    let start_year = start_date.year();
    let end_year = end_date.year();
    let mut all_holidays = Vec::new();
    for yr in start_year..=end_year {
        let mut h = get_holidays(pool, company_id, yr).await?;
        all_holidays.append(&mut h);
    }

    let holiday_dates: std::collections::HashSet<NaiveDate> =
        all_holidays.iter().map(|h| h.date).collect();

    let mut working_days_of_week = [false; 7];
    if config.is_empty() {
        working_days_of_week[1..=5].fill(true);
    } else {
        for c in &config {
            if (0..=6).contains(&c.day_of_week) {
                working_days_of_week[c.day_of_week as usize] = c.is_working_day;
            }
        }
    }

    let mut count = 0;
    let mut d = start_date;
    while d <= end_date {
        let dow = d.weekday().num_days_from_sunday() as usize;
        if working_days_of_week[dow] && !holiday_dates.contains(&d) {
            count += 1;
        }
        match d.succ_opt() {
            Some(next) => d = next,
            None => break,
        }
    }

    Ok(count)
}

/// Import holidays from a Google Calendar ICS URL
pub async fn import_from_ics(
    pool: &PgPool,
    company_id: Uuid,
    ics_url: &str,
    created_by: Uuid,
) -> AppResult<Vec<Holiday>> {
    let client = reqwest::Client::new();
    let response = client
        .get(ics_url)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to fetch ICS URL: {}", e)))?;

    let ics_text = response
        .text()
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to read ICS response: {}", e)))?;

    import_from_ics_text(pool, company_id, &ics_text, created_by).await
}

/// Import holidays from raw ICS text content
pub async fn import_from_ics_text(
    pool: &PgPool,
    company_id: Uuid,
    ics_text: &str,
    created_by: Uuid,
) -> AppResult<Vec<Holiday>> {
    let mut holidays = Vec::new();
    let mut in_event = false;
    let mut name = String::new();
    let mut date: Option<NaiveDate> = None;
    let mut description: Option<String> = None;

    for line in ics_text.lines() {
        let line = line.trim();
        if line == "BEGIN:VEVENT" {
            in_event = true;
            name = String::new();
            date = None;
            description = None;
        } else if line == "END:VEVENT" {
            if in_event
                && let (n, Some(d)) = (&name, date)
                && !n.is_empty()
            {
                let exists = holiday_repo::count_matching(pool, company_id, d, n.as_str()).await?;

                if exists == 0 {
                    let h = create_holiday(
                        pool,
                        company_id,
                        n,
                        d,
                        "public_holiday",
                        description.as_deref(),
                        false,
                        None,
                        created_by,
                    )
                    .await?;
                    holidays.push(h);
                }
            }
            in_event = false;
        } else if in_event {
            if let Some(val) = line.strip_prefix("SUMMARY:") {
                name = val.to_string();
            } else if let Some(val) = line.strip_prefix("DTSTART;VALUE=DATE:") {
                date = NaiveDate::parse_from_str(val, "%Y%m%d").ok();
            } else if let Some(val) = line.strip_prefix("DTSTART:") {
                let date_str = if val.len() >= 8 { &val[..8] } else { val };
                date = NaiveDate::parse_from_str(date_str, "%Y%m%d").ok();
            } else if let Some(val) = line.strip_prefix("DESCRIPTION:") {
                description = Some(val.replace("\\n", "\n").replace("\\,", ","));
            }
        }
    }

    Ok(holidays)
}

/// Get total working days in a month for a company
pub async fn get_working_days_in_month(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
    month: u32,
) -> AppResult<i32> {
    let first_day = NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| AppError::BadRequest("Invalid month".into()))?;
    let last_day = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .and_then(|d| d.pred_opt())
    .ok_or_else(|| AppError::BadRequest("Invalid month".into()))?;

    count_working_days_between(pool, company_id, first_day, last_day).await
}
