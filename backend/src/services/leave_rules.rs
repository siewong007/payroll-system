use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::repositories::{leave_balances, leave_types};

/// Reserves `days` against an employee's balance for a new leave request.
///
/// Paid leave is bounded by the entitlement so `taken + pending` cannot exceed
/// `entitled + carried_forward` — previously both submit and approve incremented
/// unconditionally, so an employee with 2 days left could have 60 approved.
/// Unpaid leave is not drawn from an entitlement and is only required to have a
/// balance row. Either way a 0-row update is an error: silently ignoring it left
/// the request approvable but invisible to balance accounting.
///
/// Takes a connection rather than the pool so the caller can run it in the same
/// transaction as the `leave_requests` insert — the reservation can now fail, and
/// a rejected request must not leave a row behind.
pub async fn reserve_pending_days(
    conn: &mut sqlx::PgConnection,
    employee_id: Uuid,
    leave_type_id: Uuid,
    days: Decimal,
    year: i32,
) -> AppResult<()> {
    // Absent an explicit flag, treat the type as paid: bounded is the safe default.
    let is_paid = leave_types::get_is_paid(&mut *conn, leave_type_id)
        .await?
        .unwrap_or(true);

    let rows = if is_paid {
        leave_balances::add_pending_within_entitlement(
            &mut *conn,
            employee_id,
            leave_type_id,
            days,
            year,
        )
        .await?
    } else {
        leave_balances::add_pending(&mut *conn, employee_id, leave_type_id, days, year).await?
    };
    if rows > 0 {
        return Ok(());
    }

    // Distinguish "never initialized" from "would overdraw" so the message is actionable.
    match leave_balances::get_balance_for_year(&mut *conn, employee_id, leave_type_id, year).await?
    {
        None => Err(AppError::BadRequest(format!(
            "Leave balance for {year} has not been initialized for this leave type"
        ))),
        Some((entitled, taken, pending, carried)) => {
            let remaining = entitled + carried - taken - pending;
            Err(AppError::BadRequest(format!(
                "Insufficient leave balance: {remaining} day(s) remaining, {days} requested"
            )))
        }
    }
}

/// Validates a leave period against the company calendar.
///
/// `days` is client-supplied and drives the balance deduction, while the date
/// range drives absence reporting and the auto-absent cron. Left unchecked the
/// two disagree in both directions: a month-long range submitted as `days = 0.5`
/// deducts half a day of balance yet excuses the employee for the whole month,
/// and an inflated `days` over-deducts the balance. `working_days` is the
/// authoritative count for the range from `calendar_service`, using the same
/// working-day config and holiday table the cron uses, so accepting only values
/// consistent with it keeps balance and attendance in agreement.
///
/// A half-day tolerance is allowed so partial-day leave (`days = 0.5` on a
/// single working day) stays valid.
pub fn validate_period(
    start_date: NaiveDate,
    end_date: NaiveDate,
    days: Decimal,
    working_days: i32,
) -> AppResult<()> {
    if start_date > end_date {
        return Err(AppError::BadRequest(
            "Leave start date must not be after the end date".into(),
        ));
    }
    if days <= Decimal::ZERO {
        return Err(AppError::BadRequest(
            "Leave days must be greater than zero".into(),
        ));
    }

    let expected = Decimal::from(working_days);
    if expected.is_zero() {
        return Err(AppError::BadRequest(
            "The selected dates contain no working days".into(),
        ));
    }
    let half = Decimal::new(5, 1);
    if days > expected {
        return Err(AppError::BadRequest(format!(
            "Leave days ({days}) cannot exceed the {expected} working day(s) in the selected dates"
        )));
    }
    if days < expected - half {
        return Err(AppError::BadRequest(format!(
            "Leave days ({days}) is less than the {expected} working day(s) covered by the selected dates; shorten the date range"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    /// The reported defect: a month-long range claimed as half a day deducts
    /// 0.5 from the balance while the range excuses the whole month.
    #[test]
    fn rejects_a_month_long_range_claimed_as_a_half_day() {
        let err = validate_period(d(2026, 7, 1), d(2026, 7, 31), Decimal::new(5, 1), 23);
        assert!(matches!(err, Err(AppError::BadRequest(_))));
    }

    #[test]
    fn rejects_days_exceeding_the_working_days_in_range() {
        let err = validate_period(d(2026, 7, 1), d(2026, 7, 3), Decimal::from(10), 3);
        assert!(matches!(err, Err(AppError::BadRequest(_))));
    }

    #[test]
    fn accepts_a_full_range_and_a_single_half_day() {
        assert!(validate_period(d(2026, 7, 1), d(2026, 7, 3), Decimal::from(3), 3).is_ok());
        // Half-day leave on one working day.
        assert!(validate_period(d(2026, 7, 1), d(2026, 7, 1), Decimal::new(5, 1), 1).is_ok());
        // A week with one public holiday: 4 working days claimed as 4.
        assert!(validate_period(d(2026, 7, 6), d(2026, 7, 10), Decimal::from(4), 4).is_ok());
        // Half day off at one end of a 3-working-day range.
        assert!(validate_period(d(2026, 7, 1), d(2026, 7, 3), Decimal::new(25, 1), 3).is_ok());
    }

    #[test]
    fn rejects_a_range_with_no_working_days() {
        let err = validate_period(d(2026, 7, 4), d(2026, 7, 5), Decimal::from(1), 0);
        assert!(matches!(err, Err(AppError::BadRequest(_))));
    }

    #[test]
    fn still_rejects_inverted_ranges_and_non_positive_days() {
        assert!(validate_period(d(2026, 7, 5), d(2026, 7, 1), Decimal::from(1), 1).is_err());
        assert!(validate_period(d(2026, 7, 1), d(2026, 7, 5), Decimal::ZERO, 5).is_err());
    }
}
