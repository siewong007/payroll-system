use chrono::{Datelike, NaiveDate};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::core::http_client;
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

/// Widest range this function will evaluate, in days.
///
/// Deliberately looser than `leave_rules::MAX_LEAVE_SPAN_DAYS`: leave policy
/// belongs in `leave_rules`, and ten years is the widest legitimate reporting
/// window. This is the floor beneath every caller, so a future endpoint cannot
/// reintroduce an unbounded walk from somewhere else.
const MAX_CALENDAR_SPAN_DAYS: i64 = 3_660;

/// Count working days between two dates (inclusive), respecting company calendar
pub async fn count_working_days_between(
    pool: &PgPool,
    company_id: Uuid,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> AppResult<i32> {
    if (end_date - start_date).num_days() > MAX_CALENDAR_SPAN_DAYS {
        return Err(AppError::BadRequest(
            "Date range is too large to evaluate".into(),
        ));
    }

    let config = get_working_days(pool, company_id).await?;

    // One query for the whole span. This used to be a `for yr in start..=end`
    // loop over `list_for_year`, so a client-supplied range acquired a fresh
    // pooled connection and ran a non-sargable seq scan once per year in it.
    let all_holidays = holiday_repo::list_for_range(pool, company_id, start_date, end_date).await?;

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
///
/// The fetch itself is `core::http_client`'s job: the URL arrives in request
/// JSON, so it is a request-forgery vector unless the scheme, the resolved
/// addresses, the redirect policy, the timeout and the body size are all
/// constrained together. Doing any of that here would put a second, drifting
/// copy of the policy next to the one the upload path already trusts.
pub async fn import_from_ics(
    pool: &PgPool,
    company_id: Uuid,
    ics_url: &str,
    created_by: Uuid,
) -> AppResult<Vec<Holiday>> {
    let ics_text = http_client::fetch_public_text(ics_url, http_client::MAX_ICS_BYTES).await?;
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

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, Utc};
    use uuid::Uuid;

    use super::count_working_days_in_month;
    use crate::models::calendar::{Holiday, WorkingDayConfig};

    fn day(day_of_week: i16, is_working_day: bool) -> WorkingDayConfig {
        WorkingDayConfig {
            id: Uuid::nil(),
            company_id: Uuid::nil(),
            day_of_week,
            is_working_day,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Sunday-indexed, matching `Weekday::num_days_from_sunday`.
    fn week(working: &[i16]) -> Vec<WorkingDayConfig> {
        (0..7).map(|dow| day(dow, working.contains(&dow))).collect()
    }

    fn holiday(year: i32, month: u32, day_of_month: u32) -> Holiday {
        Holiday {
            id: Uuid::nil(),
            company_id: Uuid::nil(),
            name: "Test Holiday".to_string(),
            date: NaiveDate::from_ymd_opt(year, month, day_of_month).expect("valid holiday date"),
            holiday_type: "public_holiday".to_string(),
            description: None,
            is_recurring: false,
            state: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: None,
            updated_by: None,
        }
    }

    #[test]
    fn empty_config_defaults_to_a_monday_to_friday_week() {
        // July 2026 starts on a Wednesday and has 23 weekdays.
        assert_eq!(count_working_days_in_month(2026, 7, &[], &[]), 23);
    }

    #[test]
    fn an_explicit_monday_to_friday_config_matches_the_default() {
        assert_eq!(
            count_working_days_in_month(2026, 7, &week(&[1, 2, 3, 4, 5]), &[]),
            count_working_days_in_month(2026, 7, &[], &[])
        );
    }

    #[test]
    fn a_six_day_week_adds_every_saturday() {
        // July 2026 has four Saturdays.
        assert_eq!(
            count_working_days_in_month(2026, 7, &week(&[1, 2, 3, 4, 5, 6]), &[]),
            27
        );
    }

    #[test]
    fn a_seven_day_week_counts_the_whole_month() {
        assert_eq!(
            count_working_days_in_month(2026, 7, &week(&[0, 1, 2, 3, 4, 5, 6]), &[]),
            31
        );
    }

    #[test]
    fn a_week_with_no_working_days_counts_nothing() {
        assert_eq!(count_working_days_in_month(2026, 7, &week(&[]), &[]), 0);
    }

    #[test]
    fn holidays_falling_on_working_days_are_deducted() {
        // 2026-07-01 is a Wednesday.
        assert_eq!(
            count_working_days_in_month(2026, 7, &[], &[holiday(2026, 7, 1)]),
            22
        );
    }

    #[test]
    fn a_holiday_on_a_non_working_day_changes_nothing() {
        // 2026-07-04 is a Saturday, already excluded by a Mon–Fri week.
        assert_eq!(
            count_working_days_in_month(2026, 7, &[], &[holiday(2026, 7, 4)]),
            23
        );
    }

    #[test]
    fn a_holiday_is_never_deducted_twice() {
        let duplicated = vec![holiday(2026, 7, 1), holiday(2026, 7, 1)];
        assert_eq!(count_working_days_in_month(2026, 7, &[], &duplicated), 22);
    }

    #[test]
    fn holidays_outside_the_month_are_ignored() {
        let others = vec![holiday(2026, 6, 15), holiday(2026, 8, 3)];
        assert_eq!(count_working_days_in_month(2026, 7, &[], &others), 23);
    }

    #[test]
    fn february_respects_leap_years() {
        // 2028 is a leap year: 29 February falls on a Tuesday.
        assert_eq!(count_working_days_in_month(2028, 2, &[], &[]), 21);
        assert_eq!(count_working_days_in_month(2026, 2, &[], &[]), 20);
    }

    #[test]
    fn december_rolls_over_the_year_to_find_its_last_day() {
        // The month-end calculation crosses into January of the next year; a
        // wrong roll-over would silently truncate December.
        assert_eq!(
            count_working_days_in_month(2026, 12, &week(&[0, 1, 2, 3, 4, 5, 6]), &[]),
            31
        );
    }

    #[test]
    fn every_month_of_a_full_year_is_counted_under_an_all_days_week() {
        let all_days = week(&[0, 1, 2, 3, 4, 5, 6]);
        let expected = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

        for (index, days) in expected.into_iter().enumerate() {
            let month = index as u32 + 1;
            assert_eq!(
                count_working_days_in_month(2026, month, &all_days, &[]),
                days,
                "month {month} should have {days} days"
            );
        }
    }

    #[test]
    fn an_invalid_month_yields_zero_rather_than_panicking() {
        assert_eq!(count_working_days_in_month(2026, 13, &[], &[]), 0);
        assert_eq!(count_working_days_in_month(2026, 0, &[], &[]), 0);
    }

    #[test]
    fn out_of_range_day_of_week_entries_are_ignored() {
        // A malformed config row must not panic on an array index.
        let config = vec![day(1, true), day(9, true), day(-1, true)];
        assert_eq!(count_working_days_in_month(2026, 7, &config, &[]), 4);
    }
}
