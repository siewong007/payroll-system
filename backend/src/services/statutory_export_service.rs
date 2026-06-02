use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::repositories::reads::statutory as statutory_reads;
use crate::repositories::reads::statutory::{CompanyStatutoryInfo, StatutoryRow};
use crate::services::pdf_helpers::sen_to_rm;

async fn get_statutory_data(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
    month: i32,
) -> AppResult<(CompanyStatutoryInfo, Vec<StatutoryRow>)> {
    let company = statutory_reads::company_statutory_info(pool, company_id).await?;

    let rows = statutory_reads::statutory_rows(pool, company_id, year, month).await?;

    if rows.is_empty() {
        return Err(AppError::NotFound(
            "No approved or paid payroll found for this period".into(),
        ));
    }

    Ok((company, rows))
}

pub async fn export_epf(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
    month: i32,
) -> AppResult<Vec<u8>> {
    let (company, rows) = get_statutory_data(pool, company_id, year, month).await?;

    let mut wtr = csv::Writer::from_writer(Vec::new());
    wtr.write_record([
        "Employer EPF No",
        "Employee EPF No",
        "IC Number",
        "Employee Name",
        "Wages (RM)",
        "Employee Contribution (RM)",
        "Employer Contribution (RM)",
    ])
    .map_err(|e| AppError::Internal(format!("CSV error: {}", e)))?;

    let employer_epf = company.epf_number.as_deref().unwrap_or("");
    for row in &rows {
        if row.epf_employee == 0 && row.epf_employer == 0 {
            continue;
        }
        wtr.write_record([
            employer_epf,
            row.epf_number.as_deref().unwrap_or(""),
            row.ic_number.as_deref().unwrap_or(""),
            &row.employee_name,
            &sen_to_rm(row.gross_salary),
            &sen_to_rm(row.epf_employee),
            &sen_to_rm(row.epf_employer),
        ])
        .map_err(|e| AppError::Internal(format!("CSV error: {}", e)))?;
    }

    wtr.into_inner()
        .map_err(|e| AppError::Internal(format!("CSV error: {}", e)))
}

pub async fn export_socso(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
    month: i32,
) -> AppResult<Vec<u8>> {
    let (company, rows) = get_statutory_data(pool, company_id, year, month).await?;

    let mut wtr = csv::Writer::from_writer(Vec::new());
    wtr.write_record([
        "Employer SOCSO Code",
        "Employee SOCSO No",
        "IC Number",
        "Employee Name",
        "Wages (RM)",
        "Employee Contribution (RM)",
        "Employer Contribution (RM)",
    ])
    .map_err(|e| AppError::Internal(format!("CSV error: {}", e)))?;

    let employer_socso = company.socso_code.as_deref().unwrap_or("");
    for row in &rows {
        if row.socso_employee == 0 && row.socso_employer == 0 {
            continue;
        }
        wtr.write_record([
            employer_socso,
            row.socso_number.as_deref().unwrap_or(""),
            row.ic_number.as_deref().unwrap_or(""),
            &row.employee_name,
            &sen_to_rm(row.gross_salary),
            &sen_to_rm(row.socso_employee),
            &sen_to_rm(row.socso_employer),
        ])
        .map_err(|e| AppError::Internal(format!("CSV error: {}", e)))?;
    }

    wtr.into_inner()
        .map_err(|e| AppError::Internal(format!("CSV error: {}", e)))
}

pub async fn export_eis(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
    month: i32,
) -> AppResult<Vec<u8>> {
    let (company, rows) = get_statutory_data(pool, company_id, year, month).await?;

    let mut wtr = csv::Writer::from_writer(Vec::new());
    wtr.write_record([
        "Employer EIS Code",
        "Employee EIS No",
        "IC Number",
        "Employee Name",
        "Wages (RM)",
        "Employee Contribution (RM)",
        "Employer Contribution (RM)",
    ])
    .map_err(|e| AppError::Internal(format!("CSV error: {}", e)))?;

    let employer_eis = company.eis_code.as_deref().unwrap_or("");
    for row in &rows {
        if row.eis_employee == 0 && row.eis_employer == 0 {
            continue;
        }
        wtr.write_record([
            employer_eis,
            row.eis_number.as_deref().unwrap_or(""),
            row.ic_number.as_deref().unwrap_or(""),
            &row.employee_name,
            &sen_to_rm(row.gross_salary),
            &sen_to_rm(row.eis_employee),
            &sen_to_rm(row.eis_employer),
        ])
        .map_err(|e| AppError::Internal(format!("CSV error: {}", e)))?;
    }

    wtr.into_inner()
        .map_err(|e| AppError::Internal(format!("CSV error: {}", e)))
}

pub async fn export_pcb_cp39(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
    month: i32,
) -> AppResult<Vec<u8>> {
    let (company, rows) = get_statutory_data(pool, company_id, year, month).await?;

    let mut output = String::new();
    // CP39 header
    output.push_str(&format!(
        "H|{}|{:02}{}|{}\n",
        company.tax_number.as_deref().unwrap_or(""),
        month,
        year,
        company.name
    ));

    for row in &rows {
        if row.pcb_amount == 0 {
            continue;
        }
        // D|Tax ID|IC Number|Employee Name|PCB Amount|Additional PCB
        output.push_str(&format!(
            "D|{}|{}|{}|{}|0.00\n",
            row.tax_identification_number.as_deref().unwrap_or(""),
            row.ic_number.as_deref().unwrap_or(""),
            row.employee_name,
            sen_to_rm(row.pcb_amount),
        ));
    }

    Ok(output.into_bytes())
}
