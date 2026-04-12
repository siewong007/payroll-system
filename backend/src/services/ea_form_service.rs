use printpdf::*;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::services::pdf_helpers::*;

#[derive(Debug, Serialize)]
pub struct EaFormData {
    // Company
    pub company_name: String,
    pub company_reg_no: String,
    pub company_tax_no: String,
    pub company_epf_no: String,
    pub company_address: String,
    // Employee
    pub employee_name: String,
    pub employee_number: String,
    pub ic_number: String,
    pub tax_id: String,
    pub epf_number: String,
    pub socso_number: String,
    pub employee_address: String,
    pub date_joined: String,
    // Income
    pub ytd_basic: i64,
    pub ytd_allowances: i64,
    pub ytd_bonus: i64,
    pub ytd_commission: i64,
    pub ytd_overtime: i64,
    pub ytd_gross: i64,
    // Deductions
    pub ytd_epf_employee: i64,
    pub ytd_socso_employee: i64,
    pub ytd_eis_employee: i64,
    pub ytd_pcb: i64,
    pub ytd_zakat: i64,
    // Meta
    pub tax_year: i32,
    pub months_worked: i32,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct EaEmployeeSummary {
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_number: String,
    pub ic_number: Option<String>,
    pub ytd_gross: i64,
    pub months_worked: i64,
}

pub async fn list_employees_for_ea(
    pool: &PgPool,
    company_id: Uuid,
    year: i32,
) -> AppResult<Vec<EaEmployeeSummary>> {
    let rows = sqlx::query_as::<_, EaEmployeeSummary>(
        r#"SELECT
            pi.employee_id,
            e.full_name as employee_name,
            e.employee_number,
            e.ic_number,
            SUM(pi.gross_salary) as ytd_gross,
            COUNT(DISTINCT pr.period_month) as months_worked
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        JOIN employees e ON pi.employee_id = e.id
        WHERE pr.company_id = $1 AND pr.period_year = $2
        AND pr.status::text IN ('processed', 'approved', 'paid')
        AND e.deleted_at IS NULL
        GROUP BY pi.employee_id, e.full_name, e.employee_number, e.ic_number
        ORDER BY e.employee_number"#,
    )
    .bind(company_id)
    .bind(year)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_ea_form_data(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    year: i32,
) -> AppResult<EaFormData> {
    #[derive(sqlx::FromRow)]
    struct CompanyRow {
        name: String,
        registration_number: Option<String>,
        tax_number: Option<String>,
        epf_number: Option<String>,
        address_line1: Option<String>,
        address_line2: Option<String>,
        city: Option<String>,
        state: Option<String>,
        postcode: Option<String>,
    }

    let company = sqlx::query_as::<_, CompanyRow>(
        "SELECT name, registration_number, tax_number, epf_number, address_line1, address_line2, city, state, postcode FROM companies WHERE id = $1",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await?;

    #[derive(sqlx::FromRow)]
    struct EmployeeRow {
        full_name: String,
        employee_number: String,
        ic_number: Option<String>,
        tax_identification_number: Option<String>,
        epf_number: Option<String>,
        socso_number: Option<String>,
        address_line1: Option<String>,
        address_line2: Option<String>,
        city: Option<String>,
        state: Option<String>,
        postcode: Option<String>,
        date_joined: chrono::NaiveDate,
    }

    let emp = sqlx::query_as::<_, EmployeeRow>(
        r#"SELECT full_name, employee_number, ic_number, tax_identification_number,
            epf_number, socso_number, address_line1, address_line2, city, state, postcode, date_joined
        FROM employees WHERE id = $1 AND company_id = $2"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Employee not found".into()))?;

    #[derive(sqlx::FromRow)]
    struct AggRow {
        ytd_basic: i64,
        ytd_allowances: i64,
        ytd_bonus: i64,
        ytd_commission: i64,
        ytd_overtime: i64,
        ytd_gross: i64,
        ytd_epf_employee: i64,
        ytd_socso_employee: i64,
        ytd_eis_employee: i64,
        ytd_pcb: i64,
        ytd_zakat: i64,
        months_worked: i64,
    }

    let agg = sqlx::query_as::<_, AggRow>(
        r#"SELECT
            COALESCE(SUM(pi.basic_salary), 0) as ytd_basic,
            COALESCE(SUM(pi.total_allowances), 0) as ytd_allowances,
            COALESCE(SUM(pi.total_bonus), 0) as ytd_bonus,
            COALESCE(SUM(pi.total_commission), 0) as ytd_commission,
            COALESCE(SUM(pi.total_overtime), 0) as ytd_overtime,
            COALESCE(SUM(pi.gross_salary), 0) as ytd_gross,
            COALESCE(SUM(pi.epf_employee), 0) as ytd_epf_employee,
            COALESCE(SUM(pi.socso_employee), 0) as ytd_socso_employee,
            COALESCE(SUM(pi.eis_employee), 0) as ytd_eis_employee,
            COALESCE(SUM(pi.pcb_amount), 0) as ytd_pcb,
            COALESCE(SUM(pi.zakat_amount), 0) as ytd_zakat,
            COUNT(DISTINCT pr.period_month) as months_worked
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        WHERE pi.employee_id = $1 AND pr.company_id = $2 AND pr.period_year = $3
        AND pr.status::text IN ('processed', 'approved', 'paid')"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(year)
    .fetch_one(pool)
    .await?;

    let mut company_addr = vec![];
    if let Some(ref a) = company.address_line1 { company_addr.push(a.clone()); }
    if let Some(ref a) = company.address_line2 { company_addr.push(a.clone()); }
    let mut city_parts = vec![];
    if let Some(ref p) = company.postcode { city_parts.push(p.clone()); }
    if let Some(ref c) = company.city { city_parts.push(c.clone()); }
    if let Some(ref s) = company.state { city_parts.push(s.clone()); }
    if !city_parts.is_empty() { company_addr.push(city_parts.join(", ")); }

    let mut emp_addr = vec![];
    if let Some(ref a) = emp.address_line1 { emp_addr.push(a.clone()); }
    if let Some(ref a) = emp.address_line2 { emp_addr.push(a.clone()); }
    let mut emp_city = vec![];
    if let Some(ref p) = emp.postcode { emp_city.push(p.clone()); }
    if let Some(ref c) = emp.city { emp_city.push(c.clone()); }
    if let Some(ref s) = emp.state { emp_city.push(s.clone()); }
    if !emp_city.is_empty() { emp_addr.push(emp_city.join(", ")); }

    Ok(EaFormData {
        company_name: company.name,
        company_reg_no: company.registration_number.unwrap_or_default(),
        company_tax_no: company.tax_number.unwrap_or_default(),
        company_epf_no: company.epf_number.unwrap_or_default(),
        company_address: company_addr.join(", "),
        employee_name: emp.full_name,
        employee_number: emp.employee_number,
        ic_number: emp.ic_number.unwrap_or_default(),
        tax_id: emp.tax_identification_number.unwrap_or_default(),
        epf_number: emp.epf_number.unwrap_or_default(),
        socso_number: emp.socso_number.unwrap_or_default(),
        employee_address: emp_addr.join(", "),
        date_joined: emp.date_joined.to_string(),
        ytd_basic: agg.ytd_basic,
        ytd_allowances: agg.ytd_allowances,
        ytd_bonus: agg.ytd_bonus,
        ytd_commission: agg.ytd_commission,
        ytd_overtime: agg.ytd_overtime,
        ytd_gross: agg.ytd_gross,
        ytd_epf_employee: agg.ytd_epf_employee,
        ytd_socso_employee: agg.ytd_socso_employee,
        ytd_eis_employee: agg.ytd_eis_employee,
        ytd_pcb: agg.ytd_pcb,
        ytd_zakat: agg.ytd_zakat,
        tax_year: year,
        months_worked: agg.months_worked as i32,
    })
}

pub fn generate_ea_form_pdf(data: &EaFormData) -> AppResult<Vec<u8>> {
    let font = PdfFontHandle::Builtin(BuiltinFont::Helvetica);
    let bold = PdfFontHandle::Builtin(BuiltinFont::HelveticaBold);

    let mut ops: Vec<Op> = Vec::new();
    let left = 15.0;
    let right = 195.0;
    let mid = 105.0;
    let mut y = 280.0;

    // Title
    add_text(&mut ops, &bold, 14.0, left, y, "BORANG EA / FORM EA");
    y -= 5.0;
    add_text(&mut ops, &font, 9.0, left, y, &format!("Statement of Remuneration from Employment - Year {}", data.tax_year));
    y -= 4.0;
    add_text(&mut ops, &font, 8.0, left, y, "Penyata Saraan Daripada Penggajian (Subseksyen 83(1A) Akta Cukai Pendapatan 1967)");
    y -= 6.0;
    draw_line(&mut ops, left, right, y);
    y -= 8.0;

    // Section A: Employer details
    add_text(&mut ops, &bold, 10.0, left, y, "SECTION A: EMPLOYER DETAILS / BUTIRAN MAJIKAN");
    y -= 6.0;
    draw_row(&mut ops, &font, &bold, 9.0, left + 3.0, mid - 5.0, y, "Employer Name:", &data.company_name, false);
    y -= 5.0;
    draw_row(&mut ops, &font, &bold, 9.0, left + 3.0, mid - 5.0, y, "Employer Tax No:", &data.company_tax_no, false);
    add_text(&mut ops, &font, 9.0, mid, y, &format!("EPF Ref No: {}", data.company_epf_no));
    y -= 5.0;
    draw_row(&mut ops, &font, &bold, 9.0, left + 3.0, mid - 5.0, y, "Reg No:", &data.company_reg_no, false);
    y -= 5.0;
    add_text(&mut ops, &font, 9.0, left + 3.0, y, &format!("Address: {}", data.company_address));
    y -= 8.0;
    draw_line(&mut ops, left, right, y);
    y -= 8.0;

    // Section B: Employee details
    add_text(&mut ops, &bold, 10.0, left, y, "SECTION B: EMPLOYEE DETAILS / BUTIRAN PEKERJA");
    y -= 6.0;
    draw_row(&mut ops, &font, &bold, 9.0, left + 3.0, mid - 5.0, y, "Employee Name:", &data.employee_name, false);
    y -= 5.0;
    draw_row(&mut ops, &font, &bold, 9.0, left + 3.0, mid - 5.0, y, "IC Number:", &data.ic_number, false);
    add_text(&mut ops, &font, 9.0, mid, y, &format!("Tax Ref No: {}", data.tax_id));
    y -= 5.0;
    draw_row(&mut ops, &font, &bold, 9.0, left + 3.0, mid - 5.0, y, "EPF Number:", &data.epf_number, false);
    add_text(&mut ops, &font, 9.0, mid, y, &format!("SOCSO No: {}", data.socso_number));
    y -= 5.0;
    add_text(&mut ops, &font, 9.0, left + 3.0, y, &format!("Address: {}", data.employee_address));
    y -= 5.0;
    add_text(&mut ops, &font, 9.0, left + 3.0, y, &format!("Date Commenced Employment: {}", data.date_joined));
    y -= 8.0;
    draw_line(&mut ops, left, right, y);
    y -= 8.0;

    // Section C: Employment income
    add_text(&mut ops, &bold, 10.0, left, y, "SECTION C: EMPLOYMENT INCOME / PENDAPATAN PENGGAJIAN");
    y -= 7.0;

    let income_items: Vec<(&str, &str, i64)> = vec![
        ("1", "Salary / Wages / Gaji / Upah", data.ytd_basic),
        ("2", "Allowances / Elaun", data.ytd_allowances),
        ("3", "Bonus / Bonus", data.ytd_bonus),
        ("4", "Commission / Komisen", data.ytd_commission),
        ("5", "Overtime / Kerja Lebih Masa", data.ytd_overtime),
    ];

    for (num, label, amount) in &income_items {
        add_text(&mut ops, &font, 9.0, left + 3.0, y, num);
        add_text(&mut ops, &font, 9.0, left + 12.0, y, label);
        add_text_right(&mut ops, &font, 9.0, right, y, &format!("RM {}", sen_to_rm(*amount)));
        y -= 5.0;
    }

    y -= 2.0;
    draw_line(&mut ops, mid + 20.0, right, y + 1.0);
    y -= 5.0;
    add_text(&mut ops, &bold, 9.0, left + 3.0, y, "TOTAL EMPLOYMENT INCOME");
    add_text_right(&mut ops, &bold, 9.0, right, y, &format!("RM {}", sen_to_rm(data.ytd_gross)));
    y -= 8.0;
    draw_line(&mut ops, left, right, y);
    y -= 8.0;

    // Section D: Deductions
    add_text(&mut ops, &bold, 10.0, left, y, "SECTION D: DEDUCTIONS / POTONGAN");
    y -= 7.0;

    let deduction_items: Vec<(&str, &str, i64)> = vec![
        ("1", "EPF / KWSP (Employee)", data.ytd_epf_employee),
        ("2", "SOCSO / PERKESO (Employee)", data.ytd_socso_employee),
        ("3", "EIS / SIP (Employee)", data.ytd_eis_employee),
        ("4", "Income Tax Deducted (PCB/MTD)", data.ytd_pcb),
        ("5", "Zakat", data.ytd_zakat),
    ];

    for (num, label, amount) in &deduction_items {
        add_text(&mut ops, &font, 9.0, left + 3.0, y, num);
        add_text(&mut ops, &font, 9.0, left + 12.0, y, label);
        add_text_right(&mut ops, &font, 9.0, right, y, &format!("RM {}", sen_to_rm(*amount)));
        y -= 5.0;
    }

    y -= 2.0;
    draw_line(&mut ops, mid + 20.0, right, y + 1.0);
    y -= 5.0;
    let total_deductions = data.ytd_epf_employee + data.ytd_socso_employee + data.ytd_eis_employee + data.ytd_pcb + data.ytd_zakat;
    add_text(&mut ops, &bold, 9.0, left + 3.0, y, "TOTAL DEDUCTIONS");
    add_text_right(&mut ops, &bold, 9.0, right, y, &format!("RM {}", sen_to_rm(total_deductions)));
    y -= 8.0;
    draw_line(&mut ops, left, right, y);
    y -= 8.0;

    // Section E: Summary
    add_text(&mut ops, &bold, 10.0, left, y, "SECTION E: SUMMARY");
    y -= 7.0;
    add_text(&mut ops, &font, 9.0, left + 3.0, y, &format!("Months of Employment in {}: {}", data.tax_year, data.months_worked));
    y -= 5.0;
    let net = data.ytd_gross - total_deductions;
    add_text(&mut ops, &bold, 9.0, left + 3.0, y, "Net Remuneration:");
    add_text_right(&mut ops, &bold, 9.0, right, y, &format!("RM {}", sen_to_rm(net)));

    // Footer
    add_text(&mut ops, &font, 7.0, left, 20.0, "This is a computer-generated EA Form. No signature is required.");
    add_text(&mut ops, &font, 7.0, left, 15.0, &format!("Generated by PayrollMY for tax year {}", data.tax_year));

    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    let mut doc = PdfDocument::new(&format!("EA Form {} - {}", data.tax_year, data.employee_name));
    doc.pages.push(page);

    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    Ok(bytes)
}
