use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;

use calamine::{DataRef, Reader, Xlsx, open_workbook_from_rs};

use crate::core::error::{AppError, AppResult};
use crate::models::employee_import::ImportRowRaw;

fn build_header_map() -> HashMap<String, &'static str> {
    let aliases: Vec<(&[&str], &str)> = vec![
        (
            &[
                "employee_number",
                "employee number",
                "emp no",
                "emp number",
                "employee no",
            ],
            "employee_number",
        ),
        (
            &["full_name", "full name", "name", "employee name"],
            "full_name",
        ),
        (
            &["ic_number", "ic number", "nric", "ic no", "mykad"],
            "ic_number",
        ),
        (
            &[
                "passport_number",
                "passport number",
                "passport no",
                "passport",
            ],
            "passport_number",
        ),
        (
            &["date_of_birth", "date of birth", "dob", "birth date"],
            "date_of_birth",
        ),
        (&["gender", "sex"], "gender"),
        (&["nationality"], "nationality"),
        (&["race", "ethnicity"], "race"),
        (
            &[
                "residency_status",
                "residency status",
                "residency",
                "resident status",
            ],
            "residency_status",
        ),
        (
            &["marital_status", "marital status", "marital"],
            "marital_status",
        ),
        (&["email", "email address"], "email"),
        (
            &["phone", "phone number", "mobile", "contact number", "tel"],
            "phone",
        ),
        (
            &["address_line1", "address line 1", "address 1", "address"],
            "address_line1",
        ),
        (
            &["address_line2", "address line 2", "address 2"],
            "address_line2",
        ),
        (&["city", "town"], "city"),
        (&["state", "province"], "state"),
        (
            &["postcode", "post code", "zip", "zip code", "postal code"],
            "postcode",
        ),
        (&["department", "dept"], "department"),
        (
            &["designation", "position", "job title", "title"],
            "designation",
        ),
        (
            &["cost_centre", "cost centre", "cost center"],
            "cost_centre",
        ),
        (&["branch", "office", "location"], "branch"),
        (
            &["employment_type", "employment type", "emp type", "type"],
            "employment_type",
        ),
        (
            &[
                "date_joined",
                "date joined",
                "join date",
                "start date",
                "joining date",
            ],
            "date_joined",
        ),
        (
            &["probation_start", "probation start", "probation start date"],
            "probation_start",
        ),
        (
            &["probation_end", "probation end", "probation end date"],
            "probation_end",
        ),
        (
            &[
                "basic_salary",
                "basic salary",
                "salary",
                "basic salary (rm)",
                "monthly salary",
            ],
            "basic_salary",
        ),
        (
            &[
                "hourly_rate",
                "hourly rate",
                "hourly rate (rm)",
                "rate per hour",
            ],
            "hourly_rate",
        ),
        (
            &[
                "daily_rate",
                "daily rate",
                "daily rate (rm)",
                "rate per day",
            ],
            "daily_rate",
        ),
        (&["bank_name", "bank name", "bank"], "bank_name"),
        (
            &[
                "bank_account_number",
                "bank account number",
                "bank account",
                "account number",
                "account no",
            ],
            "bank_account_number",
        ),
        (
            &["bank_account_type", "bank account type", "account type"],
            "bank_account_type",
        ),
        (
            &[
                "tax_identification_number",
                "tax identification number",
                "tin",
                "tax number",
                "tax id",
            ],
            "tax_identification_number",
        ),
        (
            &["epf_number", "epf number", "epf no", "kwsp number"],
            "epf_number",
        ),
        (
            &["socso_number", "socso number", "socso no", "perkeso number"],
            "socso_number",
        ),
        (
            &["eis_number", "eis number", "eis no", "sip number"],
            "eis_number",
        ),
        (
            &["working_spouse", "working spouse", "spouse working"],
            "working_spouse",
        ),
        (
            &[
                "num_children",
                "num children",
                "number of children",
                "children",
                "no of children",
            ],
            "num_children",
        ),
        (
            &["epf_category", "epf category", "kwsp category"],
            "epf_category",
        ),
        (&["is_muslim", "is muslim", "muslim"], "is_muslim"),
        (
            &["zakat_eligible", "zakat eligible", "zakat"],
            "zakat_eligible",
        ),
        (
            &[
                "zakat_monthly_amount",
                "zakat monthly amount",
                "zakat amount",
                "zakat monthly (rm)",
            ],
            "zakat_monthly_amount",
        ),
        (
            &[
                "ptptn_monthly_amount",
                "ptptn monthly amount",
                "ptptn amount",
                "ptptn monthly (rm)",
            ],
            "ptptn_monthly_amount",
        ),
        (
            &[
                "tabung_haji_amount",
                "tabung haji amount",
                "tabung haji",
                "tabung haji (rm)",
            ],
            "tabung_haji_amount",
        ),
        (
            &["payroll_group_id", "payroll group id", "payroll group"],
            "payroll_group_id",
        ),
        (&["salary_group", "salary group"], "salary_group"),
    ];

    let mut map = HashMap::new();
    for (keys, field) in aliases {
        for key in keys {
            map.insert(key.to_string(), field);
        }
    }
    map
}

