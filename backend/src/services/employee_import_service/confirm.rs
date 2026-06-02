use sqlx::PgPool;
use uuid::Uuid;

use super::values::{parse_bool, parse_date, parse_money_to_sen};
use crate::core::error::{AppError, AppResult};
use crate::models::employee::CreateEmployeeRequest;
use crate::models::employee_import::*;
use crate::repositories::{
    audit_logs, bulk_import_sessions, employees as employee_repo, salary_history,
};

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
    let session = bulk_import_sessions::get(pool, req.session_id)
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

        let result =
            employee_repo::insert_bulk_import(&mut *tx, id, company_id, &create_req, user_id).await;

        match result {
            Ok(_) => {
                let _ = salary_history::insert_bulk_import_initial(
                    &mut *tx,
                    Uuid::now_v7(),
                    id,
                    create_req.basic_salary,
                    create_req.date_joined,
                    user_id,
                )
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

    bulk_import_sessions::mark_confirmed(pool, req.session_id).await?;

    let audit_data = serde_json::json!({
        "session_id": req.session_id,
        "total_imported": imported_count,
        "skipped": failed_rows.len() + invalid_rows.len(),
    });

    audit_logs::insert_bulk_import(
        pool,
        Uuid::now_v7(),
        user_id,
        &format!("Bulk imported {} employees", imported_count),
        audit_data,
    )
    .await?;

    let skipped_count = failed_rows.len() + invalid_rows.len();

    Ok(ImportConfirmResponse {
        imported_count,
        skipped_count,
        errors: failed_rows,
    })
}
