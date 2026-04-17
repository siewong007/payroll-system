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
