use chrono::NaiveDate;

use crate::services::pdf_helpers::unclassified_earnings;
use crate::services::{payroll_engine, payroll_service};
use crate::tests::support::{
    seed_company, seed_employee, seed_payroll_group, seed_user, skip_if_no_db,
};

/// End-to-end test: seed one employee on RM5,000 basic, process payroll for
/// January 2024, verify the resulting `PayrollRun` + `PayrollItem` match the
/// values derived from the prototype statutory tables in `1001_data.sql`.
#[tokio::test]
async fn process_payroll_single_employee_rm5000() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        1,
        NaiveDate::from_ymd_opt(2024, 2, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll should succeed");

    assert_eq!(run.period_year, 2024);
    assert_eq!(run.period_month, 1);
    assert_eq!(run.employee_count, 1);
    assert_eq!(run.status, "processed");

    // One payroll_item row for the one employee.
    let (
        basic,
        gross,
        net,
        epf_ee,
        epf_er,
        socso_ee,
        socso_er,
        eis_ee,
        eis_er,
        pcb,
        total_deductions,
        employer_cost,
    ): (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"SELECT basic_salary, gross_salary, net_salary,
                  epf_employee, epf_employer,
                  socso_employee, socso_employer,
                  eis_employee, eis_employer,
                  pcb_amount, total_deductions, employer_cost
           FROM payroll_items
           WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("item should exist");

    // Basic/gross come straight from the employee row (no allowances, no OT).
    assert_eq!(basic, 500_000);
    assert_eq!(gross, 500_000);

    // Prototype reference values: wage 500_000 hits the top of each bracket.
    assert_eq!(
        epf_ee, 53_000,
        "EPF employee from seed bracket (480001, 500000)"
    );
    assert_eq!(epf_er, 58_000);
    assert_eq!(socso_ee, 1_825, "SOCSO employee (490001, 500000) first cat");
    assert_eq!(socso_er, 3_335);
    assert_eq!(eis_ee, 990, "EIS employee at ceiling");
    assert_eq!(eis_er, 990);

    // Accounting identities must hold.
    assert!(pcb >= 0, "PCB must not be negative");
    assert_eq!(
        total_deductions,
        epf_ee + socso_ee + eis_ee + pcb,
        "total_deductions = statutory contributions + PCB (no zakat/ptptn/haji/custom)"
    );
    assert_eq!(
        net,
        gross - total_deductions,
        "net = gross - deductions (no reimbursable claims in this scenario)"
    );
    assert_eq!(employer_cost, gross + epf_er + socso_er + eis_er);

    // Run totals equal item totals (one employee).
    assert_eq!(run.total_gross, gross);
    assert_eq!(run.total_net, net);
    assert_eq!(run.total_epf_employee, epf_ee);
    assert_eq!(run.total_socso_employee, socso_ee);
    assert_eq!(run.total_eis_employee, eis_ee);
    assert_eq!(run.total_pcb, pcb);
    assert_eq!(run.total_employer_cost, employer_cost);
}

/// The engine rejects a second run for the same (company, group, period)
/// unless the prior run was removed or is a retained cancelled legacy row. Protects against accidentally running
/// payroll twice.
#[tokio::test]
async fn process_payroll_rejects_duplicate_period() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let _emp = seed_employee(&pool, company_id, Some(group_id), 300_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let first = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        2,
        NaiveDate::from_ymd_opt(2024, 3, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await;
    assert!(first.is_ok(), "first run should succeed: {first:?}");

    let second = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        2,
        NaiveDate::from_ymd_opt(2024, 3, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await;

    let err = second.expect_err("second run for same period must fail");
    assert!(
        format!("{err:?}").contains("already exists"),
        "expected Conflict, got: {err:?}"
    );
}

/// Empty payroll group → BadRequest. Protects the "no employees found" branch.
#[tokio::test]
async fn process_payroll_rejects_empty_group() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let err = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        3,
        NaiveDate::from_ymd_opt(2024, 4, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect_err("empty group should fail");

    assert!(
        format!("{err:?}").contains("No active employees"),
        "expected 'No active employees' error, got: {err:?}"
    );
}

/// An employee who joins mid-period is paid only for the days they were employed.
/// Employment Act 1955 s.18B prorates an incomplete month by CALENDAR days:
/// monthly wages ÷ days in the month × days eligible.
#[tokio::test]
async fn mid_period_joiner_basic_salary_is_prorated() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    // Joined on the last day of a 31-day month → 1 of 31 days.
    sqlx::query("UPDATE employees SET date_joined = $2 WHERE id = $1")
        .bind(employee_id)
        .bind(NaiveDate::from_ymd_opt(2024, 5, 31).unwrap())
        .execute(&pool)
        .await
        .expect("set date_joined");

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        5,
        NaiveDate::from_ymd_opt(2024, 6, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    let (basic_salary, working_days, days_worked, is_prorated): (
        i64,
        Option<i32>,
        Option<rust_decimal::Decimal>,
        Option<bool>,
    ) = sqlx::query_as(
        r#"SELECT basic_salary, working_days, days_worked, is_prorated
           FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 500_000 × 1 / 31 = 16_129.03 → 16_129 sen.
    assert_eq!(basic_salary, 16_129, "one day of a 31-day month");
    assert_eq!(working_days, Some(31), "calendar days in the period");
    assert_eq!(days_worked, Some(rust_decimal::Decimal::from(1)));
    assert_eq!(is_prorated, Some(true));
}

/// A committed payslip stores the lines that explain it, and those lines
/// reconcile to the stored totals.
///
/// `payroll_item_details` previously had no write path at all, so every payslip
/// was a set of totals with nothing behind them.
#[tokio::test]
async fn committed_payslip_stores_a_reconciling_breakdown() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    // `company_id` is not optional: migration 1009 anchors the row to its tenant
    // and this seed uses the runtime `sqlx::query`, so a missing column is a
    // not-null violation at run time rather than a compile error.
    sqlx::query(
        r#"INSERT INTO employee_allowances
              (employee_id, company_id, category, name, amount, is_taxable, is_recurring, effective_from, is_active)
           VALUES ($1, $2, 'earning', 'Travel allowance', 30_000, TRUE, TRUE, '2020-01-01', TRUE)"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("seed allowance");

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        7,
        NaiveDate::from_ymd_opt(2024, 8, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    let lines: Vec<(String, String, String, i64, Option<bool>)> = sqlx::query_as(
        r#"SELECT pid.category, pid.item_type, pid.description, pid.amount, pid.is_statutory
           FROM payroll_item_details pid
           JOIN payroll_items pi ON pi.id = pid.payroll_item_id
           WHERE pi.payroll_run_id = $1 AND pi.employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_all(&pool)
    .await
    .expect("breakdown lines");

    assert!(!lines.is_empty(), "a payslip must store its breakdown");

    let (gross, total_deductions): (i64, i64) = sqlx::query_as(
        r#"SELECT gross_salary, total_deductions FROM payroll_items
           WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Claims are reimbursements paid on top of net, so they are the one earning
    // line deliberately outside gross.
    let earnings: i64 = lines
        .iter()
        .filter(|(category, item_type, ..)| {
            category == "earning" && item_type != "claim_reimbursement"
        })
        .map(|(.., amount, _)| amount)
        .sum();
    let deductions: i64 = lines
        .iter()
        .filter(|(category, ..)| category == "deduction")
        .map(|(.., amount, _)| amount)
        .sum();

    assert_eq!(earnings, gross, "earning lines must reconcile to gross");
    assert_eq!(
        deductions, total_deductions,
        "deduction lines must reconcile to total_deductions"
    );

    assert!(
        lines.iter().any(
            |(_, item_type, description, amount, _)| item_type == "basic_salary"
                && description == "Basic salary"
                && *amount == 500_000
        ),
        "basic salary must appear as its own line: {lines:?}"
    );
    assert!(
        lines.iter().any(
            |(_, _, description, amount, _)| description == "Travel allowance" && *amount == 30_000
        ),
        "the named allowance must survive into the breakdown: {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|(_, item_type, _, _, is_statutory)| item_type == "epf"
                && *is_statutory == Some(true)),
        "EPF must be flagged as statutory: {lines:?}"
    );
    assert!(
        lines.iter().all(|(.., amount, _)| *amount != 0)
            || lines
                .iter()
                .filter(|(.., amount, _)| *amount == 0)
                .all(|(_, item_type, ..)| item_type == "basic_salary"),
        "zero-valued lines other than basic salary should be dropped: {lines:?}"
    );
}

/// A run records which verified rule sets produced it.
#[tokio::test]
async fn committed_run_records_its_calculation_provenance() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let _emp = seed_employee(&pool, company_id, Some(group_id), 400_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        8,
        NaiveDate::from_ymd_opt(2024, 9, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    let snapshot: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT calculation_snapshot FROM payroll_runs WHERE id = $1")
            .bind(run.id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let snapshot = snapshot.expect("a committed run must record its provenance");
    assert_eq!(
        snapshot["effective_date"], "2024-08-31",
        "provenance is dated at the period end"
    );

    let rule_sets = snapshot["statutory_rule_sets"]
        .as_array()
        .expect("statutory_rule_sets must be an array");
    let codes: Vec<&str> = rule_sets
        .iter()
        .filter_map(|r| r["rule_code"].as_str())
        .collect();
    for domain in ["epf", "socso", "eis", "pcb"] {
        assert!(
            codes.contains(&domain),
            "provenance must name the {domain} rule set: {codes:?}"
        );
    }
    assert!(
        rule_sets
            .iter()
            .all(|r| r["rule_set_id"].is_string() && r["dataset_key"].is_string()),
        "each rule set must be identifiable: {rule_sets:?}"
    );

    assert!(
        snapshot["overtime_settings"]["multiplier_normal"].is_string(),
        "overtime configuration is recorded alongside the rule sets"
    );
}

/// Preview computes the run without writing anything.
#[tokio::test]
async fn preview_projects_the_run_without_committing_it() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;

    let preview = payroll_engine::preview_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        9,
        NaiveDate::from_ymd_opt(2024, 10, 5).unwrap(),
    )
    .await
    .expect("preview_payroll");

    assert!(preview.can_process, "blocking: {:?}", preview.blocking);
    assert_eq!(preview.employee_count, 1);
    assert_eq!(preview.payable_count, 1);
    assert_eq!(preview.employees.len(), 1);
    assert_eq!(preview.employees[0].employee_id, employee_id);
    assert_eq!(preview.employees[0].gross_salary, 500_000);
    assert!(preview.employees[0].error.is_none());
    assert_eq!(preview.total_gross, 500_000);

    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM payroll_runs WHERE company_id = $1")
        .bind(company_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(runs, 0, "preview must not create a payroll run");

    // The committed run agrees with what the preview projected.
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;
    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        9,
        NaiveDate::from_ymd_opt(2024, 10, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    assert_eq!(run.total_gross, preview.total_gross);
    assert_eq!(run.total_net, preview.total_net);
    assert_eq!(run.total_pcb, preview.total_pcb);
    assert_eq!(run.total_employer_cost, preview.total_employer_cost);
}

/// Preview reports every uncomputable employee at once, and processing refuses
/// the whole run rather than committing the employees that happened to work.
///
/// The engine used to abort on the first failure, so an operator fixing a batch
/// of bad entries discovered them one run at a time.
#[tokio::test]
async fn preview_and_process_report_all_failing_employees_together() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let good = seed_employee(&pool, company_id, Some(group_id), 400_000).await;
    let bad_one = seed_employee(&pool, company_id, Some(group_id), 300_000).await;
    let bad_two = seed_employee(&pool, company_id, Some(group_id), 300_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    // A deduction larger than gross would produce a negative net salary.
    for employee_id in [bad_one, bad_two] {
        sqlx::query(
            r#"INSERT INTO payroll_entries
                  (employee_id, company_id, period_year, period_month, category,
                   item_type, description, amount, is_processed)
               VALUES ($1, $2, 2024, 10, 'deduction', 'manual_deduction',
                       'Overstated loan repayment', 900_000, FALSE)"#,
        )
        .bind(employee_id)
        .bind(company_id)
        .execute(&pool)
        .await
        .expect("seed deduction");
    }

    let preview = payroll_engine::preview_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        10,
        NaiveDate::from_ymd_opt(2024, 11, 5).unwrap(),
    )
    .await
    .expect("preview_payroll");

    assert!(!preview.can_process);
    assert_eq!(preview.employee_count, 3);
    assert_eq!(preview.payable_count, 1, "only the good employee computes");

    let failed: Vec<_> = preview
        .blocking
        .iter()
        .filter(|d| d.code == "employee_calculation_failed")
        .filter_map(|d| d.employee_id)
        .collect();
    assert_eq!(failed.len(), 2, "both failures reported: {failed:?}");
    assert!(failed.contains(&bad_one) && failed.contains(&bad_two));
    assert!(!failed.contains(&good));

    // Per-employee rows carry their own error so the UI can point at the row.
    let bad_row = preview
        .employees
        .iter()
        .find(|e| e.employee_id == bad_one)
        .unwrap();
    assert!(bad_row.error.is_some());

    let err = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        10,
        NaiveDate::from_ymd_opt(2024, 11, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect_err("a run with uncomputable employees must not commit");

    let message = format!("{err:?}");
    assert!(
        message.contains("2 of the selected employees"),
        "the error must name every failure, got: {message}"
    );

    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM payroll_runs WHERE company_id = $1")
        .bind(company_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(runs, 0, "nothing may be left behind by a rejected run");
}

/// Preview warns about staged entries that the selected run will not pay.
#[tokio::test]
async fn preview_warns_about_entries_staged_outside_the_run() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let _in_run = seed_employee(&pool, company_id, Some(group_id), 400_000).await;
    // No payroll group, so no run will ever pick this employee's entry up.
    let orphan = seed_employee(&pool, company_id, None, 400_000).await;

    sqlx::query(
        r#"INSERT INTO payroll_entries
              (employee_id, company_id, period_year, period_month, category,
               item_type, description, amount, is_processed)
           VALUES ($1, $2, 2024, 11, 'earning', 'manual_adjustment',
                   'Retention bonus', 250_000, FALSE)"#,
    )
    .bind(orphan)
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("seed orphan entry");

    let preview = payroll_engine::preview_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        11,
        NaiveDate::from_ymd_opt(2024, 12, 5).unwrap(),
    )
    .await
    .expect("preview_payroll");

    let warning = preview
        .warnings
        .iter()
        .find(|w| w.code == "staged_entries_not_in_run")
        .expect("an unpayable staged entry must be surfaced");

    assert_eq!(warning.employee_id, Some(orphan));
    assert!(
        preview.can_process,
        "an orphaned entry is a warning, not a blocker"
    );
}

/// A full-month employee is not prorated — guards the proration branch itself.
#[tokio::test]
async fn full_month_employee_is_not_prorated() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        5,
        NaiveDate::from_ymd_opt(2024, 6, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    let (basic_salary, is_prorated): (i64, Option<bool>) = sqlx::query_as(
        r#"SELECT basic_salary, is_prorated
           FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(basic_salary, 500_000, "full month pays the full basic");
    assert_eq!(is_prorated, Some(false));
}

// ─── Statutory wage bases ───

/// Overtime is outside the EPF contributable wage but inside the SOCSO, EIS and
/// PCB bases.
///
/// EPF Act 1991 s.2 expressly excludes overtime from "wages"; ESSA 1969 s.2 and
/// EIS Act 2017 s.2 expressly include it, and MTD rates on total taxable
/// remuneration. The engine used to feed one OT-inclusive `gross` to all four,
/// so any payslip carrying overtime resolved an inflated EPF Third Schedule
/// band. The basic here sits exactly on an EPF band edge, so two hours of
/// overtime is enough to move it: before the fix EPF came back 34_000/37_500.
#[tokio::test]
async fn overtime_is_excluded_from_the_epf_wage_but_not_socso_eis_pcb() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 300_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    // 2 h approved overtime at 1.5x on an hourly rate of 300_000 / 26 / 8.
    sqlx::query(
        r#"INSERT INTO overtime_applications
              (employee_id, company_id, ot_date, start_time, end_time, hours, ot_type, status)
           VALUES ($1, $2, '2024-04-10', TIME '18:00', TIME '20:00', 2.00, 'normal', 'approved')"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("seed approved overtime");

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        4,
        NaiveDate::from_ymd_opt(2024, 5, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    let (gross, total_overtime, epf_ee, epf_er, socso_ee, socso_er, eis_ee, eis_er): (
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = sqlx::query_as(
        r#"SELECT gross_salary, total_overtime, epf_employee, epf_employer,
                  socso_employee, socso_employer, eis_employee, eis_employer
           FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("item should exist");

    assert_eq!(total_overtime, 4_327, "2 h @ 1.5x on 300_000 / 26 / 8");
    assert_eq!(
        gross, 304_327,
        "gross stays OT-inclusive — only the EPF base is narrowed"
    );

    // EPF rates on 300_000 (band 280001-300000), not on 304_327.
    assert_eq!(epf_ee, 32_000, "EPF must ignore the overtime");
    assert_eq!(epf_er, 35_000);

    // SOCSO and EIS rate on 304_327 (band 300001-310000) — overtime is wages
    // for both, so these deliberately differ from the OT-free figures.
    assert_eq!(socso_ee, 1_125, "SOCSO includes overtime");
    assert_eq!(socso_er, 2_065);
    assert_eq!(eis_ee, 610, "EIS includes overtime");
    assert_eq!(eis_er, 610);
}

/// The same split holds for attendance-derived overtime, which reaches the
/// engine by a different route than an approved application.
#[tokio::test]
async fn epf_band_does_not_shift_with_attendance_overtime() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 300_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    sqlx::query(
        r#"INSERT INTO attendance_records
              (company_id, employee_id, check_in_at, check_out_at, method, status,
               hours_worked, overtime_hours)
           VALUES ($1, $2,
                   ('2024-06-10'::date + TIME '09:00')::timestamp AT TIME ZONE 'Asia/Kuala_Lumpur',
                   ('2024-06-10'::date + TIME '20:00')::timestamp AT TIME ZONE 'Asia/Kuala_Lumpur',
                   'manual', 'present', 11.00, 2.00)"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .execute(&pool)
    .await
    .expect("seed attendance overtime");

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        6,
        NaiveDate::from_ymd_opt(2024, 7, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    let (gross, total_overtime, epf_ee, socso_ee): (i64, i64, i64, i64) = sqlx::query_as(
        r#"SELECT gross_salary, total_overtime, epf_employee, socso_employee
           FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("item should exist");

    assert_eq!(total_overtime, 4_327);
    assert_eq!(gross, 304_327);
    assert_eq!(
        epf_ee, 32_000,
        "attendance overtime is outside the EPF wage"
    );
    assert_eq!(socso_ee, 1_125, "but inside the SOCSO wage");
}

// ─── Missing date of birth ───

/// Blank the employee's date of birth, as a tenant that imported without one has.
async fn clear_date_of_birth(pool: &sqlx::PgPool, employee_id: uuid::Uuid) {
    sqlx::query("UPDATE employees SET date_of_birth = NULL WHERE id = $1")
        .bind(employee_id)
        .execute(pool)
        .await
        .expect("clear date_of_birth");
}

/// A run refuses to commit rather than rating an employee at an assumed age.
///
/// The engine used to substitute 30, which clears every age-based branch: the
/// SOCSO 55-59 guard, the Second Category split at 60, and the EIS 57-59 / 60+
/// exemptions. A 60-year-old with no date of birth on record therefore had an
/// employee contribution deducted they are exempt from.
#[tokio::test]
async fn payroll_run_is_blocked_when_an_employee_has_no_date_of_birth() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 400_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;
    clear_date_of_birth(&pool, employee_id).await;

    let employee_number: String =
        sqlx::query_scalar("SELECT employee_number FROM employees WHERE id = $1")
            .bind(employee_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let err = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        4,
        NaiveDate::from_ymd_opt(2024, 5, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect_err("a run must not rate an employee whose age is unknown");

    let message = format!("{err:?}");
    assert!(
        message.contains(&employee_number),
        "the error must name the employee to fix, got: {message}"
    );
    assert!(
        message.contains("date of birth"),
        "the error must name the field to fix, got: {message}"
    );

    let items: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM payroll_items pi
         JOIN payroll_runs pr ON pr.id = pi.payroll_run_id
         WHERE pr.company_id = $1",
    )
    .bind(company_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(items, 0, "nothing may be committed by a refused run");
}

/// Preview names the affected employees under the blocking heading, once.
#[tokio::test]
async fn preview_blocks_and_names_employees_missing_a_date_of_birth() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 400_000).await;
    clear_date_of_birth(&pool, employee_id).await;

    let preview = payroll_engine::preview_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        4,
        NaiveDate::from_ymd_opt(2024, 5, 5).unwrap(),
    )
    .await
    .expect("preview_payroll");

    assert!(!preview.can_process, "the run must not be offered");

    let flagged: Vec<_> = preview
        .blocking
        .iter()
        .filter(|d| d.code == "missing_date_of_birth")
        .collect();
    assert_eq!(flagged.len(), 1, "one line per employee: {flagged:?}");
    assert_eq!(flagged[0].employee_id, Some(employee_id));
    assert!(flagged[0].employee_number.is_some());
    assert!(flagged[0].employee_name.is_some());

    assert!(
        !preview
            .warnings
            .iter()
            .any(|w| w.code == "missing_date_of_birth"),
        "it is a blocker now, not an advisory: {:?}",
        preview.warnings
    );

    // The generic calculation-failed line would repeat what the specific one
    // already says, so it is suppressed for the same employee.
    assert!(
        !preview.blocking.iter().any(|d| {
            d.code == "employee_calculation_failed" && d.employee_id == Some(employee_id)
        }),
        "the specific diagnostic must not be duplicated: {:?}",
        preview.blocking
    );

    // The projected-payslip row still carries its own error so the UI can mark it.
    let row = preview
        .employees
        .iter()
        .find(|e| e.employee_id == employee_id)
        .expect("the employee is still listed");
    assert!(
        row.error.is_some(),
        "the row must be marked as uncalculable"
    );
}

/// One unrateable employee does not hide what the rest of the group would be paid.
#[tokio::test]
async fn preview_still_prices_the_other_employees_when_one_is_missing_a_dob() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let rateable = seed_employee(&pool, company_id, Some(group_id), 400_000).await;
    let unrateable = seed_employee(&pool, company_id, Some(group_id), 300_000).await;
    clear_date_of_birth(&pool, unrateable).await;

    let preview = payroll_engine::preview_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        4,
        NaiveDate::from_ymd_opt(2024, 5, 5).unwrap(),
    )
    .await
    .expect("preview_payroll");

    assert_eq!(preview.employee_count, 2);
    assert_eq!(preview.payable_count, 1, "only the rateable one computes");
    assert_eq!(
        preview.total_gross, 400_000,
        "totals cover only the employees that could be rated"
    );

    let rateable_row = preview
        .employees
        .iter()
        .find(|e| e.employee_id == rateable)
        .expect("the rateable employee is listed");
    assert!(rateable_row.error.is_none());
    assert_eq!(rateable_row.gross_salary, 400_000);
}

// ─── Unrated (forgotten check-out) overtime ───

/// Seed one closed attendance record on `date` whose overtime was left unrated.
async fn seed_unrated_overtime_record(
    pool: &sqlx::PgPool,
    company_id: uuid::Uuid,
    employee_id: uuid::Uuid,
    date: NaiveDate,
) {
    sqlx::query(
        r#"INSERT INTO attendance_records
              (company_id, employee_id, check_in_at, check_out_at, method, status,
               hours_worked, overtime_hours)
           VALUES ($1, $2,
                   ($3::date + TIME '09:00')::timestamp AT TIME ZONE 'Asia/Kuala_Lumpur',
                   ($3::date + TIME '08:00' + INTERVAL '1 day')::timestamp AT TIME ZONE 'Asia/Kuala_Lumpur',
                   'manual', 'present', 23.00, NULL)"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .bind(date)
    .execute(pool)
    .await
    .expect("seed unrated attendance record");
}

/// The ~14 h of overtime a forgotten check-out used to produce is not paid.
///
/// This is the regression that pins the defect: the figure was written from
/// wall-clock elapsed time and flowed straight into gross, inflating every
/// statutory contribution derived from it.
#[tokio::test]
async fn unrated_overtime_is_not_paid() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 300_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    seed_unrated_overtime_record(
        &pool,
        company_id,
        employee_id,
        NaiveDate::from_ymd_opt(2024, 6, 10).unwrap(),
    )
    .await;

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        6,
        NaiveDate::from_ymd_opt(2024, 7, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("an unrated record must not stop the run");

    let (gross, total_overtime, epf_ee): (i64, i64, i64) = sqlx::query_as(
        r#"SELECT gross_salary, total_overtime, epf_employee
           FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("item should exist");

    assert_eq!(total_overtime, 0, "unrated hours are not paid");
    assert_eq!(
        gross, 300_000,
        "gross matches a run with no attendance record at all"
    );
    assert_eq!(epf_ee, 32_000, "and so does every statutory figure");
}

/// An employee whose only attendance overtime is NULL must not fail the read.
///
/// `SUM` over an all-NULL group returns NULL while the group still exists, and
/// the non-null assertion on the projection turned that into a runtime error
/// that failed the whole run. Already reachable via a correction that clears the
/// check-out; the per-day ceiling makes it routine.
#[tokio::test]
async fn payroll_reads_survive_an_employee_whose_only_overtime_is_null() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    use crate::repositories::reads::payroll as payroll_reads;

    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 300_000).await;
    seed_unrated_overtime_record(
        &pool,
        company_id,
        employee_id,
        NaiveDate::from_ymd_opt(2024, 6, 10).unwrap(),
    )
    .await;

    let rows = payroll_reads::attendance_ot_hours(
        &pool,
        &[employee_id],
        NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
        NaiveDate::from_ymd_opt(2024, 6, 30).unwrap(),
        "Asia/Kuala_Lumpur",
    )
    .await
    .expect("an all-NULL overtime group must not fail the read");

    let hours = rows
        .iter()
        .find(|r| r.employee_id == employee_id)
        .map(|r| r.hours)
        .unwrap_or(0.0);
    assert_eq!(hours, 0.0, "unrated hours read as zero, not as an error");
}

/// The preview tells the operator a correction is owed, without blocking the run.
#[tokio::test]
async fn preview_warns_about_unrated_overtime() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 300_000).await;
    seed_unrated_overtime_record(
        &pool,
        company_id,
        employee_id,
        NaiveDate::from_ymd_opt(2024, 6, 10).unwrap(),
    )
    .await;

    let preview = payroll_engine::preview_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        6,
        NaiveDate::from_ymd_opt(2024, 7, 5).unwrap(),
    )
    .await
    .expect("preview_payroll");

    let warning = preview
        .warnings
        .iter()
        .find(|w| w.code == "unrated_overtime")
        .expect("an unrated record must be surfaced");
    assert_eq!(warning.employee_id, Some(employee_id));
    assert!(
        warning.message.contains("1 attendance record"),
        "the warning names how many records are affected: {}",
        warning.message
    );
    assert!(
        preview.can_process,
        "unrated hours are already excluded from pay, so this is advisory: {:?}",
        preview.blocking
    );
}

// ─── Recurring allowance / deduction windows ───

/// Seed one recurring allowance or deduction with an explicit effective window.
#[allow(clippy::too_many_arguments)]
async fn seed_recurring_line(
    pool: &sqlx::PgPool,
    company_id: uuid::Uuid,
    employee_id: uuid::Uuid,
    category: &str,
    name: &str,
    amount: i64,
    effective_from: NaiveDate,
    effective_to: Option<NaiveDate>,
) {
    sqlx::query(
        r#"INSERT INTO employee_allowances
              (employee_id, company_id, category, name, amount, is_taxable,
               is_recurring, effective_from, effective_to, is_active)
           VALUES ($1, $2, $3, $4, $5, TRUE, TRUE, $6, $7, TRUE)"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(category)
    .bind(name)
    .bind(amount)
    .bind(effective_from)
    .bind(effective_to)
    .execute(pool)
    .await
    .expect("seed recurring line");
}

/// A leaver's allowance, correctly ended on their last day, is prorated rather
/// than dropped.
///
/// Selection used to be a point test at the period end, so `effective_to` on the
/// 15th failed `effective_to >= period_end` and the allowance paid RM0 while the
/// basic was prorated to 15/31. The allowance reaches gross, so every statutory
/// contribution was understated with it.
#[tokio::test]
async fn leaver_allowance_ending_mid_month_is_prorated_not_dropped() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let last_day = NaiveDate::from_ymd_opt(2024, 7, 15).unwrap();
    sqlx::query("UPDATE employees SET date_resigned = $2 WHERE id = $1")
        .bind(employee_id)
        .bind(last_day)
        .execute(&pool)
        .await
        .expect("set date_resigned");

    // RM310 a month, so 15 of 31 days is exactly RM150 with no rounding to
    // argue about.
    seed_recurring_line(
        &pool,
        company_id,
        employee_id,
        "earning",
        "Travel allowance",
        31_000,
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        Some(last_day),
    )
    .await;

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        7,
        NaiveDate::from_ymd_opt(2024, 8, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    let (basic, total_allowances, gross): (i64, i64, i64) = sqlx::query_as(
        r#"SELECT basic_salary, total_allowances, gross_salary
           FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("item should exist");

    assert_eq!(
        total_allowances, 15_000,
        "15 of 31 days of a RM310 allowance, not RM0"
    );
    assert_eq!(basic, 241_935, "500_000 x 15 / 31");
    assert_eq!(
        gross,
        basic + 15_000,
        "the prorated allowance moves the statutory bases with it"
    );

    let description: String = sqlx::query_scalar(
        r#"SELECT pid.description FROM payroll_item_details pid
           JOIN payroll_items pi ON pi.id = pid.payroll_item_id
           WHERE pi.payroll_run_id = $1 AND pi.employee_id = $2
             AND pid.description LIKE 'Travel allowance%'"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("the allowance line is stored");
    assert!(
        description.contains("prorated") && description.contains("15 of 31"),
        "the stored line must say why it is not the configured amount: {description}"
    );
}

/// An allowance granted part-way through the month is not paid for the whole of
/// it. The mirror of the leaver case: `effective_from <= period_end` passed, so
/// a grant dated the 25th paid all 31 days.
#[tokio::test]
async fn allowance_granted_mid_month_is_not_paid_in_full() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    seed_recurring_line(
        &pool,
        company_id,
        employee_id,
        "earning",
        "Site allowance",
        31_000,
        NaiveDate::from_ymd_opt(2024, 7, 25).unwrap(),
        None,
    )
    .await;

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        7,
        NaiveDate::from_ymd_opt(2024, 8, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    let (basic, total_allowances, is_prorated): (i64, i64, Option<bool>) = sqlx::query_as(
        r#"SELECT basic_salary, total_allowances, is_prorated
           FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("item should exist");

    assert_eq!(
        total_allowances, 7_000,
        "the 25th to the 31st is 7 of 31 days"
    );
    assert_eq!(
        basic, 500_000,
        "the employee themselves worked the full month"
    );
    assert_eq!(
        is_prorated,
        Some(false),
        "`is_prorated` describes the employment window, not an allowance window"
    );
}

/// A recurring *deduction* window prorates identically — it flows through the
/// same read and the same branch, so a mid-month end used to drop the whole
/// deduction and a mid-month start used to charge a full one.
#[tokio::test]
async fn recurring_deduction_window_prorates_the_same_way() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    seed_recurring_line(
        &pool,
        company_id,
        employee_id,
        "deduction",
        "Staff loan",
        31_000,
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        Some(NaiveDate::from_ymd_opt(2024, 7, 15).unwrap()),
    )
    .await;

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        7,
        NaiveDate::from_ymd_opt(2024, 8, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    let other_deductions: i64 = sqlx::query_scalar(
        r#"SELECT total_other_deductions FROM payroll_items
           WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("item should exist");

    assert_eq!(
        other_deductions, 15_000,
        "the loan is charged for the 15 days it was in force, not 0 and not 31"
    );
}

/// An earning staged under an `item_type` outside the four printed allow-lists
/// still reconciles: the stored breakdown carries it by name, and the residual
/// the EA form prints closes the same gap for a document that cannot.
///
/// This is the reported scenario — items 1..5 summing to less than the total
/// printed above them on a statutory form.
#[tokio::test]
async fn an_unclassified_earning_reconciles_in_the_breakdown_and_as_a_residual() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 300_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    // Exactly what the payroll-entry UI writes for its "Other Earning" option.
    sqlx::query(
        r#"INSERT INTO payroll_entries
              (employee_id, company_id, period_year, period_month, category,
               item_type, description, amount, is_processed)
           VALUES ($1, $2, 2024, 3, 'earning', 'manual_adjustment',
                   'Retention bonus', 50_000, FALSE)"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("seed unclassified earning");

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        3,
        NaiveDate::from_ymd_opt(2024, 4, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    let (basic, allowances, overtime, bonus, commission, gross): (i64, i64, i64, i64, i64, i64) =
        sqlx::query_as(
            r#"SELECT basic_salary, total_allowances, total_overtime,
                      total_bonus, total_commission, gross_salary
               FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2"#,
        )
        .bind(run.id)
        .bind(employee_id)
        .fetch_one(&pool)
        .await
        .expect("item should exist");

    assert_eq!(gross, 350_000, "the staged earning reached gross");
    assert_eq!(
        unclassified_earnings(gross, basic, allowances, overtime, bonus, commission),
        50_000,
        "and none of the five named categories accounts for it"
    );

    // The payslip PDF renders from these lines, so the same money is named
    // rather than left as a gap between the rows and the total.
    let lines: Vec<(String, String, i64)> = sqlx::query_as(
        r#"SELECT pid.category, pid.description, pid.amount
           FROM payroll_item_details pid
           JOIN payroll_items pi ON pi.id = pid.payroll_item_id
           WHERE pi.payroll_run_id = $1 AND pi.employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_all(&pool)
    .await
    .expect("breakdown lines");

    let earnings: i64 = lines
        .iter()
        .filter(|(category, ..)| category == "earning")
        .map(|(.., amount)| amount)
        .sum();
    assert_eq!(earnings, gross, "the printed rows must add up to the total");
    assert!(
        lines
            .iter()
            .any(|(_, description, amount)| description == "Retention bonus" && *amount == 50_000),
        "the employee sees the real description, not an anonymous residual: {lines:?}"
    );
}

// ─── PCB edits ───

/// Process a one-employee run; returns `(company_id, run_id, employee_id, actor_id)`.
async fn processed_run_for_pcb_edit(
    pool: &sqlx::PgPool,
    month: i32,
) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let company_id = seed_company(pool).await;
    let group_id = seed_payroll_group(pool, company_id).await;
    let employee_id = seed_employee(pool, company_id, Some(group_id), 500_000).await;
    let user_id = seed_user(pool, company_id, "payroll_admin").await;

    let run = payroll_engine::process_payroll(
        pool,
        company_id,
        group_id,
        2024,
        month,
        NaiveDate::from_ymd_opt(2024, 12, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    (company_id, run.id, employee_id, user_id)
}

/// `(pcb_amount, total_deductions, net_salary)` for one payslip.
async fn pcb_figures(
    pool: &sqlx::PgPool,
    run_id: uuid::Uuid,
    employee_id: uuid::Uuid,
) -> (i64, i64, i64) {
    sqlx::query_as(
        r#"SELECT pcb_amount, total_deductions, net_salary FROM payroll_items
           WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run_id)
    .bind(employee_id)
    .fetch_one(pool)
    .await
    .expect("item should exist")
}

/// `(sum of deduction lines, number of PCB lines)` in the stored breakdown.
async fn stored_deduction_lines(
    pool: &sqlx::PgPool,
    run_id: uuid::Uuid,
    employee_id: uuid::Uuid,
) -> (i64, i64) {
    sqlx::query_as(
        r#"SELECT COALESCE(SUM(pid.amount), 0)::BIGINT,
                  COUNT(*) FILTER (WHERE pid.item_type = 'pcb')
           FROM payroll_item_details pid
           JOIN payroll_items pi ON pi.id = pid.payroll_item_id
           WHERE pi.payroll_run_id = $1 AND pi.employee_id = $2
             AND pid.category = 'deduction'"#,
    )
    .bind(run_id)
    .bind(employee_id)
    .fetch_one(pool)
    .await
    .expect("deduction lines")
}

/// A PCB edit cannot drive the payslip's net salary below zero.
///
/// `compute_payslip` fails the whole run rather than create a negative net, but
/// the edit path had only a `pcb_amount < 0` guard — an operator typing sen for
/// ringgit wrote a negative net that survived submit, approve and pay, because
/// the lifecycle transitions never re-validate the figures.
#[tokio::test]
async fn pcb_edit_cannot_drive_net_salary_negative() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let (company_id, run_id, employee_id, actor_id) = processed_run_for_pcb_edit(&pool, 1).await;
    let (pcb_before, deductions_before, net_before) = pcb_figures(&pool, run_id, employee_id).await;
    let run_net_before: i64 =
        sqlx::query_scalar("SELECT total_net FROM payroll_runs WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let err = payroll_service::update_item_pcb(
        &pool,
        company_id,
        run_id,
        employee_id,
        net_before + pcb_before + 1,
        actor_id,
        None,
    )
    .await
    .expect_err("an edit past the ceiling must be refused");

    let message = format!("{err:?}");
    assert!(
        message.contains("negative net salary"),
        "the error must say what is wrong, got: {message}"
    );
    assert!(
        message.contains(&(net_before + pcb_before).to_string()),
        "and quote the ceiling so the operator can act on it, got: {message}"
    );

    // The guard runs before any write, so the transaction rolls back untouched.
    assert_eq!(
        pcb_figures(&pool, run_id, employee_id).await,
        (pcb_before, deductions_before, net_before)
    );
    let run_net_after: i64 = sqlx::query_scalar("SELECT total_net FROM payroll_runs WHERE id = $1")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(run_net_after, run_net_before, "the run total is untouched");
}

/// The boundary itself is legal: the whole of net may go to PCB, leaving zero.
#[tokio::test]
async fn pcb_edit_at_the_exact_ceiling_is_allowed() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let (company_id, run_id, employee_id, actor_id) = processed_run_for_pcb_edit(&pool, 2).await;
    let (pcb_before, _, net_before) = pcb_figures(&pool, run_id, employee_id).await;

    payroll_service::update_item_pcb(
        &pool,
        company_id,
        run_id,
        employee_id,
        net_before + pcb_before,
        actor_id,
        None,
    )
    .await
    .expect("the exact ceiling is a legal edit");

    let (pcb_after, _, net_after) = pcb_figures(&pool, run_id, employee_id).await;
    assert_eq!(pcb_after, net_before + pcb_before);
    assert_eq!(net_after, 0);
}

/// A PCB edit keeps the stored breakdown in step, including when the line has to
/// be created or removed.
///
/// The engine drops zero-valued lines, so a payslip whose PCB was 0 has no row
/// to update and an edit back to 0 must remove one. Leaving the table stale made
/// the deduction lines stop summing to `total_deductions` — on the very table
/// the payslip PDF renders from.
#[tokio::test]
async fn pcb_edit_keeps_the_stored_breakdown_reconciling() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let (company_id, run_id, employee_id, actor_id) = processed_run_for_pcb_edit(&pool, 3).await;

    for target in [50_000_i64, 0, 30_000] {
        payroll_service::update_item_pcb(
            &pool,
            company_id,
            run_id,
            employee_id,
            target,
            actor_id,
            None,
        )
        .await
        .unwrap_or_else(|e| panic!("PCB edit to {target} should succeed: {e:?}"));

        let (_, total_deductions, _) = pcb_figures(&pool, run_id, employee_id).await;
        let (line_total, pcb_lines) = stored_deduction_lines(&pool, run_id, employee_id).await;

        assert_eq!(
            line_total, total_deductions,
            "deduction lines must still reconcile after editing PCB to {target}"
        );
        assert_eq!(
            pcb_lines,
            i64::from(target != 0),
            "a zero PCB has no line and a non-zero PCB has exactly one, at {target}"
        );
    }
}

// ─── PCB base: additional remuneration ───

/// RM6,000/month, with a one-off payment of one month's pay.
///
/// The income level is load-bearing. On the RM3,000 salary and RM1,000 payment
/// this used to seed, the year's tax came to RM108 with the payment taxed once
/// and RM264 with it annualised — both inside the RM400 individual rebate, so
/// the correct deduction was zero either way and the comparison below had
/// nothing to observe. At RM6,000 every leg's chargeable income is far above the
/// RM35,000 rebate ceiling, so the rebate is out of the picture and the only
/// thing separating the legs is whether the payment was annualised.
const ADDITIONAL_REMUNERATION_SALARY: i64 = 600_000;
const ADDITIONAL_REMUNERATION_PAYMENT: i64 = 600_000;

/// Process a one-employee run, optionally staging one earning first; returns the
/// payslip's `(gross, epf_employee, socso_employee, eis_employee, pcb)`.
///
/// Each call gets its own company, so every leg can use the same period — the
/// month drives `months_worked` and therefore the annualisation, and comparing
/// legs across different months would compare two different calculations.
async fn run_with_staged_earning(
    pool: &sqlx::PgPool,
    month: i32,
    staged: Option<(&str, i64)>,
) -> (i64, i64, i64, i64, i64) {
    let company_id = seed_company(pool).await;
    let group_id = seed_payroll_group(pool, company_id).await;
    let employee_id = seed_employee(
        pool,
        company_id,
        Some(group_id),
        ADDITIONAL_REMUNERATION_SALARY,
    )
    .await;
    let user_id = seed_user(pool, company_id, "payroll_admin").await;

    if let Some((item_type, amount)) = staged {
        sqlx::query(
            r#"INSERT INTO payroll_entries
                  (employee_id, company_id, period_year, period_month, category,
                   item_type, description, amount, is_processed)
               VALUES ($1, $2, 2024, $3, 'earning', $4, 'Staged earning', $5, FALSE)"#,
        )
        .bind(employee_id)
        .bind(company_id)
        .bind(month)
        .bind(item_type)
        .bind(amount)
        .execute(pool)
        .await
        .expect("seed staged earning");
    }

    let run = payroll_engine::process_payroll(
        pool,
        company_id,
        group_id,
        2024,
        month,
        NaiveDate::from_ymd_opt(2024, 12, 5).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("process_payroll");

    sqlx::query_as(
        r#"SELECT gross_salary, epf_employee, socso_employee, eis_employee, pcb_amount
           FROM payroll_items WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run.id)
    .bind(employee_id)
    .fetch_one(pool)
    .await
    .expect("item should exist")
}

/// Bonus and commission are wages for EPF/SOCSO/EIS but *additional
/// remuneration* for MTD, and the engine must route them accordingly.
///
/// The same RM6,000 is staged three ways on a RM6,000 salary, against a fourth
/// run that stages nothing. All three produce identical gross and identical
/// statutory contributions, because all three are wages. Only the PCB differs,
/// and the salary-only run is what isolates the difference: its normal
/// remuneration is the same RM6,000 the bonus legs annualise, so subtracting it
/// leaves exactly what each treatment charged for the payment itself —
///
///   * as bonus or commission: the Schedule 2 differential, the year's tax on
///     the payment, charged in the month it is paid;
///   * as `manual_adjustment`: normal remuneration, so the payment is multiplied
///     by the nine remaining months and taxed as if it recurred — which is
///     exactly what bonus and commission used to do.
///
/// Derived against the seed rules in `migrations/1001_data.sql` for April 2024,
/// nine remaining months, no YTD. Reliefs are identical in all four legs (RM9,000
/// individual, EPF at its RM3,000 cap either way, SOCSO and EIS at their schedule
/// ceilings), so the legs differ only in the base they annualise:
///
///   salary only    9 × RM6,000  = RM54,000  chargeable RM41,715 → PCB RM112
///   as bonus       same base, plus the differential on RM6,000   → PCB RM472
///   as recurring   9 × RM12,000 = RM108,000 chargeable RM95,715 → PCB RM954
///
/// RM360 charged for the payment against RM842 — the annualised leg deducts more
/// than twice as much, in a month where nothing recurs.
#[tokio::test]
async fn bonus_and_commission_are_taxed_as_additional_remuneration() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let month = 4;
    let salary = ADDITIONAL_REMUNERATION_SALARY;
    let payment = ADDITIONAL_REMUNERATION_PAYMENT;
    let gross = salary + payment;

    let salary_only = run_with_staged_earning(&pool, month, None).await;
    let as_bonus = run_with_staged_earning(&pool, month, Some(("bonus", payment))).await;
    let as_commission = run_with_staged_earning(&pool, month, Some(("commission", payment))).await;
    let as_recurring =
        run_with_staged_earning(&pool, month, Some(("manual_adjustment", payment))).await;

    // Gross and every statutory contribution include the payment however it is
    // labelled — narrowing the PCB base must not narrow the others.
    assert_eq!(salary_only.0, salary, "the baseline run stages nothing");
    assert_eq!(as_bonus.0, gross, "the bonus is inside gross");
    assert_eq!(
        (as_bonus.0, as_bonus.1, as_bonus.2, as_bonus.3),
        (
            as_recurring.0,
            as_recurring.1,
            as_recurring.2,
            as_recurring.3
        ),
        "EPF, SOCSO and EIS levy on additional remuneration too"
    );

    // Commission is summed into the same base as bonus, so the two are
    // indistinguishable. If the engine dropped either term these would diverge.
    assert_eq!(
        as_bonus, as_commission,
        "bonus and commission follow the same path"
    );

    // What each treatment charged for the payment, over and above the month the
    // employee would have had without it.
    let charged_as_additional = as_bonus.4 - salary_only.4;
    let charged_as_recurring = as_recurring.4 - salary_only.4;

    assert!(
        charged_as_additional > 0,
        "additional remuneration is still taxed — the whole year's tax on it falls due in the \
         month it is paid, got {charged_as_additional} sen"
    );
    assert!(
        charged_as_additional < charged_as_recurring,
        "annualising a one-off payment over-deducts: {charged_as_additional} sen charged as \
         additional remuneration vs {charged_as_recurring} sen annualised"
    );
}