fn resolve_headers(raw_headers: &[String]) -> Vec<Option<&'static str>> {
    let alias_map = build_header_map();
    raw_headers
        .iter()
        .map(|h| {
            let normalized = h.trim_start_matches('\u{FEFF}').trim().to_lowercase();
            alias_map.get(&normalized).copied()
        })
        .collect()
}

pub fn parse_csv(data: &[u8]) -> AppResult<(Vec<String>, Vec<Vec<String>>)> {
    let data = data.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(data);
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(data);

    let headers_record = rdr
        .headers()
        .map_err(|e| AppError::BadRequest(format!("Failed to read CSV headers: {}", e)))?;
    let mut total_bytes: usize = 0;
    let mut headers = Vec::with_capacity(headers_record.len());
    for s in headers_record.iter() {
        if s.len() > MAX_CELL_BYTES {
            return Err(AppError::BadRequest(format!(
                "CSV cell exceeds maximum allowed length ({MAX_CELL_BYTES} bytes)"
            )));
        }
        total_bytes = total_bytes
            .checked_add(s.len())
            .filter(|&n| n <= MAX_TOTAL_PARSED_BYTES)
            .ok_or_else(|| {
                AppError::BadRequest("CSV file exceeds maximum parsed text size".into())
            })?;
        headers.push(s.to_string());
    }

    if headers.is_empty() {
        return Err(AppError::BadRequest("CSV file has no headers".into()));
    }
    if headers.len() > MAX_XLSX_COLS {
        return Err(AppError::BadRequest(format!(
            "CSV exceeds maximum supported column count ({MAX_XLSX_COLS})"
        )));
    }

    let mut rows = Vec::new();
    for result in rdr.records() {
        let record =
            result.map_err(|e| AppError::BadRequest(format!("Failed to read CSV row: {}", e)))?;
        if record.len() > MAX_XLSX_COLS {
            return Err(AppError::BadRequest(format!(
                "CSV row exceeds maximum supported column count ({MAX_XLSX_COLS})"
            )));
        }
        let mut row = Vec::with_capacity(record.len());
        for s in record.iter() {
            if s.len() > MAX_CELL_BYTES {
                return Err(AppError::BadRequest(format!(
                    "CSV cell exceeds maximum allowed length ({MAX_CELL_BYTES} bytes)"
                )));
            }
            total_bytes = total_bytes
                .checked_add(s.len())
                .filter(|&n| n <= MAX_TOTAL_PARSED_BYTES)
                .ok_or_else(|| {
                    AppError::BadRequest("CSV file exceeds maximum parsed text size".into())
                })?;
            row.push(s.to_string());
        }
        if row.iter().all(|s| s.trim().is_empty()) {
            continue;
        }
        if rows.len() >= MAX_IMPORT_ROWS {
            return Err(AppError::BadRequest(
                "Maximum 1000 rows per import. Please split into smaller files.".into(),
            ));
        }
        rows.push(row);
    }

    Ok((headers, rows))
}

