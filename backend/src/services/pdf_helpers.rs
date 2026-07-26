use printpdf::*;

/// Convert sen (i64) to formatted RM string e.g. "1,234.56"
pub fn sen_to_rm(sen: i64) -> String {
    let rm = sen as f64 / 100.0;
    let formatted = format!("{:.2}", rm.abs());
    let parts: Vec<&str> = formatted.split('.').collect();
    let int_part = parts[0];
    let dec_part = parts[1];

    // Add thousands separators
    let chars: Vec<char> = int_part.chars().collect();
    let mut result = String::new();
    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(*c);
    }

    if sen < 0 {
        format!("-{}.{}", result, dec_part)
    } else {
        format!("{}.{}", result, dec_part)
    }
}

/// Push ops to write text at a position (x, y in mm, from bottom-left)
pub fn add_text(ops: &mut Vec<Op>, font: &PdfFontHandle, size: f32, x: f32, y: f32, text: &str) {
    ops.push(Op::StartTextSection);
    ops.push(Op::SetFont {
        font: font.clone(),
        size: Pt(size),
    });
    ops.push(Op::SetTextCursor {
        pos: Point {
            x: Mm(x).into(),
            y: Mm(y).into(),
        },
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::Text(text.to_string())],
    });
    ops.push(Op::EndTextSection);
}

/// Push ops to write right-aligned text (approximate based on char count)
pub fn add_text_right(
    ops: &mut Vec<Op>,
    font: &PdfFontHandle,
    size: f32,
    right_x: f32,
    y: f32,
    text: &str,
) {
    let approx_width = text.len() as f32 * size * 0.22;
    let x = right_x - approx_width;
    add_text(ops, font, size, x, y, text);
}

/// Push ops to draw a horizontal line
pub fn draw_line(ops: &mut Vec<Op>, x1: f32, x2: f32, y: f32) {
    ops.push(Op::SetOutlineColor {
        col: Color::Greyscale(Greyscale::new(0.7, None)),
    });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });
    ops.push(Op::DrawLine {
        line: Line {
            points: vec![
                LinePoint {
                    p: Point::new(Mm(x1), Mm(y)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(x2), Mm(y)),
                    bezier: false,
                },
            ],
            is_closed: false,
        },
    });
}

/// Push ops for a simple table row with label on left and value on right
#[allow(clippy::too_many_arguments)]
pub fn draw_row(
    ops: &mut Vec<Op>,
    font: &PdfFontHandle,
    bold_font: &PdfFontHandle,
    size: f32,
    left_x: f32,
    right_x: f32,
    y: f32,
    label: &str,
    value: &str,
    bold: bool,
) {
    let f = if bold { bold_font } else { font };
    add_text(ops, f, size, left_x, y, label);
    add_text_right(ops, f, size, right_x, y, value);
}

#[cfg(test)]
mod tests {
    use super::sen_to_rm;

    #[test]
    fn groups_thousands_at_every_magnitude() {
        assert_eq!(sen_to_rm(99_999), "999.99");
        assert_eq!(sen_to_rm(100_000), "1,000.00");
        assert_eq!(sen_to_rm(99_999_999), "999,999.99");
        assert_eq!(sen_to_rm(100_000_000), "1,000,000.00");
        assert_eq!(sen_to_rm(123_456_789_012), "1,234,567,890.12");
    }

    #[test]
    fn never_emits_a_leading_separator() {
        // A boundary-length integer part (3, 6, 9 digits) is where an
        // off-by-one in the grouping loop shows up as ",100.00".
        // RM 100.00, RM 100,000.00 and RM 100,000,000.00 — integer parts of
        // exactly 3, 6 and 9 digits.
        for sen in [10_000, 10_000_000, 10_000_000_000] {
            let formatted = sen_to_rm(sen);
            assert!(
                !formatted.starts_with(','),
                "unexpected leading separator in {formatted}"
            );
        }
    }

    #[test]
    fn always_keeps_exactly_two_decimal_places() {
        for sen in [0, 5, 50, 500, 5_000, 100_000] {
            let formatted = sen_to_rm(sen);
            let decimals = formatted.split('.').nth(1).expect("a decimal part");
            assert_eq!(decimals.len(), 2, "{formatted} should have 2 decimals");
        }
    }

    #[test]
    fn negatives_keep_the_sign_outside_the_grouped_digits() {
        assert_eq!(sen_to_rm(-1), "-0.01");
        assert_eq!(sen_to_rm(-100_000), "-1,000.00");
        assert_eq!(sen_to_rm(-123_456_789), "-1,234,567.89");
    }

    #[test]
    fn a_negative_and_positive_amount_differ_only_by_the_sign() {
        for sen in [1_i64, 12_345, 100_000, 987_654_321] {
            assert_eq!(sen_to_rm(-sen), format!("-{}", sen_to_rm(sen)));
        }
    }

    #[test]
    fn formats_zero_without_a_sign() {
        assert_eq!(sen_to_rm(0), "0.00");
        assert!(!sen_to_rm(0).starts_with('-'));
    }
}
