//! Golden & integration tests for statutory exports, EA forms, payslip PDFs,
//! and admin report aggregations (plan item 46).

use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::{
    ea_form_service, payslip_pdf_service, report_service, statutory_export_service,
};
use crate::tests::support::{seed_company, seed_payroll_group, seed_user, skip_if_no_db};

struct SeededPayrollReportFixture {
    company_id: Uuid,
    company_name: String,
    run_id: Uuid,
    emp_a: Uuid,
    item_a: Uuid,
}

async fn seed_fixture(pool: &PgPool) -> SeededPayrollReportFixture {
    let company_id = seed_company(pool).await;
    let group_id = seed_payroll_group(pool, company_id).await;
    let reviewer_id = seed_user(pool, company_id, "admin").await;

    let company_name: String = sqlx::query_scalar(
        "UPDATE companies
         SET registration_number = '202001000123',
             tax_number = 'E1234567890',
             epf_number = 'EPF-CO-001',
             socso_code = 'SOCSO-CO-001',
             eis_code = 'EIS-CO-001',
             address_line1 = 'Level 10, Menara Golden',
             city = 'Kuala Lumpur',
             postcode = '50000',
             state = 'WP Kuala Lumpur'
         WHERE id = $1
         RETURNING name",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await
    .expect("update company statutory details");

    let emp_a = Uuid::new_v4();
    let emp_b = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO employees (
               id, company_id, payroll_group_id, employee_number, full_name,
               ic_number, department, date_joined, date_of_birth,
               basic_salary, epf_number, socso_number, eis_number,
               tax_identification_number, epf_category
           ) VALUES
               ($1, $3, $4, 'EMP-A', 'Aisyah Ahmad',
                '900101105511', 'Engineering', '2020-01-01', '1990-01-01',
                500000, 'EPF-A', 'SOCSO-A', 'EIS-A', 'SG111111', 'A'),
               ($2, $3, $4, 'EMP-B', 'Bala Subramaniam',
                '910202106622', 'Finance', '2021-03-01', '1991-02-02',
                400000, 'EPF-B', 'SOCSO-B', 'EIS-B', 'SG222222', 'A')"#,
    )
    .bind(emp_a)
    .bind(emp_b)
    .bind(company_id)
    .bind(group_id)
    .execute(pool)
    .await
    .expect("insert two employees");

    let run_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO payroll_runs (
               id, company_id, payroll_group_id, period_year, period_month,
               period_start, period_end, pay_date, status, employee_count,
               total_gross, total_net,
               total_epf_employee, total_epf_employer,
               total_socso_employee, total_socso_employer,
               total_eis_employee, total_eis_employer,
               total_pcb, total_zakat, total_employer_cost,
               processed_by, approved_by
           ) VALUES (
               $1, $2, $3, 2025, 4,
               '2025-04-01', '2025-04-30', '2025-04-30', 'approved', 2,
               900000, 775000,
               99000, 108000,
               3500, 12000,
               1400, 1400,
               20100, 1000, 1021400,
               $4, $4
           )"#,
    )
    .bind(run_id)
    .bind(company_id)
    .bind(group_id)
    .bind(reviewer_id)
    .execute(pool)
    .await
    .expect("insert approved payroll run");

    let item_a = Uuid::new_v4();
    let item_b = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO payroll_items (
               id, payroll_run_id, employee_id,
               basic_salary, gross_salary, total_allowances, total_overtime,
               total_bonus, total_commission, total_claims,
               epf_employee, epf_employer, socso_employee, socso_employer,
               eis_employee, eis_employer, pcb_amount, zakat_amount,
               total_deductions, net_salary, employer_cost
           ) VALUES
               ($1, $3, $5, 500000, 500000, 0, 0, 0, 0, 0, 55000, 60000, 2000, 7000, 800, 800, 15000, 1000, 73800, 426200, 567800),
               ($2, $3, $6, 400000, 400000, 0, 0, 0, 0, 0, 44000, 48000, 1500, 5000, 600, 600, 5100, 0, 51200, 348800, 453600)"#,
    )
    .bind(item_a)
    .bind(item_b)
    .bind(run_id)
    .bind(company_id)
    .bind(emp_a)
    .bind(emp_b)
    .execute(pool)
    .await
    .expect("insert payroll items");

    sqlx::query(
        r#"INSERT INTO payroll_item_details (payroll_item_id, category, item_type, description, amount, is_taxable)
           VALUES
               ($1, 'earning', 'basic', 'Basic Salary', 500000, TRUE),
               ($1, 'deduction', 'epf_employee', 'EPF Employee', 55000, FALSE),
               ($1, 'deduction', 'socso_employee', 'SOCSO Employee', 2000, FALSE),
               ($1, 'deduction', 'eis_employee', 'EIS Employee', 800, FALSE),
               ($1, 'deduction', 'pcb', 'PCB / MTD Tax', 15000, FALSE),
               ($1, 'deduction', 'zakat', 'Zakat', 1000, FALSE)"#,
    )
    .bind(item_a)
    .execute(pool)
    .await
    .expect("insert payroll item details");

    // Seed leave type + balance + claim for report tests.
    let leave_type_id: Uuid = sqlx::query_scalar(
        "INSERT INTO leave_types (company_id, name, default_days)
         VALUES ($1, 'Annual Leave', 14) RETURNING id",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await
    .expect("insert leave type");

    sqlx::query(
        "INSERT INTO leave_balances (employee_id, leave_type_id, year, entitled_days, taken_days, pending_days)
         VALUES ($1, $2, 2025, 14.0, 2.0, 1.0)",
    )
    .bind(emp_a)
    .bind(leave_type_id)
    .execute(pool)
    .await
    .expect("insert leave balance");

    sqlx::query(
        "INSERT INTO claims (employee_id, company_id, title, amount, expense_date, status)
         VALUES ($1, $2, 'Flight Ticket', 125000, '2025-04-10', 'approved')",
    )
    .bind(emp_a)
    .bind(company_id)
    .execute(pool)
    .await
    .expect("insert approved claim");

    SeededPayrollReportFixture {
        company_id,
        company_name,
        run_id,
        emp_a,
        item_a,
    }
}

