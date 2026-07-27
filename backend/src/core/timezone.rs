//! IANA timezone handling: the single definition of the platform fallback plus
//! the two guards around a stored zone.
//!
//! Every attendance and payroll date bucket ends in `AT TIME ZONE $n` fed from
//! `company_work_schedules.timezone` (or `companies.timezone`) — free-text
//! columns a tenant admin writes. Postgres answers an unknown zone with
//! `invalid_parameter_value`, so the two paths need opposite treatment:
//!
//! * the **write** path rejects it up front, as a 400 naming the bad value;
//! * the **read** path degrades to the default, because a row corrupted before
//!   the guard existed must not 500 every check-in for that tenant forever.
//!
//! Parsing here rather than in SQL also keeps the failure domain per tenant:
//! a loop over every company can skip the one bad zone instead of having the
//! whole query aborted by it.

use chrono_tz::Tz;

use crate::core::error::{AppError, AppResult};

/// Platform fallback zone. The product is Malaysian; a company that never
/// configured a work schedule is on MYT.
pub const DEFAULT_TIMEZONE: &str = "Asia/Kuala_Lumpur";

/// Parse a stored or incoming zone into its canonical `chrono_tz::Tz`.
pub fn parse(tz: &str) -> Option<Tz> {
    tz.parse().ok()
}

/// Write path: reject an unknown zone before it reaches the column.
pub fn validate(tz: &str) -> AppResult<&str> {
    if parse(tz).is_some() {
        Ok(tz)
    } else {
        Err(AppError::BadRequest(format!(
            "'{tz}' is not a recognised IANA timezone (for example {DEFAULT_TIMEZONE})"
        )))
    }
}

/// Read path: a stored value that no longer parses degrades to the default,
/// with a warning naming it. `None` (no schedule row at all) is the ordinary
/// case, not an anomaly, and is not logged.
pub fn sanitize(stored: Option<String>) -> String {
    match stored {
        Some(tz) if parse(&tz).is_some() => tz,
        Some(bad) => {
            tracing::warn!(
                timezone = %bad,
                "unrecognised company timezone stored; falling back to {DEFAULT_TIMEZONE}"
            );
            DEFAULT_TIMEZONE.to_string()
        }
        None => DEFAULT_TIMEZONE.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_real_iana_zones() {
        for tz in [DEFAULT_TIMEZONE, "Asia/Jakarta", "Pacific/Honolulu", "UTC"] {
            assert!(parse(tz).is_some(), "should parse: {tz}");
        }
    }

    #[test]
    fn parse_rejects_typos_offsets_and_injection() {
        // "Asia/Kuala_Lumpr" is the realistic case: a hand-typed settings value
        // that Postgres would reject at `AT TIME ZONE`, mid-transaction.
        for tz in [
            "Asia/Kuala_Lumpr",
            "",
            "UTC+8",
            "+08:00",
            "'; DROP TABLE companies; --",
        ] {
            assert!(parse(tz).is_none(), "should not parse: {tz}");
        }
    }

    #[test]
    fn validate_is_a_bad_request_naming_the_offending_value() {
        assert!(validate("Asia/Jakarta").is_ok());

        let err = validate("Asia/Kuala_Lumpr").expect_err("a typo must be rejected");
        match err {
            AppError::BadRequest(msg) => assert!(
                msg.contains("Asia/Kuala_Lumpr"),
                "the message must name the value the admin typed: {msg}"
            ),
            other => panic!("expected BadRequest, got: {other:?}"),
        }
    }

    #[test]
    fn sanitize_passes_through_valid_zones_and_degrades_the_rest() {
        assert_eq!(sanitize(Some("Asia/Jakarta".into())), "Asia/Jakarta");
        assert_eq!(sanitize(Some("Asia/Kuala_Lumpr".into())), DEFAULT_TIMEZONE);
        assert_eq!(sanitize(None), DEFAULT_TIMEZONE);
    }
}
