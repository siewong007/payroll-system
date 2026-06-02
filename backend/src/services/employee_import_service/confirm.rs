use sqlx::PgPool;
use uuid::Uuid;

use super::values::{parse_bool, parse_date, parse_money_to_sen};
use crate::core::error::{AppError, AppResult};
use crate::models::employee::CreateEmployeeRequest;
use crate::models::employee_import::*;

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
    let session = sqlx::query!(
        r#"SELECT company_id, user_id, file_name, validated_data, status, expires_at
            FROM bulk_import_sessions WHERE id = $1"#,
        req.session_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Import session not found".into()))?;

    let sess_company_id = session.company_id;
    let sess_user_id = session.user_id;
    let validated_data = session.validated_data;
    let status = session.status;
    let expires_at = session.expires_at;

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
    let mut tx = pool.begin().await?;

    for row_validation in &valid_rows {
        let create_req = row_to_create_request(&row_validation.data);
        let id = Uuid::now_v7();

        let result = sqlx::query!(
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
                $1, $2, $3, $4, $5, $6, $7, $8::text::gender_type, $9, $10::text::race_type,
                $11::text::residency_status, $12::text::marital_status,
                $13, $14, $15, $16, $17, $18, $19,
                $20, $21, $22, $23, $24::text::employment_type, $25, $26, $27,
                $28, $29, $30, $31, $32, $33, $34, $35, $36, $37,
                $38, $39, $40, $41, $42, $43, $44, $45, $46, $47, $48
            )"#,
            id,
            company_id,
            create_req.employee_number,
            create_req.full_name,
            create_req.ic_number,
            create_req.passport_number,
            create_req.date_of_birth,
            create_req.gender,
            create_req.nationality,
            create_req.race,
            create_req.residency_status.as_deref().unwrap_or("citizen"),
            create_req.marital_status,
            create_req.email,
            create_req.phone,
            create_req.address_line1,
            create_req.address_line2,
            create_req.city,
            create_req.state,
            create_req.postcode,
            create_req.department,
            create_req.designation,
            create_req.cost_centre,
            create_req.branch,
            create_req.employment_type.as_deref().unwrap_or("permanent"),
            create_req.date_joined,
            create_req.probation_start,
            create_req.probation_end,
            create_req.basic_salary,
            create_req.hourly_rate,
            create_req.daily_rate,
            create_req.bank_name,
            create_req.bank_account_number,
            create_req.bank_account_type,
            create_req.tax_identification_number,
            create_req.epf_number,
            create_req.socso_number,
            create_req.eis_number,
            create_req.working_spouse,
            create_req.num_children,
            create_req.epf_category,
            create_req.is_muslim,
            create_req.zakat_eligible,
            create_req.zakat_monthly_amount,
            create_req.ptptn_monthly_amount,
            create_req.tabung_haji_amount,
            create_req.payroll_group_id,
            create_req.salary_group,
            user_id,
        )
        .execute(&mut *tx)
        .await;

        match result {
            Ok(_) => {
                let _ = sqlx::query!(
                    r#"INSERT INTO salary_history (id, employee_id, old_salary, new_salary, effective_date, reason, created_by)
                    VALUES ($1, $2, 0, $3, $4, 'Initial salary (bulk import)', $5)"#,
                    Uuid::now_v7(),
                    id,
                    create_req.basic_salary,
                    create_req.date_joined,
                    user_id,
                )
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

    sqlx::query!(
        "UPDATE bulk_import_sessions SET status = 'confirmed', confirmed_at = NOW() WHERE id = $1",
        req.session_id,
    )
    .execute(pool)
    .await?;

    let audit_data = serde_json::json!({
        "session_id": req.session_id,
        "total_imported": imported_count,
        "skipped": failed_rows.len() + invalid_rows.len(),
    });

    sqlx::query!(
        r#"INSERT INTO audit_logs (id, user_id, action, entity_type, description, new_values)
        VALUES ($1, $2, 'bulk_import', 'employee', $3, $4)"#,
        Uuid::now_v7(),
        user_id,
        format!("Bulk imported {} employees", imported_count),
        audit_data,
    )
    .execute(pool)
    .await?;

    let skipped_count = failed_rows.len() + invalid_rows.len();

    Ok(ImportConfirmResponse {
        imported_count,
        skipped_count,
        errors: failed_rows,
    })
}