#[tokio::test]
async fn statutory_exports_render_expected_headers_and_amounts() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let fx = seed_fixture(&pool).await;

    let epf_csv = String::from_utf8(
        statutory_export_service::export_epf(&pool, fx.company_id, 2025, 4)
            .await
            .expect("export epf"),
    )
    .expect("utf8 epf");
    assert!(epf_csv.starts_with("Employer EPF No,Employee EPF No,IC Number,Employee Name,Wages (RM),Employee Contribution (RM),Employer Contribution (RM)"));
    assert!(epf_csv.contains("EPF-CO-001,EPF-A,900101105511,Aisyah Ahmad,5000.00,550.00,600.00"));
    assert!(
        epf_csv.contains("EPF-CO-001,EPF-B,910202106622,Bala Subramaniam,4000.00,440.00,480.00")
    );

    let socso_csv = String::from_utf8(
        statutory_export_service::export_socso(&pool, fx.company_id, 2025, 4)
            .await
            .expect("export socso"),
    )
    .expect("utf8 socso");
    assert!(
        socso_csv.contains("SOCSO-CO-001,SOCSO-A,900101105511,Aisyah Ahmad,5000.00,20.00,70.00")
    );

    let eis_csv = String::from_utf8(
        statutory_export_service::export_eis(&pool, fx.company_id, 2025, 4)
            .await
            .expect("export eis"),
    )
    .expect("utf8 eis");
    assert!(eis_csv.contains("EIS-CO-001,EIS-A,900101105511,Aisyah Ahmad,5000.00,8.00,8.00"));

    let cp39 = String::from_utf8(
        statutory_export_service::export_pcb_cp39(&pool, fx.company_id, 2025, 4)
            .await
            .expect("export cp39"),
    )
    .expect("utf8 cp39");
    assert!(cp39.starts_with(&format!("H|E1234567890|042025|{}\n", fx.company_name)));
    assert!(cp39.contains("D|SG111111|900101105511|Aisyah Ahmad|150.00|0.00\n"));
    assert!(cp39.contains("D|SG222222|910202106622|Bala Subramaniam|51.00|0.00\n"));
}

