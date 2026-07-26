//! Scheduling helpers for the background tasks spawned from `main.rs`.
//!
//! The daily auto-absent job must wake at a fixed UTC time of day. Computing
//! that instant is kept as a pure function so the "next run is strictly in the
//! future" invariant can be unit-tested: a zero or negative sleep duration in a
//! scheduling loop degrades into a busy spin that starves the runtime, which is
//! exactly the failure mode these tests pin down.

use chrono::{DateTime, Duration, Utc};

/// Next occurrence of `hour:minute:00` UTC strictly after `now`.
///
/// Returns today's target if it is still ahead; otherwise tomorrow's. The
/// result is never equal to `now`, so the delay derived from it is always
/// strictly positive — a tick that fires exactly at the target schedules the
/// following day rather than re-arming for "now".
pub fn next_daily_run_utc(now: DateTime<Utc>, hour: u32, minute: u32) -> DateTime<Utc> {
    let today_target = now
        .date_naive()
        .and_hms_opt(hour, minute, 0)
        .expect("valid wall-clock time of day")
        .and_utc();

    if today_target > now {
        today_target
    } else {
        today_target + Duration::days(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
    }

    #[test]
    fn before_target_schedules_same_day() {
        let next = next_daily_run_utc(at(2026, 7, 26, 2, 0, 0), 4, 30);
        assert_eq!(next, at(2026, 7, 26, 4, 30, 0));
    }

    #[test]
    fn after_target_schedules_next_day() {
        let next = next_daily_run_utc(at(2026, 7, 26, 4, 31, 0), 4, 30);
        assert_eq!(next, at(2026, 7, 27, 4, 30, 0));
    }

    #[test]
    fn exactly_at_target_schedules_next_day_not_zero_delay() {
        // The regression this file exists for: an "exactly at target" tick must
        // never produce a zero-length sleep (which would spin the loop).
        let now = at(2026, 7, 26, 4, 30, 0);
        let next = next_daily_run_utc(now, 4, 30);
        assert_eq!(next, at(2026, 7, 27, 4, 30, 0));
        assert!(next > now);
    }

    #[test]
    fn one_nanosecond_past_target_schedules_next_day() {
        let now = at(2026, 7, 26, 4, 30, 0) + Duration::nanoseconds(1);
        let next = next_daily_run_utc(now, 4, 30);
        assert_eq!(next, at(2026, 7, 27, 4, 30, 0));
    }

    #[test]
    fn rolls_over_month_and_year() {
        let next = next_daily_run_utc(at(2026, 12, 31, 23, 59, 59), 4, 30);
        assert_eq!(next, at(2027, 1, 1, 4, 30, 0));
    }

    #[test]
    fn handles_leap_day() {
        let next = next_daily_run_utc(at(2028, 2, 28, 5, 0, 0), 4, 30);
        assert_eq!(next, at(2028, 2, 29, 4, 30, 0));
    }

    #[test]
    fn delay_is_always_strictly_positive_and_at_most_a_day() {
        // Sweep a day of odd offsets around the target to pin the invariant.
        let base = at(2026, 7, 26, 0, 0, 0);
        for minutes in (0..(24 * 60)).step_by(7) {
            for extra_nanos in [0i64, 1, 999_999_999] {
                let now = base + Duration::minutes(minutes) + Duration::nanoseconds(extra_nanos);
                let next = next_daily_run_utc(now, 4, 30);
                let delay = next - now;
                assert!(delay > Duration::zero(), "zero/negative delay at {now}");
                assert!(delay <= Duration::days(1), "over-long delay at {now}");
            }
        }
    }
}