const MAX_XLSX_ZIP_ENTRIES: usize = 200;
const MAX_XLSX_ENTRY_BYTES: u64 = 32 * 1024 * 1024;
const MAX_XLSX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_IMPORT_ROWS: usize = 1000;
const MAX_XLSX_ROWS: usize = 10_000;
const MAX_XLSX_COLS: usize = 200;
const MAX_CELL_BYTES: usize = 4_096;
const MAX_TOTAL_PARSED_BYTES: usize = 16 * 1024 * 1024;

fn validate_xlsx_archive(data: &[u8]) -> AppResult<()> {
    use std::io::Read;

    let mut archive = zip::ZipArchive::new(Cursor::new(data))
        .map_err(|e| AppError::BadRequest(format!("Invalid Excel archive: {}", e)))?;

    if archive.len() > MAX_XLSX_ZIP_ENTRIES {
        return Err(AppError::BadRequest(
            "Excel archive exceeds maximum entry count".into(),
        ));
    }

    let mut total_uncompressed: u64 = 0;
    for idx in 0..archive.len() {
        let entry = archive
            .by_index(idx)
            .map_err(|e| AppError::BadRequest(format!("Invalid Excel archive entry: {}", e)))?;
        let name = entry
            .name()
            .map_err(|e| AppError::BadRequest(format!("Invalid Excel archive entry: {}", e)))?;
        if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
            return Err(AppError::BadRequest(
                "Excel archive contains unsafe entry path".into(),
            ));
        }

        let mut limited = entry.take(MAX_XLSX_ENTRY_BYTES + 1);
        let written = std::io::copy(&mut limited, &mut std::io::sink()).map_err(|e| {
            AppError::BadRequest(format!("Failed to decompress Excel entry: {}", e))
        })?;
        if written > MAX_XLSX_ENTRY_BYTES {
            return Err(AppError::BadRequest(
                "Excel archive exceeds maximum entry size limit".into(),
            ));
        }
        total_uncompressed = total_uncompressed.saturating_add(written);
        if total_uncompressed > MAX_XLSX_TOTAL_BYTES {
            return Err(AppError::BadRequest(
                "Excel archive exceeds maximum uncompressed size limit".into(),
            ));
        }
    }

    Ok(())
}

fn excel_serial_to_date(serial: f64) -> Option<chrono::NaiveDate> {
    if !serial.is_finite() || !(1.0..=2_958_465.0).contains(&serial) {
        return None;
    }
    let whole = serial.floor() as i64;
    // Excel's 1900 epoch counts 1900-02-29 as serial 60, so dates >= 60 anchor
    // to 1899-12-30.
    let epoch = if whole >= 60 {
        chrono::NaiveDate::from_ymd_opt(1899, 12, 30)?
    } else {
        chrono::NaiveDate::from_ymd_opt(1899, 12, 31)?
    };
    epoch.checked_add_signed(chrono::Duration::days(whole))
}

fn check_xlsx_raw_cell_len(cell: &DataRef<'_>) -> AppResult<()> {
    let raw_len = match cell {
        DataRef::SharedString(s) => s.len(),
        DataRef::String(s) => s.len(),
        DataRef::DateTimeIso(s) => s.len(),
        DataRef::DurationIso(s) => s.len(),
        _ => 0,
    };
    if raw_len > MAX_CELL_BYTES {
        return Err(AppError::BadRequest(format!(
            "Excel cell exceeds maximum allowed length ({MAX_CELL_BYTES} bytes)"
        )));
    }
    Ok(())
}

