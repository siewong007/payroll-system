use rust_xlsxwriter::{Format, Workbook};

use crate::core::error::{AppError, AppResult};

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
