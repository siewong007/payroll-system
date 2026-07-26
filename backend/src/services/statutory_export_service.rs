use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::statutory::{CompanyStatutoryInfo, StatutoryRow};
use crate::repositories::reads::statutory as statutory_reads;
use crate::services::csv_helpers::{cp39_field, neutralize_formula, sen_to_plain_rm};

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

/// Every free-text column of these files is operator-entered — a name, an
/// agency reference — so all of them are formula-neutralised before they reach
/// the CSV writer. The writer still adds quoting where it is needed; the two are
/// orthogonal and compose.
fn text_field(value: &str) -> String {
    neutralize_formula(value)
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

    let employer_epf = text_field(company.epf_number.as_deref().unwrap_or(""));
    for row in &rows {
        if row.epf_employee == 0 && row.epf_employer == 0 {
            continue;
        }
        wtr.write_record([
            employer_epf.clone(),
            text_field(row.epf_number.as_deref().unwrap_or("")),
            text_field(row.ic_number.as_deref().unwrap_or("")),
            text_field(&row.employee_name),
            sen_to_plain_rm(row.gross_salary),
            sen_to_plain_rm(row.epf_employee),
            sen_to_plain_rm(row.epf_employer),
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

    let employer_socso = text_field(company.socso_code.as_deref().unwrap_or(""));
    for row in &rows {
        if row.socso_employee == 0 && row.socso_employer == 0 {
            continue;
        }
        wtr.write_record([
            employer_socso.clone(),
            text_field(row.socso_number.as_deref().unwrap_or("")),
            text_field(row.ic_number.as_deref().unwrap_or("")),
            text_field(&row.employee_name),
            sen_to_plain_rm(row.gross_salary),
            sen_to_plain_rm(row.socso_employee),
            sen_to_plain_rm(row.socso_employer),
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

    let employer_eis = text_field(company.eis_code.as_deref().unwrap_or(""));
    for row in &rows {
        if row.eis_employee == 0 && row.eis_employer == 0 {
            continue;
        }
        wtr.write_record([
            employer_eis.clone(),
            text_field(row.eis_number.as_deref().unwrap_or("")),
            text_field(row.ic_number.as_deref().unwrap_or("")),
            text_field(&row.employee_name),
            sen_to_plain_rm(row.gross_salary),
            sen_to_plain_rm(row.eis_employee),
            sen_to_plain_rm(row.eis_employer),
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
    // CP39 is a pipe-delimited fixed-shape record built by interpolation, with
    // no escaping mechanism of its own: a `|` in a name yields seven fields and
    // a newline splits one employee across two records. Nothing validates the
    // file before the operator uploads it to LHDN, so every interpolated field
    // is sanitised here and a changed value is logged — a mangled name has to
    // stay traceable after submission.
    output.push_str(&format!(
        "H|{}|{:02}{}|{}\n",
        cp39_field(company.tax_number.as_deref().unwrap_or("")),
        month,
        year,
        cp39_field(&company.name)
    ));

    for row in &rows {
        if row.pcb_amount == 0 {
            continue;
        }
        let name = cp39_field(&row.employee_name);
        if name != row.employee_name {
            tracing::warn!(
                original = %row.employee_name,
                sanitised = %name,
                "CP39 export: employee name contained a delimiter or control character"
            );
        }
        // D|Tax ID|IC Number|Employee Name|PCB Amount|Additional PCB
        output.push_str(&format!(
            "D|{}|{}|{}|{}|0.00\n",
            cp39_field(row.tax_identification_number.as_deref().unwrap_or("")),
            cp39_field(row.ic_number.as_deref().unwrap_or("")),
            name,
            sen_to_plain_rm(row.pcb_amount),
        ));
    }

    Ok(output.into_bytes())
}
