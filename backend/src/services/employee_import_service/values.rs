use chrono::NaiveDate;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};

pub(crate) fn parse_date(s: &str) -> Result<NaiveDate, String> {
    let formats = ["%Y-%m-%d", "%d/%m/%Y", "%d-%m-%Y", "%Y/%m/%d", "%d.%m.%Y"];
    for fmt in &formats {
        if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
            return Ok(d);
        }
    }
    Err(format!(
        "Invalid date '{}'. Use YYYY-MM-DD or DD/MM/YYYY",
        s
    ))
}

pub(crate) fn parse_money_to_sen(s: &str) -> Result<i64, String> {
    let cleaned = s
        .replace(',', "")
        .replace("RM", "")
        .replace("rm", "")
        .trim()
        .to_string();
    let amount: Decimal = cleaned
        .parse()
        .map_err(|_| format!("Invalid amount '{}'. Enter a number like 3500.00", s))?;
    if amount < Decimal::ZERO {
        return Err("Amount cannot be negative".into());
    }

    amount
        .checked_mul(Decimal::from(100))
        .and_then(|sen| {
            sen.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                .to_i64()
        })
        .ok_or_else(|| format!("Amount '{}' is outside the supported range", s))
}

pub(crate) fn parse_bool(s: &str) -> Result<bool, String> {
    match s.to_lowercase().trim() {
        "true" | "yes" | "1" | "y" => Ok(true),
        "false" | "no" | "0" | "n" => Ok(false),
        _ => Err(format!("Invalid boolean '{}'. Use yes/no or true/false", s)),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_bool, parse_date, parse_money_to_sen};
    use chrono::NaiveDate;

    #[test]
    fn parses_supported_date_formats() {
        let expected = NaiveDate::from_ymd_opt(2026, 7, 14).unwrap();
        for value in [
            "2026-07-14",
            "14/07/2026",
            "14-07-2026",
            "2026/07/14",
            "14.07.2026",
        ] {
            assert_eq!(parse_date(value), Ok(expected), "failed for {value}");
        }
        assert!(parse_date("2026-02-30").is_err());
    }

    #[test]
    fn parses_money_exactly_and_rounds_fractional_sen() {
        assert_eq!(parse_money_to_sen("RM 1,234.56"), Ok(123_456));
        assert_eq!(parse_money_to_sen("rm0.29"), Ok(29));
        assert_eq!(parse_money_to_sen("10.004"), Ok(1_000));
        assert_eq!(parse_money_to_sen("10.005"), Ok(1_001));
        assert_eq!(parse_money_to_sen("92233720368547758.07"), Ok(i64::MAX));
    }

    #[test]
    fn rejects_non_finite_negative_and_out_of_range_money() {
        for value in [
            "NaN",
            "inf",
            "-inf",
            "-0.01",
            "92233720368547758.08",
            "not money",
        ] {
            assert!(
                parse_money_to_sen(value).is_err(),
                "amount should be rejected: {value}"
            );
        }
    }

    #[test]
    fn parses_boolean_aliases_case_insensitively() {
        for value in ["true", "YES", "1", " y "] {
            assert_eq!(parse_bool(value), Ok(true), "failed for {value}");
        }
        for value in ["false", "NO", "0", " N "] {
            assert_eq!(parse_bool(value), Ok(false), "failed for {value}");
        }
        assert!(parse_bool("enabled").is_err());
    }
}