fn format_xlsx_cell_ref(cell: &DataRef<'_>) -> String {
    match cell {
        DataRef::DateTime(dt) => excel_serial_to_date(dt.as_f64())
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| dt.to_string()),
        DataRef::DateTimeIso(s) => s.trim().get(..10).unwrap_or(s.trim()).to_string(),
        DataRef::DurationIso(s) => s.trim().to_string(),
        DataRef::SharedString(s) => (*s).to_string(),
        DataRef::String(s) => s.clone(),
        DataRef::Int(v) => v.to_string(),
        DataRef::Float(v) => v.to_string(),
        DataRef::Bool(v) => v.to_string(),
        DataRef::Error(e) => e.to_string(),
        DataRef::Empty => String::new(),
    }
    .trim()
    .to_string()
}

pub fn parse_xlsx(data: &[u8]) -> AppResult<(Vec<String>, Vec<Vec<String>>)> {
    validate_xlsx_archive(data)?;

    let cursor = Cursor::new(data);
    let mut workbook: Xlsx<_> = open_workbook_from_rs(cursor)
        .map_err(|e| AppError::BadRequest(format!("Failed to open Excel file: {}", e)))?;

    let sheet_name = workbook
        .sheet_names()
        .first()
        .ok_or_else(|| AppError::BadRequest("Excel file has no sheets".into()))?
        .clone();

    let mut cells_reader = workbook
        .worksheet_cells_reader(&sheet_name)
        .map_err(|e| AppError::BadRequest(format!("Failed to read sheet: {}", e)))?;

    let mut rows_by_idx: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    let mut total_bytes: usize = 0;
    while let Some(cell) = cells_reader
        .next_cell()
        .map_err(|e| AppError::BadRequest(format!("Failed to read sheet: {}", e)))?
    {
        let (row_idx, col_idx) = cell.get_position();
        if row_idx as usize >= MAX_XLSX_ROWS || col_idx as usize >= MAX_XLSX_COLS {
            return Err(AppError::BadRequest(
                "Excel sheet dimensions exceed maximum allowed size".into(),
            ));
        }

        check_xlsx_raw_cell_len(cell.get_value())?;
        let formatted = format_xlsx_cell_ref(cell.get_value());
        if formatted.is_empty() {
            continue;
        }
        if formatted.len() > MAX_CELL_BYTES {
            return Err(AppError::BadRequest(format!(
                "Excel cell exceeds maximum allowed length ({MAX_CELL_BYTES} bytes)"
            )));
        }
        total_bytes = total_bytes
            .checked_add(formatted.len())
            .filter(|&n| n <= MAX_TOTAL_PARSED_BYTES)
            .ok_or_else(|| {
                AppError::BadRequest("Excel sheet exceeds maximum parsed text size".into())
            })?;

        if !rows_by_idx.contains_key(&row_idx) && rows_by_idx.len() >= MAX_IMPORT_ROWS + 2 {
            return Err(AppError::BadRequest(
                "Maximum 1000 rows per import. Please split into smaller files.".into(),
            ));
        }

        let col = col_idx as usize;
        let row_vec = rows_by_idx.entry(row_idx).or_default();
        if row_vec.len() <= col {
            row_vec.resize(col + 1, String::new());
        }
        row_vec[col] = formatted;
    }

    let mut row_iter = rows_by_idx.into_values();

    let headers = row_iter
        .next()
        .ok_or_else(|| AppError::BadRequest("Excel file has no headers".into()))?;

    if headers.is_empty() || headers.iter().all(|h| h.is_empty()) {
        return Err(AppError::BadRequest(
            "Excel file has no valid headers".into(),
        ));
    }

    let mut rows = Vec::new();
    let mut checked_hint_row = false;
    for cells in row_iter {
        if cells.iter().all(|s| s.is_empty()) {
            continue;
        }
        if !checked_hint_row {
            checked_hint_row = true;
            if is_template_hint_row(&cells) {
                continue;
            }
        }
        if rows.len() >= MAX_IMPORT_ROWS {
            return Err(AppError::BadRequest(
                "Maximum 1000 rows per import. Please split into smaller files.".into(),
            ));
        }
        rows.push(cells);
    }

    Ok((headers, rows))
}

