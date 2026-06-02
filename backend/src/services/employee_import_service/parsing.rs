use std::collections::HashMap;
use std::io::Cursor;

use calamine::{Reader, Xlsx, open_workbook_from_rs};

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
            &["hourly_rate", "hourly rate", "rate per hour"],
            "hourly_rate",
        ),
        (&["daily_rate", "daily rate", "rate per day"], "daily_rate"),
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
            let normalized = h.trim().to_lowercase();
            alias_map.get(&normalized).copied()
        })
        .collect()
}

pub fn parse_csv(data: &[u8]) -> AppResult<(Vec<String>, Vec<Vec<String>>)> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(data);

    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| AppError::BadRequest(format!("Failed to read CSV headers: {}", e)))?
        .iter()
        .map(|s| s.to_string())
        .collect();

    if headers.is_empty() {
        return Err(AppError::BadRequest("CSV file has no headers".into()));
    }

    let mut rows = Vec::new();
    for result in rdr.records() {
        let record =
            result.map_err(|e| AppError::BadRequest(format!("Failed to read CSV row: {}", e)))?;
        let row: Vec<String> = record.iter().map(|s| s.to_string()).collect();
        if row.iter().all(|s| s.trim().is_empty()) {
            continue;
        }
        rows.push(row);
    }

    Ok((headers, rows))
}

pub fn parse_xlsx(data: &[u8]) -> AppResult<(Vec<String>, Vec<Vec<String>>)> {
    let cursor = Cursor::new(data);
    let mut workbook: Xlsx<_> = open_workbook_from_rs(cursor)
        .map_err(|e| AppError::BadRequest(format!("Failed to open Excel file: {}", e)))?;

    let sheet_name = workbook
        .sheet_names()
        .first()
        .ok_or_else(|| AppError::BadRequest("Excel file has no sheets".into()))?
        .clone();

    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|e| AppError::BadRequest(format!("Failed to read sheet: {}", e)))?;

    let mut row_iter = range.rows();

    let header_row = row_iter
        .next()
        .ok_or_else(|| AppError::BadRequest("Excel file has no headers".into()))?;

    let headers: Vec<String> = header_row
        .iter()
        .map(|cell| cell.to_string().trim().to_string())
        .collect();

    if headers.is_empty() || headers.iter().all(|h| h.is_empty()) {
        return Err(AppError::BadRequest(
            "Excel file has no valid headers".into(),
        ));
    }

    let mut rows = Vec::new();
    for row in row_iter {
        let cells: Vec<String> = row
            .iter()
            .map(|cell| cell.to_string().trim().to_string())
            .collect();
        if cells.iter().all(|s| s.is_empty()) {
            continue;
        }
        rows.push(cells);
    }

    Ok((headers, rows))
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
