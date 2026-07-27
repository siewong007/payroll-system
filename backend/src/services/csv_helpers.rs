//! Formatting for machine-read export files, as opposed to `pdf_helpers`, which
//! formats for a human reading a page.
//!
//! The statutory submissions (EPF, SOCSO, EIS, CP39) are parsed by the agency's
//! importer, not read by a person, so display conventions actively break them.

/// Machine-readable ringgit: no thousands separators, always two decimals.
///
/// `pdf_helpers::sen_to_rm` is for display and inserts separators, so RM1,700.00
/// of wages reached a numeric column of a statutory submission as `1,700.00` —
/// which the CSV writer then quotes, leaving a value the importer truncates at
/// the comma or rejects outright. In the pipe-delimited CP39 it produced a
/// seventh field.
///
/// Integer arithmetic deliberately: `sen as f64 / 100.0` is a rounding decision
/// on a figure that is already exact in sen.
pub fn sen_to_plain_rm(sen: i64) -> String {
    let sign = if sen < 0 { "-" } else { "" };
    let abs = sen.unsigned_abs();
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}

/// Force a spreadsheet to treat a value as text.
///
/// A cell opening with one of these bytes is executed as a formula by Excel and
/// LibreOffice, so an employee name (or an operator-entered reference number)
/// beginning with `=` is an injection vector into whoever opens the export.
/// Quoting is the CSV writer's job and is orthogonal — this only handles the
/// leading-byte execution vector, so the two compose.
pub fn neutralize_formula(s: &str) -> String {
    let formula_like = s
        .as_bytes()
        .first()
        .is_some_and(|first| matches!(*first, b'=' | b'+' | b'-' | b'@' | b'\t' | b'\r'));
    if formula_like {
        format!("'{s}")
    } else {
        s.to_string()
    }
}

/// Make a value safe to interpolate into a pipe-delimited fixed-shape record.
///
/// CP39 is hand-built with `format!` and has no escaping mechanism at all: a `|`
/// in an employee name turns six fields into seven, and a newline splits one
/// employee across two records. Neither is detectable by the operator before
/// they upload to LHDN, so the separators are replaced rather than escaped.
/// Other control characters go too — they survive the file but not the importer.
pub fn cp39_field(s: &str) -> String {
    s.chars()
        .map(|c| if c == '|' || c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{cp39_field, neutralize_formula, sen_to_plain_rm};

    #[test]
    fn plain_rm_never_groups_thousands() {
        assert_eq!(sen_to_plain_rm(170_000), "1700.00");
        assert_eq!(sen_to_plain_rm(100_000_000), "1000000.00");
        assert_eq!(sen_to_plain_rm(123_456_789_012), "1234567890.12");
    }

    #[test]
    fn plain_rm_always_keeps_two_decimals() {
        assert_eq!(sen_to_plain_rm(0), "0.00");
        assert_eq!(sen_to_plain_rm(5), "0.05");
        assert_eq!(sen_to_plain_rm(50), "0.50");
        assert_eq!(sen_to_plain_rm(500), "5.00");
    }

    #[test]
    fn plain_rm_keeps_the_sign_and_stays_parseable() {
        assert_eq!(sen_to_plain_rm(-170_000), "-1700.00");
        assert_eq!(sen_to_plain_rm(-1), "-0.01");
    }

    /// The whole point of the helper: an importer reading a numeric column must
    /// never meet a delimiter inside the value.
    #[test]
    fn plain_rm_output_is_free_of_separators_at_every_magnitude() {
        for sen in [
            0_i64,
            1,
            99,
            100,
            99_999,
            100_000,
            123_456_789_012,
            -100_000,
        ] {
            let formatted = sen_to_plain_rm(sen);
            assert!(!formatted.contains(','), "separator in {formatted}");
            assert!(
                formatted.parse::<f64>().is_ok(),
                "{formatted} does not parse as a number"
            );
        }
    }

    #[test]
    fn formula_prefixes_are_neutralized() {
        for value in [
            "=1+1",
            "+cmd|' /C calc'!A0",
            "-2+3",
            "@SUM(1,2)",
            "\t=1+1",
            "\r=1+1",
        ] {
            assert!(
                neutralize_formula(value).starts_with('\''),
                "not neutralized: {value:?}"
            );
        }
    }

    #[test]
    fn ordinary_text_is_untouched() {
        assert_eq!(neutralize_formula("Lee Wei"), "Lee Wei");
        assert_eq!(neutralize_formula("Doe, Jane"), "Doe, Jane");
        assert_eq!(neutralize_formula(""), "");
    }

    #[test]
    fn cp39_field_cannot_carry_the_delimiter_or_a_line_break() {
        assert_eq!(cp39_field("Lee|Wei"), "Lee Wei");
        assert_eq!(cp39_field("Lee\nWei"), "Lee Wei");
        assert_eq!(cp39_field("Lee\r\nWei"), "Lee  Wei");
        assert_eq!(cp39_field("  Lee Wei  "), "Lee Wei");
    }

    #[test]
    fn cp39_field_leaves_an_ordinary_name_alone() {
        assert_eq!(
            cp39_field("Nurul Ain binti Abdullah"),
            "Nurul Ain binti Abdullah"
        );
        assert_eq!(cp39_field("O'Brien-Tan"), "O'Brien-Tan");
    }

    /// The record shape is the invariant: sanitising every interpolated field
    /// must leave exactly the intended number of fields, whatever was in them.
    #[test]
    fn a_sanitised_cp39_record_keeps_its_field_count() {
        let record = format!(
            "D|{}|{}|{}|{}|0.00",
            cp39_field("SG|12345"),
            cp39_field("900101\n105432"),
            cp39_field("Lee|Wei"),
            sen_to_plain_rm(170_000),
        );
        assert_eq!(record.split('|').count(), 6);
        assert!(!record.contains('\n'));
    }
}
