use chrono::NaiveDate;

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
    let amount: f64 = cleaned
        .parse()
        .map_err(|_| format!("Invalid amount '{}'. Enter a number like 3500.00", s))?;
    if amount < 0.0 {
        return Err("Amount cannot be negative".into());
    }
    Ok((amount * 100.0).round() as i64)
}

pub(crate) fn parse_bool(s: &str) -> Result<bool, String> {
    match s.to_lowercase().trim() {
        "true" | "yes" | "1" | "y" => Ok(true),
        "false" | "no" | "0" | "n" => Ok(false),
        _ => Err(format!("Invalid boolean '{}'. Use yes/no or true/false", s)),
    }
}
