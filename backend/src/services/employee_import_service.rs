use std::collections::{HashMap, HashSet};
use std::io::Cursor;

use calamine::{Reader, Xlsx, open_workbook_from_rs};
use chrono::NaiveDate;
use rust_xlsxwriter::{Format, Workbook};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::employee::CreateEmployeeRequest;
use crate::models::employee_import::*;

// ─── Header Aliases ───

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

// ─── File Parsing ───

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
        // Skip completely empty rows
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

fn rows_to_import_rows(headers: &[String], rows: Vec<Vec<String>>) -> AppResult<Vec<ImportRowRaw>> {
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

// ─── Validation ───

fn parse_date(s: &str) -> Result<NaiveDate, String> {
    // Try common formats
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

fn parse_money_to_sen(s: &str) -> Result<i64, String> {
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

fn parse_bool(s: &str) -> Result<bool, String> {
    match s.to_lowercase().trim() {
        "true" | "yes" | "1" | "y" => Ok(true),
        "false" | "no" | "0" | "n" => Ok(false),
        _ => Err(format!("Invalid boolean '{}'. Use yes/no or true/false", s)),
    }
}

fn validate_row(row: &ImportRowRaw) -> Vec<FieldError> {
    let mut errors = Vec::new();

    // Required fields
    if row.employee_number.as_ref().is_none_or(|s| s.is_empty()) {
        errors.push(FieldError {
            field: "employee_number".into(),
            message: "Employee number is required".into(),
        });
    }
    if row.full_name.as_ref().is_none_or(|s| s.is_empty()) {
        errors.push(FieldError {
            field: "full_name".into(),
            message: "Full name is required".into(),
        });
    }
    if row.date_joined.is_none() {
        errors.push(FieldError {
            field: "date_joined".into(),
            message: "Date joined is required".into(),
        });
    }
    if row.basic_salary.is_none() {
        errors.push(FieldError {
            field: "basic_salary".into(),
            message: "Basic salary is required".into(),
        });
    }

    // Date validations
    for (field, value) in [
        ("date_of_birth", &row.date_of_birth),
        ("date_joined", &row.date_joined),
        ("probation_start", &row.probation_start),
        ("probation_end", &row.probation_end),
    ] {
        if let Some(v) = value
            && let Err(msg) = parse_date(v)
        {
            errors.push(FieldError {
                field: field.into(),
                message: msg,
            });
        }
    }

    // Money validations
    for (field, value) in [
        ("basic_salary", &row.basic_salary),
        ("hourly_rate", &row.hourly_rate),
        ("daily_rate", &row.daily_rate),
        ("zakat_monthly_amount", &row.zakat_monthly_amount),
        ("ptptn_monthly_amount", &row.ptptn_monthly_amount),
        ("tabung_haji_amount", &row.tabung_haji_amount),
    ] {
        if let Some(v) = value
            && let Err(msg) = parse_money_to_sen(v)
        {
            errors.push(FieldError {
                field: field.into(),
                message: msg,
            });
        }
    }

    // Enum validations
    if let Some(v) = &row.gender {
        let lower = v.to_lowercase();
        if !["male", "female"].contains(&lower.as_str()) {
            errors.push(FieldError {
                field: "gender".into(),
                message: format!("Invalid gender '{}'. Use male or female", v),
            });
        }
    }

    if let Some(v) = &row.employment_type {
        let lower = v.to_lowercase();
        if !["permanent", "contract", "part_time", "intern"].contains(&lower.as_str()) {
            errors.push(FieldError {
                field: "employment_type".into(),
                message: format!(
                    "Invalid employment type '{}'. Use permanent, contract, part_time, or intern",
                    v
                ),
            });
        }
    }

    if let Some(v) = &row.residency_status {
        let lower = v.to_lowercase();
        if !["citizen", "pr", "foreigner"].contains(&lower.as_str()) {
            errors.push(FieldError {
                field: "residency_status".into(),
                message: format!(
                    "Invalid residency status '{}'. Use citizen, pr, or foreigner",
                    v
                ),
            });
        }
    }

    if let Some(v) = &row.marital_status {
        let lower = v.to_lowercase();
        if !["single", "married", "divorced", "widowed"].contains(&lower.as_str()) {
            errors.push(FieldError {
                field: "marital_status".into(),
                message: format!(
                    "Invalid marital status '{}'. Use single, married, divorced, or widowed",
                    v
                ),
            });
        }
    }

    if let Some(v) = &row.race {
        let lower = v.to_lowercase();
        if !["malay", "chinese", "indian", "other"].contains(&lower.as_str()) {
            errors.push(FieldError {
                field: "race".into(),
                message: format!("Invalid race '{}'. Use malay, chinese, indian, or other", v),
            });
        }
    }

    // Boolean validations
    for (field, value) in [
        ("working_spouse", &row.working_spouse),
        ("is_muslim", &row.is_muslim),
        ("zakat_eligible", &row.zakat_eligible),
    ] {
        if let Some(v) = value
            && let Err(msg) = parse_bool(v)
        {
            errors.push(FieldError {
                field: field.into(),
                message: msg,
            });
        }
    }

    // Integer validation
    if let Some(v) = &row.num_children
        && v.parse::<i32>().is_err()
    {
        errors.push(FieldError {
            field: "num_children".into(),
            message: format!("Invalid number '{}'. Enter a whole number", v),
        });
    }

    // Email format
    if let Some(v) = &row.email
        && (!v.contains('@') || !v.contains('.'))
    {
        errors.push(FieldError {
            field: "email".into(),
            message: format!("Invalid email address '{}'", v),
        });
    }

    // UUID validation for payroll_group_id
    if let Some(v) = &row.payroll_group_id
        && Uuid::parse_str(v).is_err()
    {
        errors.push(FieldError {
            field: "payroll_group_id".into(),
            message: format!("Invalid payroll group ID '{}'. Must be a valid UUID", v),
        });
    }

    errors
}

// ─── Duplicate Detection ───

struct ExistingEmployees {
    employee_numbers: HashSet<String>,
    ic_numbers: HashSet<String>,
}

async fn load_existing(pool: &PgPool, company_id: Uuid) -> AppResult<ExistingEmployees> {
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT employee_number, ic_number FROM employees WHERE company_id = $1 AND deleted_at IS NULL",
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;

    let mut employee_numbers = HashSet::new();
    let mut ic_numbers = HashSet::new();
    for (emp_no, ic) in rows {
        employee_numbers.insert(emp_no.to_lowercase());
        if let Some(ic) = ic
            && !ic.is_empty()
        {
            ic_numbers.insert(ic.to_lowercase());
        }
    }

    Ok(ExistingEmployees {
        employee_numbers,
        ic_numbers,
    })
}

fn check_duplicates(
    row: &ImportRowRaw,
    existing: &ExistingEmployees,
    seen_emp_numbers: &HashMap<String, usize>,
    seen_ic_numbers: &HashMap<String, usize>,
) -> Vec<FieldError> {
    let mut errors = Vec::new();

    if let Some(emp_no) = &row.employee_number {
        let key = emp_no.to_lowercase();
        if existing.employee_numbers.contains(&key) {
            errors.push(FieldError {
                field: "employee_number".into(),
                message: format!("Employee number '{}' already exists", emp_no),
            });
        }
        if let Some(&other_row) = seen_emp_numbers.get(&key) {
            errors.push(FieldError {
                field: "employee_number".into(),
                message: format!(
                    "Duplicate employee number '{}' within file (also on row {})",
                    emp_no, other_row
                ),
            });
        }
    }

    if let Some(ic) = &row.ic_number {
        let key = ic.to_lowercase();
        if existing.ic_numbers.contains(&key) {
            errors.push(FieldError {
                field: "ic_number".into(),
                message: format!("IC number '{}' already exists", ic),
            });
        }
        if let Some(&other_row) = seen_ic_numbers.get(&key) {
            errors.push(FieldError {
                field: "ic_number".into(),
                message: format!(
                    "Duplicate IC number '{}' within file (also on row {})",
                    ic, other_row
                ),
            });
        }
    }

    errors
}

// ─── Public API ───

pub async fn validate_file(
    pool: &PgPool,
    company_id: Uuid,
    user_id: Uuid,
    file_name: &str,
    data: &[u8],
    is_xlsx: bool,
) -> AppResult<ImportValidationResponse> {
    let (headers, rows) = if is_xlsx {
        parse_xlsx(data)?
    } else {
        parse_csv(data)?
    };

    if rows.is_empty() {
        return Err(AppError::BadRequest("File contains no data rows".into()));
    }

    if rows.len() > 1000 {
        return Err(AppError::BadRequest(
            "Maximum 1000 rows per import. Please split into smaller files.".into(),
        ));
    }

    let import_rows = rows_to_import_rows(&headers, rows)?;
    let existing = load_existing(pool, company_id).await?;

    let mut validated_rows = Vec::with_capacity(import_rows.len());
    let mut seen_emp_numbers: HashMap<String, usize> = HashMap::new();
    let mut seen_ic_numbers: HashMap<String, usize> = HashMap::new();

    for row in import_rows {
        let mut errors = validate_row(&row);
        let dup_errors = check_duplicates(&row, &existing, &seen_emp_numbers, &seen_ic_numbers);

        // Track seen values for within-file duplicate detection
        if let Some(emp_no) = &row.employee_number {
            seen_emp_numbers
                .entry(emp_no.to_lowercase())
                .or_insert(row.row_number);
        }
        if let Some(ic) = &row.ic_number
            && !ic.is_empty()
        {
            seen_ic_numbers
                .entry(ic.to_lowercase())
                .or_insert(row.row_number);
        }

        let status = if !dup_errors.is_empty() {
            errors.extend(dup_errors);
            RowStatus::Duplicate
        } else if !errors.is_empty() {
            RowStatus::Error
        } else {
            RowStatus::Valid
        };

        validated_rows.push(ImportRowValidation {
            row_number: row.row_number,
            status,
            errors,
            data: row,
        });
    }

    let total_rows = validated_rows.len();
    let valid_rows = validated_rows
        .iter()
        .filter(|r| r.status == RowStatus::Valid)
        .count();
    let error_rows = validated_rows
        .iter()
        .filter(|r| r.status == RowStatus::Error)
        .count();
    let duplicate_rows = validated_rows
        .iter()
        .filter(|r| r.status == RowStatus::Duplicate)
        .count();

    // Store session
    let session_id = Uuid::new_v4();
    let validated_json = serde_json::to_value(&validated_rows)
        .map_err(|e| AppError::Internal(format!("Failed to serialize validation data: {}", e)))?;

    sqlx::query(
        r#"INSERT INTO bulk_import_sessions (id, company_id, user_id, file_name, row_count, valid_count, validated_data, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending')"#,
    )
    .bind(session_id)
    .bind(company_id)
    .bind(user_id)
    .bind(file_name)
    .bind(total_rows as i32)
    .bind(valid_rows as i32)
    .bind(&validated_json)
    .execute(pool)
    .await?;

    Ok(ImportValidationResponse {
        session_id,
        total_rows,
        valid_rows,
        error_rows,
        duplicate_rows,
        rows: validated_rows,
    })
}

fn row_to_create_request(row: &ImportRowRaw) -> CreateEmployeeRequest {
    CreateEmployeeRequest {
        employee_number: row.employee_number.clone().unwrap_or_default(),
        full_name: row.full_name.clone().unwrap_or_default(),
        ic_number: row.ic_number.clone(),
        passport_number: row.passport_number.clone(),
        date_of_birth: row.date_of_birth.as_ref().and_then(|s| parse_date(s).ok()),
        gender: row.gender.as_ref().map(|s| s.to_lowercase()),
        nationality: row.nationality.clone(),
        race: row.race.as_ref().map(|s| s.to_lowercase()),
        residency_status: row.residency_status.as_ref().map(|s| s.to_lowercase()),
        marital_status: row.marital_status.as_ref().map(|s| s.to_lowercase()),
        email: row.email.clone(),
        phone: row.phone.clone(),
        address_line1: row.address_line1.clone(),
        address_line2: row.address_line2.clone(),
        city: row.city.clone(),
        state: row.state.clone(),
        postcode: row.postcode.clone(),
        department: row.department.clone(),
        designation: row.designation.clone(),
        cost_centre: row.cost_centre.clone(),
        branch: row.branch.clone(),
        employment_type: row.employment_type.as_ref().map(|s| s.to_lowercase()),
        date_joined: row
            .date_joined
            .as_ref()
            .and_then(|s| parse_date(s).ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive()),
        probation_start: row
            .probation_start
            .as_ref()
            .and_then(|s| parse_date(s).ok()),
        probation_end: row.probation_end.as_ref().and_then(|s| parse_date(s).ok()),
        basic_salary: row
            .basic_salary
            .as_ref()
            .and_then(|s| parse_money_to_sen(s).ok())
            .unwrap_or(0),
        hourly_rate: row
            .hourly_rate
            .as_ref()
            .and_then(|s| parse_money_to_sen(s).ok()),
        daily_rate: row
            .daily_rate
            .as_ref()
            .and_then(|s| parse_money_to_sen(s).ok()),
        bank_name: row.bank_name.clone(),
        bank_account_number: row.bank_account_number.clone(),
        bank_account_type: row.bank_account_type.clone(),
        tax_identification_number: row.tax_identification_number.clone(),
        epf_number: row.epf_number.clone(),
        socso_number: row.socso_number.clone(),
        eis_number: row.eis_number.clone(),
        working_spouse: row.working_spouse.as_ref().and_then(|s| parse_bool(s).ok()),
        num_children: row.num_children.as_ref().and_then(|s| s.parse().ok()),
        epf_category: row.epf_category.clone(),
        is_muslim: row.is_muslim.as_ref().and_then(|s| parse_bool(s).ok()),
        zakat_eligible: row.zakat_eligible.as_ref().and_then(|s| parse_bool(s).ok()),
        zakat_monthly_amount: row
            .zakat_monthly_amount
            .as_ref()
            .and_then(|s| parse_money_to_sen(s).ok()),
        ptptn_monthly_amount: row
            .ptptn_monthly_amount
            .as_ref()
            .and_then(|s| parse_money_to_sen(s).ok()),
        tabung_haji_amount: row
            .tabung_haji_amount
            .as_ref()
            .and_then(|s| parse_money_to_sen(s).ok()),
        payroll_group_id: row
            .payroll_group_id
            .as_ref()
            .and_then(|s| Uuid::parse_str(s).ok()),
        salary_group: row.salary_group.clone(),
    }
}

pub async fn confirm_import(
    pool: &PgPool,
    company_id: Uuid,
    user_id: Uuid,
    req: ImportConfirmRequest,
) -> AppResult<ImportConfirmResponse> {
    // Load and verify session
    let session: (
        Uuid,
        Uuid,
        String,
        serde_json::Value,
        String,
        chrono::DateTime<chrono::Utc>,
    ) = sqlx::query_as(
        r#"SELECT company_id, user_id, file_name, validated_data, status, expires_at
            FROM bulk_import_sessions WHERE id = $1"#,
    )
    .bind(req.session_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Import session not found".into()))?;

    let (sess_company_id, sess_user_id, _file_name, validated_data, status, expires_at) = session;

    if sess_company_id != company_id || sess_user_id != user_id {
        return Err(AppError::Forbidden(
            "This import session belongs to another user".into(),
        ));
    }

    if status != "pending" {
        return Err(AppError::BadRequest(format!(
            "Import session is already {}",
            status
        )));
    }

    if expires_at < chrono::Utc::now() {
        return Err(AppError::BadRequest(
            "Import session has expired. Please upload the file again.".into(),
        ));
    }

    let rows: Vec<ImportRowValidation> = serde_json::from_value(validated_data)
        .map_err(|e| AppError::Internal(format!("Failed to deserialize session data: {}", e)))?;

    let (valid_rows, invalid_rows): (Vec<ImportRowValidation>, Vec<ImportRowValidation>) =
        rows.into_iter().partition(|r| r.status == RowStatus::Valid);

    if !req.skip_invalid && !invalid_rows.is_empty() {
        return Err(AppError::BadRequest(format!(
            "Cannot import: {} rows have errors. Set skip_invalid to true to import only valid rows.",
            invalid_rows.len()
        )));
    }

    let mut imported_count = 0;
    let mut failed_rows = Vec::new();

    // Use a transaction for the entire batch
    let mut tx = pool.begin().await?;

    for row_validation in &valid_rows {
        let create_req = row_to_create_request(&row_validation.data);
        let id = Uuid::new_v4();

        let result = sqlx::query(
            r#"INSERT INTO employees (
                id, company_id, employee_number, full_name, ic_number, passport_number,
                date_of_birth, gender, nationality, race, residency_status, marital_status,
                email, phone, address_line1, address_line2, city, state, postcode,
                department, designation, cost_centre, branch,
                employment_type, date_joined, probation_start, probation_end,
                basic_salary, hourly_rate, daily_rate,
                bank_name, bank_account_number, bank_account_type,
                tax_identification_number, epf_number, socso_number, eis_number,
                working_spouse, num_children, epf_category,
                is_muslim, zakat_eligible, zakat_monthly_amount,
                ptptn_monthly_amount, tabung_haji_amount,
                payroll_group_id, salary_group,
                created_by
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17, $18, $19,
                $20, $21, $22, $23, $24, $25, $26, $27,
                $28, $29, $30, $31, $32, $33, $34, $35, $36, $37,
                $38, $39, $40, $41, $42, $43, $44, $45, $46, $47, $48
            )"#,
        )
        .bind(id)
        .bind(company_id)
        .bind(&create_req.employee_number)
        .bind(&create_req.full_name)
        .bind(&create_req.ic_number)
        .bind(&create_req.passport_number)
        .bind(create_req.date_of_birth)
        .bind(&create_req.gender)
        .bind(&create_req.nationality)
        .bind(&create_req.race)
        .bind(create_req.residency_status.as_deref().unwrap_or("citizen"))
        .bind(&create_req.marital_status)
        .bind(&create_req.email)
        .bind(&create_req.phone)
        .bind(&create_req.address_line1)
        .bind(&create_req.address_line2)
        .bind(&create_req.city)
        .bind(&create_req.state)
        .bind(&create_req.postcode)
        .bind(&create_req.department)
        .bind(&create_req.designation)
        .bind(&create_req.cost_centre)
        .bind(&create_req.branch)
        .bind(create_req.employment_type.as_deref().unwrap_or("permanent"))
        .bind(create_req.date_joined)
        .bind(create_req.probation_start)
        .bind(create_req.probation_end)
        .bind(create_req.basic_salary)
        .bind(create_req.hourly_rate)
        .bind(create_req.daily_rate)
        .bind(&create_req.bank_name)
        .bind(&create_req.bank_account_number)
        .bind(&create_req.bank_account_type)
        .bind(&create_req.tax_identification_number)
        .bind(&create_req.epf_number)
        .bind(&create_req.socso_number)
        .bind(&create_req.eis_number)
        .bind(create_req.working_spouse)
        .bind(create_req.num_children)
        .bind(&create_req.epf_category)
        .bind(create_req.is_muslim)
        .bind(create_req.zakat_eligible)
        .bind(create_req.zakat_monthly_amount)
        .bind(create_req.ptptn_monthly_amount)
        .bind(create_req.tabung_haji_amount)
        .bind(create_req.payroll_group_id)
        .bind(&create_req.salary_group)
        .bind(user_id)
        .execute(&mut *tx)
        .await;

        match result {
            Ok(_) => {
                // Also create salary history record
                let _ = sqlx::query(
                    r#"INSERT INTO salary_history (id, employee_id, basic_salary, effective_date, reason, created_by)
                    VALUES ($1, $2, $3, $4, 'Initial salary (bulk import)', $5)"#,
                )
                .bind(Uuid::new_v4())
                .bind(id)
                .bind(create_req.basic_salary)
                .bind(create_req.date_joined)
                .bind(user_id)
                .execute(&mut *tx)
                .await;

                imported_count += 1;
            }
            Err(e) => {
                if req.skip_invalid {
                    failed_rows.push(ImportRowValidation {
                        row_number: row_validation.row_number,
                        status: RowStatus::Error,
                        errors: vec![FieldError {
                            field: "database".into(),
                            message: format!("Insert failed: {}", e),
                        }],
                        data: row_validation.data.clone(),
                    });
                } else {
                    return Err(AppError::Internal(format!(
                        "Failed to insert row {}: {}",
                        row_validation.row_number, e
                    )));
                }
            }
        }
    }

    tx.commit().await?;

    // Update session status
    sqlx::query(
        "UPDATE bulk_import_sessions SET status = 'confirmed', confirmed_at = NOW() WHERE id = $1",
    )
    .bind(req.session_id)
    .execute(pool)
    .await?;

    // Audit log
    let audit_data = serde_json::json!({
        "session_id": req.session_id,
        "total_imported": imported_count,
        "skipped": failed_rows.len() + invalid_rows.len(),
    });

    sqlx::query(
        r#"INSERT INTO audit_logs (id, user_id, action, entity_type, description, new_values)
        VALUES ($1, $2, 'bulk_import', 'employee', $3, $4)"#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(format!("Bulk imported {} employees", imported_count))
    .bind(&audit_data)
    .execute(pool)
    .await?;

    let skipped_count = failed_rows.len() + invalid_rows.len();

    Ok(ImportConfirmResponse {
        imported_count,
        skipped_count,
        errors: failed_rows,
    })
}

// ─── Template Generation ───

pub fn generate_template_xlsx() -> AppResult<Vec<u8>> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    let header_format = Format::new().set_bold();

    let headers = [
        "Employee Number",
        "Full Name",
        "IC Number",
        "Passport Number",
        "Date of Birth",
        "Gender",
        "Nationality",
        "Race",
        "Residency Status",
        "Marital Status",
        "Email",
        "Phone",
        "Address Line 1",
        "Address Line 2",
        "City",
        "State",
        "Postcode",
        "Department",
        "Designation",
        "Cost Centre",
        "Branch",
        "Employment Type",
        "Date Joined",
        "Probation Start",
        "Probation End",
        "Basic Salary (RM)",
        "Hourly Rate (RM)",
        "Daily Rate (RM)",
        "Bank Name",
        "Bank Account Number",
        "Bank Account Type",
        "Tax Identification Number",
        "EPF Number",
        "SOCSO Number",
        "EIS Number",
        "Working Spouse",
        "Num Children",
        "EPF Category",
        "Is Muslim",
        "Zakat Eligible",
        "Zakat Monthly (RM)",
        "PTPTN Monthly (RM)",
        "Tabung Haji (RM)",
        "Payroll Group ID",
        "Salary Group",
    ];

    let hints = [
        "Required",
        "Required",
        "12-digit MyKad number",
        "",
        "YYYY-MM-DD",
        "male / female",
        "",
        "malay / chinese / indian / other",
        "citizen / pr / foreigner",
        "single / married / divorced / widowed",
        "email@example.com",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "permanent / contract / part_time / intern",
        "Required. YYYY-MM-DD",
        "YYYY-MM-DD",
        "YYYY-MM-DD",
        "Required. e.g. 3500.00",
        "e.g. 20.00",
        "e.g. 160.00",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "yes / no",
        "Whole number",
        "",
        "yes / no",
        "yes / no",
        "e.g. 50.00",
        "e.g. 200.00",
        "e.g. 100.00",
        "UUID",
        "",
    ];

    let sample = [
        "EMP001",
        "Ahmad bin Abdullah",
        "901215145678",
        "",
        "1990-12-15",
        "male",
        "Malaysian",
        "malay",
        "citizen",
        "married",
        "ahmad@example.com",
        "0123456789",
        "123 Jalan Ampang",
        "Taman Maju",
        "Kuala Lumpur",
        "Selangor",
        "50450",
        "Engineering",
        "Software Engineer",
        "",
        "HQ",
        "permanent",
        "2024-01-15",
        "2024-01-15",
        "2024-04-15",
        "5000.00",
        "",
        "",
        "Maybank",
        "1234567890",
        "savings",
        "SG12345678",
        "12345678",
        "A12345678",
        "E12345678",
        "yes",
        "2",
        "A",
        "yes",
        "yes",
        "50.00",
        "",
        "",
        "",
        "",
    ];

    let hint_format = Format::new().set_italic().set_font_color("#666666");

    for (col, header) in headers.iter().enumerate() {
        worksheet
            .write_string_with_format(0, col as u16, *header, &header_format)
            .map_err(|e| AppError::Internal(format!("Failed to write header: {}", e)))?;

        if !hints[col].is_empty() {
            worksheet
                .write_string_with_format(1, col as u16, hints[col], &hint_format)
                .map_err(|e| AppError::Internal(format!("Failed to write hint: {}", e)))?;
        }

        if !sample[col].is_empty() {
            worksheet
                .write_string(2, col as u16, sample[col])
                .map_err(|e| AppError::Internal(format!("Failed to write sample: {}", e)))?;
        }
    }

    // Auto-fit column widths (approximate)
    for (col, header) in headers.iter().enumerate() {
        let width = header.len().max(15) as f64 + 2.0;
        worksheet
            .set_column_width(col as u16, width)
            .map_err(|e| AppError::Internal(format!("Failed to set column width: {}", e)))?;
    }

    let buf = workbook
        .save_to_buffer()
        .map_err(|e| AppError::Internal(format!("Failed to generate Excel file: {}", e)))?;

    Ok(buf)
}

pub fn generate_template_csv() -> AppResult<Vec<u8>> {
    let mut wtr = csv::Writer::from_writer(Vec::new());

    wtr.write_record([
        "Employee Number",
        "Full Name",
        "IC Number",
        "Passport Number",
        "Date of Birth",
        "Gender",
        "Nationality",
        "Race",
        "Residency Status",
        "Marital Status",
        "Email",
        "Phone",
        "Address Line 1",
        "Address Line 2",
        "City",
        "State",
        "Postcode",
        "Department",
        "Designation",
        "Cost Centre",
        "Branch",
        "Employment Type",
        "Date Joined",
        "Probation Start",
        "Probation End",
        "Basic Salary (RM)",
        "Hourly Rate (RM)",
        "Daily Rate (RM)",
        "Bank Name",
        "Bank Account Number",
        "Bank Account Type",
        "Tax Identification Number",
        "EPF Number",
        "SOCSO Number",
        "EIS Number",
        "Working Spouse",
        "Num Children",
        "EPF Category",
        "Is Muslim",
        "Zakat Eligible",
        "Zakat Monthly (RM)",
        "PTPTN Monthly (RM)",
        "Tabung Haji (RM)",
        "Payroll Group ID",
        "Salary Group",
    ])
    .map_err(|e| AppError::Internal(format!("Failed to write CSV headers: {}", e)))?;

    wtr.write_record([
        "EMP001",
        "Ahmad bin Abdullah",
        "901215145678",
        "",
        "1990-12-15",
        "male",
        "Malaysian",
        "malay",
        "citizen",
        "married",
        "ahmad@example.com",
        "0123456789",
        "123 Jalan Ampang",
        "Taman Maju",
        "Kuala Lumpur",
        "Selangor",
        "50450",
        "Engineering",
        "Software Engineer",
        "",
        "HQ",
        "permanent",
        "2024-01-15",
        "2024-01-15",
        "2024-04-15",
        "5000.00",
        "",
        "",
        "Maybank",
        "1234567890",
        "savings",
        "SG12345678",
        "12345678",
        "A12345678",
        "E12345678",
        "yes",
        "2",
        "A",
        "yes",
        "yes",
        "50.00",
        "",
        "",
        "",
        "",
    ])
    .map_err(|e| AppError::Internal(format!("Failed to write CSV sample: {}", e)))?;

    let buf = wtr
        .into_inner()
        .map_err(|e| AppError::Internal(format!("Failed to finalize CSV: {}", e)))?;

    Ok(buf)
}
