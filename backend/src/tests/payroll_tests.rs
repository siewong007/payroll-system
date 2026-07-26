use chrono::NaiveDate;

use crate::services::payroll_engine;
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
