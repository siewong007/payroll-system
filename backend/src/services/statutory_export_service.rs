use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::services::pdf_helpers::sen_to_rm;

#[derive(Debug, sqlx::FromRow)]
struct StatutoryRow {
    employee_name: String,
    ic_number: Option<String>,
    tax_identification_number: Option<String>,
    epf_number: Option<String>,
    socso_number: Option<String>,
    eis_number: Option<String>,
    gross_salary: i64,
    epf_employee: i64,
    epf_employer: i64,
    socso_employee: i64,
    socso_employer: i64,
    eis_employee: i64,
    eis_employer: i64,
    pcb_amount: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct CompanyStatutoryInfo {
    name: String,
    epf_number: Option<String>,
    socso_code: Option<String>,
    eis_code: Option<String>,
    tax_number: Option<String>,
}

async fn get_statutory_data(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
    month: i32,
) -> AppResult<(CompanyStatutoryInfo, Vec<StatutoryRow>)> {
    let company = sqlx::query_as::<_, CompanyStatutoryInfo>(
        "SELECT name, epf_number, socso_code, eis_code, tax_number FROM companies WHERE id = $1",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await?;

    let rows = sqlx::query_as::<_, StatutoryRow>(
        r#"SELECT
            e.full_name as employee_name, e.employee_number, e.ic_number,
            e.tax_identification_number, e.epf_number, e.socso_number, e.eis_number,
            pi.gross_salary,
            pi.epf_employee, pi.epf_employer,
            pi.socso_employee, pi.socso_employer,
            pi.eis_employee, pi.eis_employer,
            pi.pcb_amount, pi.zakat_amount
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        JOIN employees e ON pi.employee_id = e.id
        WHERE pr.company_id = $1 AND pr.period_year = $2 AND pr.period_month = $3
        AND pr.status::text IN ('processed', 'approved', 'paid')
        ORDER BY e.employee_number"#,
    )
    .bind(company_id)
    .bind(year)
    .bind(month)
    .fetch_all(pool)
    .await?;

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
    wtr.write_record(["Employer EPF No", "Employee EPF No", "IC Number", "Employee Name", "Wages (RM)", "Employee Contribution (RM)", "Employer Contribution (RM)"])
        .map_err(|e| AppError::Internal(format!("CSV error: {}", e)))?;

    let employer_epf = company.epf_number.as_deref().unwrap_or("");
    for row in &rows {
        if row.epf_employee == 0 && row.epf_employer == 0 { continue; }
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
    wtr.write_record(["Employer SOCSO Code", "Employee SOCSO No", "IC Number", "Employee Name", "Wages (RM)", "Employee Contribution (RM)", "Employer Contribution (RM)"])
        .map_err(|e| AppError::Internal(format!("CSV error: {}", e)))?;

    let employer_socso = company.socso_code.as_deref().unwrap_or("");
    for row in &rows {
        if row.socso_employee == 0 && row.socso_employer == 0 { continue; }
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
    wtr.write_record(["Employer EIS Code", "Employee EIS No", "IC Number", "Employee Name", "Wages (RM)", "Employee Contribution (RM)", "Employer Contribution (RM)"])
        .map_err(|e| AppError::Internal(format!("CSV error: {}", e)))?;

    let employer_eis = company.eis_code.as_deref().unwrap_or("");
    for row in &rows {
        if row.eis_employee == 0 && row.eis_employer == 0 { continue; }
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
        if row.pcb_amount == 0 { continue; }
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