fn is_template_hint_row(cells: &[String]) -> bool {
    let lower: Vec<String> = cells
        .iter()
        .map(|c| c.trim().to_ascii_lowercase())
        .collect();
    let has_req_cols = lower.first().is_some_and(|c| c == "required")
        && lower.get(1).is_some_and(|c| c == "required");
    let has_hint_markers = lower.iter().any(|c| c == "12-digit mykad number")
        || lower.iter().any(|c| c == "male / female")
        || lower.iter().any(|c| c.starts_with("permanent / contract"));
    has_req_cols && has_hint_markers
}

pub(crate) fn rows_to_import_rows(
    headers: &[String],
    rows: Vec<Vec<String>>,
) -> AppResult<Vec<ImportRowRaw>> {
    let resolved = resolve_headers(headers);
    let mut result = Vec::with_capacity(rows.len());

    for (idx, row) in rows.into_iter().enumerate() {
        let mut fields: HashMap<&str, String> = HashMap::new();
        for (col_idx, value) in row.into_iter().enumerate() {
            if let Some(Some(field_name)) = resolved.get(col_idx) {
                let trimmed = value.trim().to_string();
                if !trimmed.is_empty() {
                    fields.insert(field_name, trimmed);
                }
            }
        }

        result.push(ImportRowRaw {
            row_number: idx + 1,
            employee_number: fields.remove("employee_number"),
            full_name: fields.remove("full_name"),
            ic_number: fields.remove("ic_number"),
            passport_number: fields.remove("passport_number"),
            date_of_birth: fields.remove("date_of_birth"),
            gender: fields.remove("gender"),
            nationality: fields.remove("nationality"),
            race: fields.remove("race"),
            residency_status: fields.remove("residency_status"),
            marital_status: fields.remove("marital_status"),
            email: fields.remove("email"),
            phone: fields.remove("phone"),
            address_line1: fields.remove("address_line1"),
            address_line2: fields.remove("address_line2"),
            city: fields.remove("city"),
            state: fields.remove("state"),
            postcode: fields.remove("postcode"),
            department: fields.remove("department"),
            designation: fields.remove("designation"),
            cost_centre: fields.remove("cost_centre"),
            branch: fields.remove("branch"),
            employment_type: fields.remove("employment_type"),
            date_joined: fields.remove("date_joined"),
            probation_start: fields.remove("probation_start"),
            probation_end: fields.remove("probation_end"),
            basic_salary: fields.remove("basic_salary"),
            hourly_rate: fields.remove("hourly_rate"),
            daily_rate: fields.remove("daily_rate"),
            bank_name: fields.remove("bank_name"),
            bank_account_number: fields.remove("bank_account_number"),
            bank_account_type: fields.remove("bank_account_type"),
            tax_identification_number: fields.remove("tax_identification_number"),
            epf_number: fields.remove("epf_number"),
            socso_number: fields.remove("socso_number"),
            eis_number: fields.remove("eis_number"),
            working_spouse: fields.remove("working_spouse"),
            num_children: fields.remove("num_children"),
            epf_category: fields.remove("epf_category"),
            is_muslim: fields.remove("is_muslim"),
            zakat_eligible: fields.remove("zakat_eligible"),
            zakat_monthly_amount: fields.remove("zakat_monthly_amount"),
            ptptn_monthly_amount: fields.remove("ptptn_monthly_amount"),
            tabung_haji_amount: fields.remove("tabung_haji_amount"),
            payroll_group_id: fields.remove("payroll_group_id"),
            salary_group: fields.remove("salary_group"),
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{parse_csv, resolve_headers, rows_to_import_rows};

    #[test]
    fn resolves_common_headers_case_insensitively() {
        let headers = vec![
            " Employee No ".to_string(),
            "NAME".to_string(),
            "Basic Salary (RM)".to_string(),
            "unknown field".to_string(),
        ];

        assert_eq!(
            resolve_headers(&headers),
            vec![
                Some("employee_number"),
                Some("full_name"),
                Some("basic_salary"),
                None,
            ]
        );
    }

    #[test]
    fn csv_parser_trims_cells_and_skips_blank_rows() {
        let csv = b"Employee Number,Name,Basic Salary\n E001 , Jane Doe ,\"3,500.00\"\n , , \n";
        let (headers, rows) = parse_csv(csv).expect("CSV should parse");

        assert_eq!(headers, ["Employee Number", "Name", "Basic Salary"]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0], ["E001", "Jane Doe", "3,500.00"]);
    }

    #[test]
    fn mapped_import_rows_ignore_unknown_columns_and_number_data_rows() {
        let headers = vec![
            "emp no".to_string(),
            "employee name".to_string(),
            "monthly salary".to_string(),
            "ignored".to_string(),
        ];
        let rows = vec![
            vec!["E001", "Jane Doe", "3500.00", "x"],
            vec!["E002", "John Doe", "4200.00", "y"],
        ]
        .into_iter()
        .map(|row| row.into_iter().map(str::to_string).collect())
        .collect();

        let mapped = rows_to_import_rows(&headers, rows).expect("rows should map");
        assert_eq!(mapped.len(), 2);
        assert_eq!(mapped[0].row_number, 1);
        assert_eq!(mapped[0].employee_number.as_deref(), Some("E001"));
        assert_eq!(mapped[0].full_name.as_deref(), Some("Jane Doe"));
        assert_eq!(mapped[0].basic_salary.as_deref(), Some("3500.00"));
        assert_eq!(mapped[1].row_number, 2);
        assert_eq!(mapped[1].employee_number.as_deref(), Some("E002"));
    }
}

/// Edge cases around the spreadsheet-shape handling, kept separate from the
/// happy-path mapping tests above.
#[cfg(test)]
mod shape_tests {
    use super::{parse_csv, parse_xlsx, resolve_headers, rows_to_import_rows};

    fn to_rows(rows: Vec<Vec<&str>>) -> Vec<Vec<String>> {
        rows.into_iter()
            .map(|row| row.into_iter().map(str::to_string).collect())
            .collect()
    }

    fn headers(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn every_alias_of_a_field_resolves_to_the_same_canonical_name() {
        let aliases = headers(&[
            "employee_number",
            "employee number",
            "emp no",
            "emp number",
            "employee no",
        ]);

        let resolved = resolve_headers(&aliases);
        assert!(
            resolved.iter().all(|r| *r == Some("employee_number")),
            "all employee-number aliases should resolve identically: {resolved:?}"
        );
    }

    #[test]
    fn identity_aliases_resolve_for_the_common_fields() {
        let canonical = headers(&[
            "full_name",
            "ic_number",
            "date_of_birth",
            "gender",
            "email",
            "basic_salary",
            "date_joined",
        ]);

        let resolved = resolve_headers(&canonical);
        assert!(
            resolved.iter().all(Option::is_some),
            "a canonical name must always resolve to itself: {resolved:?}"
        );
    }

    #[test]
    fn an_unknown_header_resolves_to_none_rather_than_erroring() {
        let resolved = resolve_headers(&headers(&["favourite colour", ""]));
        assert_eq!(resolved, vec![None, None]);
    }

    #[test]
    fn a_blank_cell_becomes_none_not_an_empty_string() {
        let mapped = rows_to_import_rows(
            &headers(&["emp no", "name", "email"]),
            to_rows(vec![vec!["E001", "Jane Doe", "   "]]),
        )
        .expect("rows should map");

        // Validation treats None as "absent"; an empty string would slip past
        // the required-field check and then fail deeper in the pipeline.
        assert_eq!(mapped[0].email, None);
        assert_eq!(mapped[0].full_name.as_deref(), Some("Jane Doe"));
    }

    #[test]
    fn a_short_row_leaves_the_trailing_fields_absent() {
        // Spreadsheet exports frequently omit trailing empty cells.
        let mapped = rows_to_import_rows(
            &headers(&["emp no", "name", "basic salary"]),
            to_rows(vec![vec!["E001", "Jane Doe"]]),
        )
        .expect("a short row should still map");

        assert_eq!(mapped[0].employee_number.as_deref(), Some("E001"));
        assert_eq!(mapped[0].basic_salary, None);
    }

    #[test]
    fn extra_cells_beyond_the_header_row_are_discarded() {
        let mapped = rows_to_import_rows(
            &headers(&["emp no", "name"]),
            to_rows(vec![vec!["E001", "Jane Doe", "stray", "more stray"]]),
        )
        .expect("a long row should still map");

        assert_eq!(mapped.len(), 1);
        assert_eq!(mapped[0].full_name.as_deref(), Some("Jane Doe"));
    }

    #[test]
    fn row_numbers_are_one_based_and_contiguous() {
        let mapped = rows_to_import_rows(
            &headers(&["emp no"]),
            to_rows(vec![vec!["E001"], vec!["E002"], vec!["E003"]]),
        )
        .expect("rows should map");

        assert_eq!(
            mapped.iter().map(|r| r.row_number).collect::<Vec<_>>(),
            [1, 2, 3]
        );
    }

    #[test]
    fn a_header_only_file_yields_no_rows() {
        let (headers, rows) = parse_csv(b"Employee Number,Name\n").expect("CSV should parse");

        assert_eq!(headers, ["Employee Number", "Name"]);
        assert!(rows.is_empty());
        assert!(
            rows_to_import_rows(&headers, rows)
                .expect("no rows should map")
                .is_empty()
        );
    }

    #[test]
    fn quoted_cells_keep_their_embedded_commas() {
        let csv = b"Name,Address\nJane Doe,\"12 Jalan Ampang, Kuala Lumpur\"\n";
        let (_, rows) = parse_csv(csv).expect("CSV should parse");

        assert_eq!(rows[0][1], "12 Jalan Ampang, Kuala Lumpur");
    }

    #[test]
    fn rows_of_differing_widths_are_accepted() {
        // The reader is built with `flexible(true)`; a ragged export must not
        // abort the whole import.
        let csv = b"A,B,C\n1,2,3\n4,5\n6,7,8,9\n";
        let (_, rows) = parse_csv(csv).expect("ragged CSV should parse");

        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn whitespace_only_rows_are_skipped_anywhere_in_the_file() {
        let csv = b"A,B\n1,2\n , \n3,4\n";
        let (_, rows) = parse_csv(csv).expect("CSV should parse");

        assert_eq!(rows, vec![vec!["1", "2"], vec!["3", "4"]]);
    }

    #[test]
    fn an_empty_upload_does_not_panic() {
        // Headers come back empty rather than the parser unwrapping a None.
        let parsed = parse_csv(b"");
        assert!(parsed.is_err() || parsed.expect("parsed").1.is_empty());
    }

    #[test]
    fn a_non_spreadsheet_upload_is_rejected_as_a_bad_request() {
        use crate::core::error::AppError;

        assert!(matches!(
            parse_xlsx(b"this is not a workbook"),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn maps_rate_header_aliases_and_detects_template_hint_row() {
        let csv = "Employee Number,Full Name,Daily Rate (RM),Hourly Rate (RM)\nEMP-01,Aisyah Rahman,120.00,15.50\n";
        let (headers, rows) = parse_csv(csv.as_bytes()).expect("CSV should parse");
        let mapped = rows_to_import_rows(&headers, rows).expect("rows should map");
        assert_eq!(mapped.len(), 1);
        assert_eq!(mapped[0].employee_number.as_deref(), Some("EMP-01"));
        assert_eq!(mapped[0].daily_rate.as_deref(), Some("120.00"));
        assert_eq!(mapped[0].hourly_rate.as_deref(), Some("15.50"));

        assert!(super::is_template_hint_row(
            &to_rows(vec![vec!["Required", "Required", "12-digit MyKad number",]])[0]
        ));
        assert!(!super::is_template_hint_row(
            &to_rows(vec![vec!["EMP-01", "Aisyah Rahman", "900101-14-5566",]])[0]
        ));
    }
}
