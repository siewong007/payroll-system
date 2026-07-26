use std::collections::{HashMap, HashSet};

use sqlx::PgPool;
use uuid::Uuid;

use super::parsing::{parse_csv, parse_xlsx, rows_to_import_rows};
use super::values::{parse_bool, parse_date, parse_money_to_sen};
use crate::core::error::{AppError, AppResult};
use crate::models::employee_import::*;
use crate::repositories::{bulk_import_sessions, employees as employee_repo};

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
    let rows = employee_repo::existing_numbers_and_ics(pool, company_id).await?;

    let mut employee_numbers = HashSet::new();
    let mut ic_numbers = HashSet::new();
    for (employee_number, ic_number) in rows {
        employee_numbers.insert(employee_number.to_lowercase());
        if let Some(ic) = ic_number
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

    bulk_import_sessions::insert_pending(
        pool,
        session_id,
        company_id,
        user_id,
        file_name,
        total_rows as i32,
        valid_rows as i32,
        validated_json,
    )
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

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use serde_json::json;

    use super::{check_duplicates, validate_row};
    use crate::models::employee_import::{ExistingEmployees, FieldError, ImportRowRaw};

    /// Builds a row from a JSON object. Every column is `Option<String>`, so
    /// serde fills the unlisted ones with `None` and each test states only the
    /// columns it cares about.
    fn row(overrides: serde_json::Value) -> ImportRowRaw {
        let mut base = json!({
            "row_number": 2,
            "employee_number": "E001",
            "full_name": "Nurul Huda",
            "date_joined": "2026-01-15",
            "basic_salary": "3500.00",
        });
        let serde_json::Value::Object(extra) = overrides else {
            panic!("row overrides must be a JSON object");
        };
        let base_map = base.as_object_mut().expect("base fixture is an object");
        for (key, value) in extra {
            base_map.insert(key, value);
        }
        serde_json::from_value(base).expect("row fixture should deserialize")
    }

    fn fields(errors: &[FieldError]) -> Vec<&str> {
        errors.iter().map(|e| e.field.as_str()).collect()
    }

    fn existing(employee_numbers: &[&str], ic_numbers: &[&str]) -> ExistingEmployees {
        ExistingEmployees {
            employee_numbers: employee_numbers.iter().map(|s| s.to_string()).collect(),
            ic_numbers: ic_numbers.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn a_minimal_complete_row_has_no_errors() {
        assert!(validate_row(&row(json!({}))).is_empty());
    }

    #[test]
    fn every_mandatory_field_is_reported_when_missing() {
        let bare: ImportRowRaw =
            serde_json::from_value(json!({ "row_number": 2 })).expect("bare row");
        let errors = validate_row(&bare);

        // All four are reported together so the operator fixes the sheet once
        // rather than re-uploading once per missing column.
        assert_eq!(
            fields(&errors),
            [
                "employee_number",
                "full_name",
                "date_joined",
                "basic_salary"
            ]
        );
    }

    #[test]
    fn empty_strings_count_as_missing_not_present() {
        let errors = validate_row(&row(json!({ "employee_number": "", "full_name": "" })));

        assert!(fields(&errors).contains(&"employee_number"));
        assert!(fields(&errors).contains(&"full_name"));
    }

    #[test]
    fn accepts_every_supported_date_format_for_date_joined() {
        for value in [
            "2026-01-15",
            "15/01/2026",
            "15-01-2026",
            "2026/01/15",
            "15.01.2026",
        ] {
            assert!(
                validate_row(&row(json!({ "date_joined": value }))).is_empty(),
                "should accept date {value}"
            );
        }
    }

    #[test]
    fn rejects_an_unparseable_or_impossible_date() {
        for value in ["15 Jan 2026", "2026-02-30", "not a date"] {
            let errors = validate_row(&row(json!({ "date_joined": value })));
            assert!(
                fields(&errors).contains(&"date_joined"),
                "should reject date {value}"
            );
        }
    }

    #[test]
    fn validates_every_optional_date_column() {
        let errors = validate_row(&row(json!({
            "date_of_birth": "bad",
            "probation_start": "bad",
            "probation_end": "bad",
        })));

        assert_eq!(
            fields(&errors),
            ["date_of_birth", "probation_start", "probation_end"]
        );
    }

    #[test]
    fn accepts_formatted_currency_and_rejects_negatives() {
        assert!(validate_row(&row(json!({ "basic_salary": "RM 3,500.00" }))).is_empty());

        let errors = validate_row(&row(json!({ "basic_salary": "-1" })));
        assert!(fields(&errors).contains(&"basic_salary"));
    }

    #[test]
    fn validates_every_optional_money_column() {
        let errors = validate_row(&row(json!({
            "hourly_rate": "abc",
            "daily_rate": "abc",
            "zakat_monthly_amount": "abc",
            "ptptn_monthly_amount": "abc",
            "tabung_haji_amount": "abc",
        })));

        assert_eq!(
            fields(&errors),
            [
                "hourly_rate",
                "daily_rate",
                "zakat_monthly_amount",
                "ptptn_monthly_amount",
                "tabung_haji_amount"
            ]
        );
    }

    #[test]
    fn enumerated_columns_accept_their_domain_case_insensitively() {
        let cases: [(&str, &[&str]); 5] = [
            ("gender", &["male", "FEMALE"]),
            (
                "employment_type",
                &["permanent", "Contract", "part_time", "INTERN"],
            ),
            ("residency_status", &["citizen", "PR", "foreigner"]),
            (
                "marital_status",
                &["single", "Married", "divorced", "WIDOWED"],
            ),
            ("race", &["malay", "Chinese", "indian", "OTHER"]),
        ];

        for (field, values) in cases {
            for value in values {
                assert!(
                    validate_row(&row(json!({ field: value }))).is_empty(),
                    "{field} should accept {value}"
                );
            }
        }
    }

    #[test]
    fn enumerated_columns_reject_values_outside_their_domain() {
        for field in [
            "gender",
            "employment_type",
            "residency_status",
            "marital_status",
            "race",
        ] {
            let errors = validate_row(&row(json!({ field: "unspecified" })));
            assert!(
                fields(&errors).contains(&field),
                "{field} should reject an out-of-domain value"
            );
        }
    }

    #[test]
    fn boolean_columns_accept_aliases_and_reject_prose() {
        assert!(
            validate_row(&row(json!({
                "working_spouse": "yes",
                "is_muslim": "TRUE",
                "zakat_eligible": "0",
            })))
            .is_empty()
        );

        let errors = validate_row(&row(json!({
            "working_spouse": "maybe",
            "is_muslim": "maybe",
            "zakat_eligible": "maybe",
        })));
        assert_eq!(
            fields(&errors),
            ["working_spouse", "is_muslim", "zakat_eligible"]
        );
    }

    #[test]
    fn children_count_must_be_a_whole_number() {
        assert!(validate_row(&row(json!({ "num_children": "3" }))).is_empty());
        assert!(validate_row(&row(json!({ "num_children": "0" }))).is_empty());

        for value in ["2.5", "two", ""] {
            let errors = validate_row(&row(json!({ "num_children": value })));
            assert!(
                fields(&errors).contains(&"num_children"),
                "should reject num_children {value:?}"
            );
        }
    }

    #[test]
    fn email_needs_both_an_at_sign_and_a_dot() {
        assert!(validate_row(&row(json!({ "email": "nurul@example.com" }))).is_empty());

        for value in ["nurul-at-example.com", "nurul@example", "plain"] {
            let errors = validate_row(&row(json!({ "email": value })));
            assert!(
                fields(&errors).contains(&"email"),
                "should reject email {value}"
            );
        }
    }

    #[test]
    fn payroll_group_must_be_a_uuid() {
        assert!(
            validate_row(&row(
                json!({ "payroll_group_id": "0193f0a0-0000-7000-8000-000000000000" })
            ))
            .is_empty()
        );

        let errors = validate_row(&row(json!({ "payroll_group_id": "monthly" })));
        assert!(fields(&errors).contains(&"payroll_group_id"));
    }

    #[test]
    fn a_row_reports_all_of_its_problems_at_once() {
        let errors = validate_row(&row(json!({
            "full_name": "",
            "gender": "unknown",
            "email": "broken",
            "basic_salary": "abc",
        })));

        assert_eq!(
            fields(&errors),
            ["full_name", "basic_salary", "gender", "email"]
        );
    }

    #[test]
    fn flags_an_employee_number_that_already_exists_in_the_tenant() {
        // Existing keys are stored lowercased, so the file's casing must not
        // let a duplicate slip past.
        let errors = check_duplicates(
            &row(json!({ "employee_number": "E001" })),
            &existing(&["e001"], &[]),
            &HashMap::new(),
            &HashMap::new(),
        );

        assert_eq!(fields(&errors), ["employee_number"]);
        assert!(errors[0].message.contains("already exists"));
    }

    #[test]
    fn flags_an_ic_number_that_already_exists_in_the_tenant() {
        let errors = check_duplicates(
            &row(json!({ "ic_number": "900101-14-5566" })),
            &existing(&[], &["900101-14-5566"]),
            &HashMap::new(),
            &HashMap::new(),
        );

        assert_eq!(fields(&errors), ["ic_number"]);
    }

    #[test]
    fn flags_a_duplicate_within_the_uploaded_file_and_names_the_other_row() {
        let mut seen = HashMap::new();
        seen.insert("e001".to_string(), 4usize);

        let errors = check_duplicates(
            &row(json!({ "employee_number": "E001" })),
            &existing(&[], &[]),
            &seen,
            &HashMap::new(),
        );

        assert_eq!(fields(&errors), ["employee_number"]);
        assert!(
            errors[0].message.contains("row 4"),
            "message should point at the other row: {}",
            errors[0].message
        );
    }

    #[test]
    fn reports_both_the_tenant_and_in_file_duplicate_when_both_apply() {
        let mut seen = HashMap::new();
        seen.insert("e001".to_string(), 4usize);

        let errors = check_duplicates(
            &row(json!({ "employee_number": "E001" })),
            &existing(&["e001"], &[]),
            &seen,
            &HashMap::new(),
        );

        assert_eq!(fields(&errors), ["employee_number", "employee_number"]);
    }

    #[test]
    fn a_clean_row_produces_no_duplicate_errors() {
        let errors = check_duplicates(
            &row(json!({ "employee_number": "E999", "ic_number": "880202-10-1234" })),
            &existing(&["e001"], &["900101-14-5566"]),
            &HashMap::new(),
            &HashMap::new(),
        );

        assert!(errors.is_empty());
    }

    #[test]
    fn a_row_without_an_ic_is_never_treated_as_an_ic_duplicate() {
        let existing = ExistingEmployees {
            employee_numbers: HashSet::new(),
            ic_numbers: HashSet::from([String::new()]),
        };

        let errors = check_duplicates(
            &row(json!({ "employee_number": "E999" })),
            &existing,
            &HashMap::new(),
            &HashMap::new(),
        );

        assert!(errors.is_empty());
    }
}
