use std::collections::{HashMap, HashSet};

use sqlx::PgPool;
use uuid::Uuid;

use super::parsing::{parse_csv, parse_xlsx, rows_to_import_rows};
use super::values::{parse_bool, parse_date, parse_money_to_sen};
use crate::core::error::{AppError, AppResult};
use crate::models::employee_import::*;

fn validate_row(row: &ImportRowRaw) -> Vec<FieldError> {
    let mut errors = Vec::new();

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

    if let Some(v) = &row.num_children
        && v.parse::<i32>().is_err()
    {
        errors.push(FieldError {
            field: "num_children".into(),
            message: format!("Invalid number '{}'. Enter a whole number", v),
        });
    }

    if let Some(v) = &row.email
        && (!v.contains('@') || !v.contains('.'))
    {
        errors.push(FieldError {
            field: "email".into(),
            message: format!("Invalid email address '{}'", v),
        });
    }

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

async fn load_existing(pool: &PgPool, company_id: Uuid) -> AppResult<ExistingEmployees> {
    let rows = sqlx::query!(
        "SELECT employee_number, ic_number FROM employees WHERE company_id = $1 AND deleted_at IS NULL",
        company_id,
    )
    .fetch_all(pool)
    .await?;

    let mut employee_numbers = HashSet::new();
    let mut ic_numbers = HashSet::new();
    for r in rows {
        employee_numbers.insert(r.employee_number.to_lowercase());
        if let Some(ic) = r.ic_number
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

    let session_id = Uuid::now_v7();
    let validated_json = serde_json::to_value(&validated_rows)
        .map_err(|e| AppError::Internal(format!("Failed to serialize validation data: {}", e)))?;

    sqlx::query!(
        r#"INSERT INTO bulk_import_sessions (id, company_id, user_id, file_name, row_count, valid_count, validated_data, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending')"#,
        session_id,
        company_id,
        user_id,
        file_name,
        total_rows as i32,
        valid_rows as i32,
        validated_json,
    )
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