#[tokio::test]
async fn ea_form_and_payslip_pdfs_render_valid_documents() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let fx = seed_fixture(&pool).await;

    let summaries = ea_form_service::list_employees_for_ea(&pool, fx.company_id, 2025)
        .await
        .expect("list EA employees");
    assert_eq!(summaries.len(), 2);

    let ea_data = ea_form_service::get_ea_form_data(&pool, fx.company_id, fx.emp_a, 2025)
        .await
        .expect("get EA data");
    assert_eq!(ea_data.ytd_basic, 500_000);
    assert_eq!(ea_data.ytd_epf_employee, 55_000);
    assert_eq!(ea_data.ytd_pcb, 15_000);
    assert_eq!(ea_data.months_worked, 1);

    let ea_pdf = ea_form_service::generate_ea_form_pdf(&ea_data).expect("render EA PDF");
    assert!(ea_pdf.starts_with(b"%PDF-"), "EA output must be valid PDF");
    assert!(ea_pdf.windows(5).any(|w| w == b"%%EOF"));

    let single_pdf = payslip_pdf_service::generate_payslip_pdf(&pool, fx.item_a, fx.emp_a)
        .await
        .expect("render single payslip PDF");
    assert!(single_pdf.starts_with(b"%PDF-"));
    assert!(single_pdf.windows(5).any(|w| w == b"%%EOF"));

    let bulk_pdf = payslip_pdf_service::generate_bulk_payslips(&pool, fx.run_id, fx.company_id)
        .await
        .expect("render bulk payslips PDF");
    assert!(bulk_pdf.starts_with(b"%PDF-"));
    assert!(bulk_pdf.len() > single_pdf.len());
}

#[tokio::test]
async fn report_service_aggregates_accurately_and_isolates_tenants() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let fx = seed_fixture(&pool).await;
    let other_company = seed_company(&pool).await;

    let summary = report_service::payroll_summary(&pool, fx.company_id, 2025)
        .await
        .expect("payroll summary");
    assert_eq!(summary.len(), 1);
    assert_eq!(summary[0].period, "2025-04");
    assert_eq!(summary[0].total_gross, 900_000);
    assert_eq!(summary[0].total_pcb, 20_100);

    let depts = report_service::payroll_by_department(&pool, fx.company_id, 2025, 4)
        .await
        .expect("dept report");
    assert_eq!(depts.len(), 2);

    let leave = report_service::leave_report(&pool, fx.company_id, 2025)
        .await
        .expect("leave report");
    assert_eq!(leave.len(), 1);
    assert_eq!(leave[0].employee_number, "EMP-A");

    let claims = report_service::claims_report(
        &pool,
        fx.company_id,
        NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
        NaiveDate::from_ymd_opt(2025, 4, 30).unwrap(),
    )
    .await
    .expect("claims report");
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].approved_amount, 125_000);

    let stat = report_service::statutory_report(&pool, fx.company_id, 2025, 4)
        .await
        .expect("statutory report");
    assert_eq!(stat.len(), 2);

    let periods = report_service::report_periods(&pool, fx.company_id)
        .await
        .expect("report periods");
    assert_eq!(periods.default_year, 2025);
    assert_eq!(periods.default_month, 4);

    // Second tenant sees zero rows across all reports.
    assert!(
        report_service::payroll_summary(&pool, other_company, 2025)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        report_service::statutory_report(&pool, other_company, 2025, 4)
            .await
            .unwrap()
            .is_empty()
    );
}
