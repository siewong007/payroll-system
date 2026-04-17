use printpdf::*;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::services::pdf_helpers::*;

#[derive(Debug, sqlx::FromRow)]
struct PayslipData {
    // Employee
    employee_name: String,
    employee_number: String,
    ic_number: Option<String>,
    department: Option<String>,
    designation: Option<String>,
    bank_name: Option<String>,
    bank_account_number: Option<String>,
    // Period
    period_year: i32,
    period_month: i32,
    period_start: chrono::NaiveDate,
    period_end: chrono::NaiveDate,
    pay_date: chrono::NaiveDate,
    // Earnings
    basic_salary: i64,
    gross_salary: i64,
    total_allowances: i64,
    total_overtime: i64,
    total_bonus: i64,
    total_commission: i64,
    total_claims: i64,
    // Deductions
    epf_employee: i64,
    epf_employer: i64,
    socso_employee: i64,
    socso_employer: i64,
    eis_employee: i64,
    eis_employer: i64,
    pcb_amount: i64,
    zakat_amount: i64,
    ptptn_amount: i64,
    tabung_haji_amount: i64,
    total_loan_deductions: i64,
    total_other_deductions: i64,
    unpaid_leave_deduction: i64,
    total_deductions: i64,
    net_salary: i64,
    employer_cost: i64,
    // YTD
    ytd_gross: i64,
    ytd_epf_employee: i64,
    ytd_pcb: i64,
    ytd_socso_employee: i64,
    ytd_eis_employee: i64,
    ytd_zakat: i64,
    ytd_net: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct CompanyInfo {
    name: String,
    registration_number: Option<String>,
    address_line1: Option<String>,
    address_line2: Option<String>,
    city: Option<String>,
    state: Option<String>,
    postcode: Option<String>,
}

pub async fn generate_payslip_pdf(
    pool: &PgPool,
    payslip_id: Uuid,
    employee_id: Uuid,
) -> AppResult<Vec<u8>> {
    let data = sqlx::query_as::<_, PayslipData>(
        r#"SELECT
            e.full_name as employee_name, e.employee_number, e.ic_number,
            e.department, e.designation, e.bank_name, e.bank_account_number,
            pr.period_year, pr.period_month, pr.period_start, pr.period_end, pr.pay_date,
            pi.basic_salary, pi.gross_salary, pi.total_allowances, pi.total_overtime,
            pi.total_bonus, pi.total_commission, pi.total_claims,
            pi.epf_employee, pi.epf_employer, pi.socso_employee, pi.socso_employer,
            pi.eis_employee, pi.eis_employer, pi.pcb_amount, pi.zakat_amount,
            pi.ptptn_amount, pi.tabung_haji_amount, pi.total_loan_deductions,
            pi.total_other_deductions, pi.unpaid_leave_deduction,
            pi.total_deductions, pi.net_salary, pi.employer_cost,
            pi.ytd_gross, pi.ytd_epf_employee, pi.ytd_pcb,
            pi.ytd_socso_employee, pi.ytd_eis_employee, pi.ytd_zakat, pi.ytd_net
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        JOIN employees e ON pi.employee_id = e.id
        WHERE pi.id = $1 AND pi.employee_id = $2"#,
    )
    .bind(payslip_id)
    .bind(employee_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Payslip not found".into()))?;

    let company = sqlx::query_as::<_, CompanyInfo>(
        r#"SELECT name, registration_number, address_line1, address_line2, city, state, postcode
        FROM companies WHERE id = (SELECT company_id FROM employees WHERE id = $1)"#,
    )
    .bind(employee_id)
    .fetch_one(pool)
    .await?;

    render_payslip_page(&data, &company)
}

fn render_payslip_page(data: &PayslipData, company: &CompanyInfo) -> AppResult<Vec<u8>> {
    let font = PdfFontHandle::Builtin(BuiltinFont::Helvetica);
    let bold = PdfFontHandle::Builtin(BuiltinFont::HelveticaBold);

    let mut ops: Vec<Op> = Vec::new();
    let left = 20.0;
    let right = 190.0;
    let mut y = 275.0;

    let months = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let month_name = months.get((data.period_month - 1) as usize).unwrap_or(&"");

    // Company header
    add_text(&mut ops, &bold, 14.0, left, y, &company.name);
    y -= 5.0;
    if let Some(ref reg) = company.registration_number {
        add_text(&mut ops, &font, 8.0, left, y, &format!("Reg No: {}", reg));
        y -= 4.0;
    }
    let mut addr_parts = vec![];
    if let Some(ref a) = company.address_line1 {
        addr_parts.push(a.clone());
    }
    if let Some(ref a) = company.address_line2 {
        addr_parts.push(a.clone());
    }
    let mut city_parts = vec![];
    if let Some(ref p) = company.postcode {
        city_parts.push(p.clone());
    }
    if let Some(ref c) = company.city {
        city_parts.push(c.clone());
    }
    if let Some(ref s) = company.state {
        city_parts.push(s.clone());
    }
    if !city_parts.is_empty() {
        addr_parts.push(city_parts.join(", "));
    }
    if !addr_parts.is_empty() {
        add_text(&mut ops, &font, 8.0, left, y, &addr_parts.join(", "));
        y -= 4.0;
    }

    y -= 4.0;
    draw_line(&mut ops, left, right, y);
    y -= 6.0;

    // Payslip title
    add_text(
        &mut ops,
        &bold,
        12.0,
        left,
        y,
        &format!("PAYSLIP - {} {}", month_name, data.period_year),
    );
    y -= 8.0;

    // Employee info
    add_text(
        &mut ops,
        &font,
        9.0,
        left,
        y,
        &format!("Employee: {}", data.employee_name),
    );
    add_text(
        &mut ops,
        &font,
        9.0,
        110.0,
        y,
        &format!("Employee No: {}", data.employee_number),
    );
    y -= 5.0;
    if let Some(ref ic) = data.ic_number {
        add_text(&mut ops, &font, 9.0, left, y, &format!("IC Number: {}", ic));
    }
    if let Some(ref dept) = data.department {
        add_text(
            &mut ops,
            &font,
            9.0,
            110.0,
            y,
            &format!("Department: {}", dept),
        );
    }
    y -= 5.0;
    if let Some(ref desig) = data.designation {
        add_text(
            &mut ops,
            &font,
            9.0,
            left,
            y,
            &format!("Designation: {}", desig),
        );
    }
    add_text(
        &mut ops,
        &font,
        9.0,
        110.0,
        y,
        &format!("Pay Date: {}", data.pay_date),
    );
    y -= 5.0;
    add_text(
        &mut ops,
        &font,
        9.0,
        left,
        y,
        &format!("Pay Period: {} to {}", data.period_start, data.period_end),
    );
    y -= 4.0;

    y -= 4.0;
    draw_line(&mut ops, left, right, y);
    y -= 8.0;

    // Earnings
    add_text(&mut ops, &bold, 10.0, left, y, "EARNINGS");
    add_text_right(&mut ops, &bold, 10.0, right, y, "RM");
    y -= 6.0;

    let earnings: Vec<(&str, i64)> = vec![
        ("Basic Salary", data.basic_salary),
        ("Allowances", data.total_allowances),
        ("Overtime", data.total_overtime),
        ("Bonus", data.total_bonus),
        ("Commission", data.total_commission),
    ];
    for (label, amount) in &earnings {
        if *amount > 0 {
            draw_row(
                &mut ops,
                &font,
                &bold,
                9.0,
                left + 5.0,
                right,
                y,
                label,
                &sen_to_rm(*amount),
                false,
            );
            y -= 5.0;
        }
    }
    draw_line(&mut ops, left, right, y + 1.0);
    y -= 5.0;
    draw_row(
        &mut ops,
        &font,
        &bold,
        9.0,
        left,
        right,
        y,
        "TOTAL EARNINGS",
        &sen_to_rm(data.gross_salary),
        true,
    );
    y -= 8.0;

    // Payments (claims)
    if data.total_claims > 0 {
        add_text(&mut ops, &bold, 10.0, left, y, "PAYMENTS");
        y -= 6.0;
        draw_row(
            &mut ops,
            &font,
            &bold,
            9.0,
            left + 5.0,
            right,
            y,
            "Reimbursements",
            &sen_to_rm(data.total_claims),
            false,
        );
        y -= 5.0;
        draw_line(&mut ops, left, right, y + 1.0);
        y -= 5.0;
        draw_row(
            &mut ops,
            &font,
            &bold,
            9.0,
            left,
            right,
            y,
            "TOTAL PAYMENTS",
            &sen_to_rm(data.total_claims),
            true,
        );
        y -= 8.0;
    }

    // Deductions
    add_text(&mut ops, &bold, 10.0, left, y, "DEDUCTIONS");
    y -= 6.0;
    let deductions: Vec<(&str, i64)> = vec![
        ("EPF (Employee)", data.epf_employee),
        ("SOCSO (Employee)", data.socso_employee),
        ("EIS (Employee)", data.eis_employee),
        ("PCB / Income Tax", data.pcb_amount),
        ("Zakat", data.zakat_amount),
        ("PTPTN", data.ptptn_amount),
        ("Tabung Haji", data.tabung_haji_amount),
        ("Loan Deductions", data.total_loan_deductions),
        ("Other Deductions", data.total_other_deductions),
        ("Unpaid Leave", data.unpaid_leave_deduction),
    ];
    for (label, amount) in &deductions {
        if *amount > 0 {
            draw_row(
                &mut ops,
                &font,
                &bold,
                9.0,
                left + 5.0,
                right,
                y,
                label,
                &format!("-{}", sen_to_rm(*amount)),
                false,
            );
            y -= 5.0;
        }
    }
    draw_line(&mut ops, left, right, y + 1.0);
    y -= 5.0;
    draw_row(
        &mut ops,
        &font,
        &bold,
        9.0,
        left,
        right,
        y,
        "TOTAL DEDUCTIONS",
        &format!("-{}", sen_to_rm(data.total_deductions)),
        true,
    );
    y -= 10.0;

    // Net Salary
    draw_line(&mut ops, left, right, y + 2.0);
    y -= 2.0;
    draw_row(
        &mut ops,
        &font,
        &bold,
        11.0,
        left,
        right,
        y,
        "NET SALARY",
        &format!("RM {}", sen_to_rm(data.net_salary)),
        true,
    );
    y -= 4.0;
    draw_line(&mut ops, left, right, y);
    y -= 10.0;

    // Bank info
    if let (Some(bank), Some(acc)) = (&data.bank_name, &data.bank_account_number) {
        add_text(
            &mut ops,
            &font,
            8.0,
            left,
            y,
            &format!("Paid by bank transfer: {} (Acc: {})", bank, acc),
        );
        y -= 8.0;
    }

    // Employer Contributions
    add_text(&mut ops, &bold, 10.0, left, y, "EMPLOYER CONTRIBUTIONS");
    y -= 6.0;
    draw_row(
        &mut ops,
        &font,
        &bold,
        9.0,
        left + 5.0,
        right,
        y,
        "EPF (Employer)",
        &sen_to_rm(data.epf_employer),
        false,
    );
    y -= 5.0;
    draw_row(
        &mut ops,
        &font,
        &bold,
        9.0,
        left + 5.0,
        right,
        y,
        "SOCSO (Employer)",
        &sen_to_rm(data.socso_employer),
        false,
    );
    y -= 5.0;
    draw_row(
        &mut ops,
        &font,
        &bold,
        9.0,
        left + 5.0,
        right,
        y,
        "EIS (Employer)",
        &sen_to_rm(data.eis_employer),
        false,
    );
    y -= 5.0;
    draw_line(&mut ops, left, right, y + 1.0);
    y -= 5.0;
    draw_row(
        &mut ops,
        &font,
        &bold,
        9.0,
        left,
        right,
        y,
        "TOTAL EMPLOYER COST",
        &sen_to_rm(data.employer_cost),
        true,
    );
    y -= 10.0;

    // YTD Summary
    add_text(
        &mut ops,
        &bold,
        10.0,
        left,
        y,
        &format!("YEAR-TO-DATE (Jan - Dec {})", data.period_year),
    );
    y -= 6.0;
    let ytd_items: Vec<(&str, i64)> = vec![
        ("YTD Gross", data.ytd_gross),
        ("YTD EPF (Employee)", data.ytd_epf_employee),
        ("YTD SOCSO (Employee)", data.ytd_socso_employee),
        ("YTD EIS (Employee)", data.ytd_eis_employee),
        ("YTD PCB", data.ytd_pcb),
        ("YTD Zakat", data.ytd_zakat),
        ("YTD Net", data.ytd_net),
    ];
    for (label, amount) in &ytd_items {
        if *amount > 0 {
            draw_row(
                &mut ops,
                &font,
                &bold,
                9.0,
                left + 5.0,
                right,
                y,
                label,
                &sen_to_rm(*amount),
                false,
            );
            y -= 5.0;
        }
    }

    // Footer
    add_text(
        &mut ops,
        &font,
        7.0,
        left,
        15.0,
        "This is a computer-generated payslip. No signature is required.",
    );

    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    let mut doc = PdfDocument::new("Payslip");
    doc.pages.push(page);

    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    Ok(bytes)
}

pub async fn generate_bulk_payslips(
    pool: &PgPool,
    payroll_run_id: Uuid,
    company_id: Uuid,
) -> AppResult<Vec<u8>> {
    let items: Vec<(Uuid, Uuid)> = sqlx::query_as(
        r#"SELECT pi.id, pi.employee_id
        FROM payroll_items pi
        JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
        WHERE pr.id = $1 AND pr.company_id = $2
        ORDER BY (SELECT employee_number FROM employees WHERE id = pi.employee_id)"#,
    )
    .bind(payroll_run_id)
    .bind(company_id)
    .fetch_all(pool)
    .await?;

    if items.is_empty() {
        return Err(AppError::NotFound("No payroll items found".into()));
    }

    // Generate individual PDFs and merge using append_document
    let mut main_doc = PdfDocument::new("Payslips");

    for (item_id, emp_id) in &items {
        let data = sqlx::query_as::<_, PayslipData>(
            r#"SELECT
                e.full_name as employee_name, e.employee_number, e.ic_number,
                e.department, e.designation, e.bank_name, e.bank_account_number,
                pr.period_year, pr.period_month, pr.period_start, pr.period_end, pr.pay_date,
                pi.basic_salary, pi.gross_salary, pi.total_allowances, pi.total_overtime,
                pi.total_bonus, pi.total_commission, pi.total_claims,
                pi.epf_employee, pi.epf_employer, pi.socso_employee, pi.socso_employer,
                pi.eis_employee, pi.eis_employer, pi.pcb_amount, pi.zakat_amount,
                pi.ptptn_amount, pi.tabung_haji_amount, pi.total_loan_deductions,
                pi.total_other_deductions, pi.unpaid_leave_deduction,
                pi.total_deductions, pi.net_salary, pi.employer_cost,
                pi.ytd_gross, pi.ytd_epf_employee, pi.ytd_pcb,
                pi.ytd_socso_employee, pi.ytd_eis_employee, pi.ytd_zakat, pi.ytd_net
            FROM payroll_items pi
            JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
            JOIN employees e ON pi.employee_id = e.id
            WHERE pi.id = $1 AND pi.employee_id = $2"#,
        )
        .bind(item_id)
        .bind(emp_id)
        .fetch_optional(pool)
        .await?;

        let company = sqlx::query_as::<_, CompanyInfo>(
            r#"SELECT name, registration_number, address_line1, address_line2, city, state, postcode
            FROM companies WHERE id = (SELECT company_id FROM employees WHERE id = $1)"#,
        )
        .bind(emp_id)
        .fetch_one(pool)
        .await?;

        if let Some(slip) = data {
            let font = PdfFontHandle::Builtin(BuiltinFont::Helvetica);
            let bold = PdfFontHandle::Builtin(BuiltinFont::HelveticaBold);
            let page = render_payslip_ops(&slip, &company, &font, &bold);
            main_doc.pages.push(page);
        }
    }

    if main_doc.pages.is_empty() {
        return Err(AppError::NotFound("No payslips generated".into()));
    }

    let mut warnings = Vec::new();
    let bytes = main_doc.save(&PdfSaveOptions::default(), &mut warnings);
    Ok(bytes)
}

fn render_payslip_ops(
    data: &PayslipData,
    company: &CompanyInfo,
    font: &PdfFontHandle,
    bold: &PdfFontHandle,
) -> PdfPage {
    let mut ops: Vec<Op> = Vec::new();
    let left = 20.0;
    let right = 190.0;
    let mut y = 275.0;

    let months = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let month_name = months.get((data.period_month - 1) as usize).unwrap_or(&"");

    add_text(&mut ops, bold, 14.0, left, y, &company.name);
    y -= 5.0;
    if let Some(ref reg) = company.registration_number {
        add_text(&mut ops, font, 8.0, left, y, &format!("Reg No: {}", reg));
        y -= 4.0;
    }
    let mut addr_parts = vec![];
    if let Some(ref a) = company.address_line1 {
        addr_parts.push(a.clone());
    }
    if let Some(ref a) = company.address_line2 {
        addr_parts.push(a.clone());
    }
    let mut city_parts = vec![];
    if let Some(ref p) = company.postcode {
        city_parts.push(p.clone());
    }
    if let Some(ref c) = company.city {
        city_parts.push(c.clone());
    }
    if let Some(ref s) = company.state {
        city_parts.push(s.clone());
    }
    if !city_parts.is_empty() {
        addr_parts.push(city_parts.join(", "));
    }
    if !addr_parts.is_empty() {
        add_text(&mut ops, font, 8.0, left, y, &addr_parts.join(", "));
        y -= 4.0;
    }

    y -= 4.0;
    draw_line(&mut ops, left, right, y);
    y -= 6.0;

    add_text(
        &mut ops,
        bold,
        12.0,
        left,
        y,
        &format!("PAYSLIP - {} {}", month_name, data.period_year),
    );
    y -= 8.0;

    add_text(
        &mut ops,
        font,
        9.0,
        left,
        y,
        &format!("Employee: {}", data.employee_name),
    );
    add_text(
        &mut ops,
        font,
        9.0,
        110.0,
        y,
        &format!("Employee No: {}", data.employee_number),
    );
    y -= 5.0;
    if let Some(ref ic) = data.ic_number {
        add_text(&mut ops, font, 9.0, left, y, &format!("IC Number: {}", ic));
    }
    if let Some(ref dept) = data.department {
        add_text(
            &mut ops,
            font,
            9.0,
            110.0,
            y,
            &format!("Department: {}", dept),
        );
    }
    y -= 5.0;
    if let Some(ref desig) = data.designation {
        add_text(
            &mut ops,
            font,
            9.0,
            left,
            y,
            &format!("Designation: {}", desig),
        );
    }
    add_text(
        &mut ops,
        font,
        9.0,
        110.0,
        y,
        &format!("Pay Date: {}", data.pay_date),
    );
    y -= 5.0;
    add_text(
        &mut ops,
        font,
        9.0,
        left,
        y,
        &format!("Pay Period: {} to {}", data.period_start, data.period_end),
    );
    y -= 8.0;
    draw_line(&mut ops, left, right, y);
    y -= 8.0;

    add_text(&mut ops, bold, 10.0, left, y, "EARNINGS");
    add_text_right(&mut ops, bold, 10.0, right, y, "RM");
    y -= 6.0;

    for (label, amount) in &[
        ("Basic Salary", data.basic_salary),
        ("Allowances", data.total_allowances),
        ("Overtime", data.total_overtime),
        ("Bonus", data.total_bonus),
        ("Commission", data.total_commission),
    ] {
        if *amount > 0 {
            draw_row(
                &mut ops,
                font,
                bold,
                9.0,
                left + 5.0,
                right,
                y,
                label,
                &sen_to_rm(*amount),
                false,
            );
            y -= 5.0;
        }
    }
    draw_line(&mut ops, left, right, y + 1.0);
    y -= 5.0;
    draw_row(
        &mut ops,
        font,
        bold,
        9.0,
        left,
        right,
        y,
        "TOTAL EARNINGS",
        &sen_to_rm(data.gross_salary),
        true,
    );
    y -= 8.0;

    if data.total_claims > 0 {
        add_text(&mut ops, bold, 10.0, left, y, "PAYMENTS");
        y -= 6.0;
        draw_row(
            &mut ops,
            font,
            bold,
            9.0,
            left + 5.0,
            right,
            y,
            "Reimbursements",
            &sen_to_rm(data.total_claims),
            false,
        );
        y -= 5.0;
        draw_line(&mut ops, left, right, y + 1.0);
        y -= 5.0;
        draw_row(
            &mut ops,
            font,
            bold,
            9.0,
            left,
            right,
            y,
            "TOTAL PAYMENTS",
            &sen_to_rm(data.total_claims),
            true,
        );
        y -= 8.0;
    }

    add_text(&mut ops, bold, 10.0, left, y, "DEDUCTIONS");
    y -= 6.0;
    for (label, amount) in &[
        ("EPF (Employee)", data.epf_employee),
        ("SOCSO (Employee)", data.socso_employee),
        ("EIS (Employee)", data.eis_employee),
        ("PCB / Income Tax", data.pcb_amount),
        ("Zakat", data.zakat_amount),
        ("PTPTN", data.ptptn_amount),
        ("Tabung Haji", data.tabung_haji_amount),
        ("Loan Deductions", data.total_loan_deductions),
        ("Other Deductions", data.total_other_deductions),
        ("Unpaid Leave", data.unpaid_leave_deduction),
    ] {
        if *amount > 0 {
            draw_row(
                &mut ops,
                font,
                bold,
                9.0,
                left + 5.0,
                right,
                y,
                label,
                &format!("-{}", sen_to_rm(*amount)),
                false,
            );
            y -= 5.0;
        }
    }
    draw_line(&mut ops, left, right, y + 1.0);
    y -= 5.0;
    draw_row(
        &mut ops,
        font,
        bold,
        9.0,
        left,
        right,
        y,
        "TOTAL DEDUCTIONS",
        &format!("-{}", sen_to_rm(data.total_deductions)),
        true,
    );
    y -= 10.0;

    draw_line(&mut ops, left, right, y + 2.0);
    y -= 2.0;
    draw_row(
        &mut ops,
        font,
        bold,
        11.0,
        left,
        right,
        y,
        "NET SALARY",
        &format!("RM {}", sen_to_rm(data.net_salary)),
        true,
    );
    y -= 4.0;
    draw_line(&mut ops, left, right, y);
    y -= 10.0;

    if let (Some(bank), Some(acc)) = (&data.bank_name, &data.bank_account_number) {
        add_text(
            &mut ops,
            font,
            8.0,
            left,
            y,
            &format!("Paid by bank transfer: {} (Acc: {})", bank, acc),
        );
        y -= 8.0;
    }

    add_text(&mut ops, bold, 10.0, left, y, "EMPLOYER CONTRIBUTIONS");
    y -= 6.0;
    draw_row(
        &mut ops,
        font,
        bold,
        9.0,
        left + 5.0,
        right,
        y,
        "EPF (Employer)",
        &sen_to_rm(data.epf_employer),
        false,
    );
    y -= 5.0;
    draw_row(
        &mut ops,
        font,
        bold,
        9.0,
        left + 5.0,
        right,
        y,
        "SOCSO (Employer)",
        &sen_to_rm(data.socso_employer),
        false,
    );
    y -= 5.0;
    draw_row(
        &mut ops,
        font,
        bold,
        9.0,
        left + 5.0,
        right,
        y,
        "EIS (Employer)",
        &sen_to_rm(data.eis_employer),
        false,
    );
    y -= 5.0;
    draw_line(&mut ops, left, right, y + 1.0);
    y -= 5.0;
    draw_row(
        &mut ops,
        font,
        bold,
        9.0,
        left,
        right,
        y,
        "TOTAL EMPLOYER COST",
        &sen_to_rm(data.employer_cost),
        true,
    );
    y -= 10.0;

    add_text(
        &mut ops,
        bold,
        10.0,
        left,
        y,
        &format!("YEAR-TO-DATE (Jan - Dec {})", data.period_year),
    );
    y -= 6.0;
    for (label, amount) in &[
        ("YTD Gross", data.ytd_gross),
        ("YTD EPF (Employee)", data.ytd_epf_employee),
        ("YTD SOCSO (Employee)", data.ytd_socso_employee),
        ("YTD EIS (Employee)", data.ytd_eis_employee),
        ("YTD PCB", data.ytd_pcb),
        ("YTD Zakat", data.ytd_zakat),
        ("YTD Net", data.ytd_net),
    ] {
        if *amount > 0 {
            draw_row(
                &mut ops,
                font,
                bold,
                9.0,
                left + 5.0,
                right,
                y,
                label,
                &sen_to_rm(*amount),
                false,
            );
            y -= 5.0;
        }
    }

    add_text(
        &mut ops,
        font,
        7.0,
        left,
        15.0,
        "This is a computer-generated payslip. No signature is required.",
    );

    PdfPage::new(Mm(210.0), Mm(297.0), ops)
}
