//! Read-side payroll-run management (summary, listing, items, groups). Run mutations
//! live in payroll_lifecycle_service (transitions) and this service (delete / PCB edit).

use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::Zero;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::audit::AuditLogWithUser;
use crate::models::payroll::{
    JournalLine, JournalPreview, PayrollActionItem, PayrollGroup, PayrollItem, PayrollOverview,
    PayrollPeriodTotals, PayrollPeriodVariance, PayrollRun, PayrollSummary, PayslipBreakdown,
};
use crate::repositories::reads::audit as audit_reads;
use crate::repositories::reads::payroll as payroll_reads;
use crate::repositories::reads::payroll_overview as overview_reads;
use crate::repositories::reads::reports as report_reads;
use crate::repositories::{
    claims, employees as employee_reads, payroll_entries, payroll_groups, payroll_item_details,
    payroll_items, payroll_runs,
};
use crate::services::audit_service::{self, AuditRequestMeta};

/// Run header + per-employee payslip summaries.
pub async fn get_summary(pool: &PgPool, company_id: Uuid, id: Uuid) -> AppResult<PayrollSummary> {
    let run = payroll_runs::get_for_company(pool, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))?;
    let items = payroll_reads::item_summaries_for_run(pool, id).await?;
    let calculation_snapshot = payroll_runs::get_calculation_snapshot(pool, id, company_id).await?;
    Ok(PayrollSummary {
        payroll_run: run,
        items,
        calculation_snapshot,
    })
}

/// One payslip with the stored lines behind each figure.
///
/// `NotFound` if the run is not in the company, so a run id alone cannot expose
/// another tenant's payslip — the same rule `list_items` applies.
pub async fn get_payslip_breakdown(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    employee_id: Uuid,
) -> AppResult<PayslipBreakdown> {
    if !payroll_runs::exists(pool, run_id, company_id).await? {
        return Err(AppError::NotFound("Payroll run not found".into()));
    }

    let item = payroll_items::get_for_employee(pool, run_id, employee_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Payroll item not found".into()))?;
    let employee = employee_reads::name_and_number(pool, employee_id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Employee not found".into()))?;
    let lines = payroll_item_details::list_for_item(pool, item.id).await?;

    Ok(PayslipBreakdown {
        employee_id,
        employee_name: employee.0,
        employee_number: employee.1,
        item,
        lines,
    })
}

pub async fn list_runs(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<PayrollRun>> {
    payroll_runs::list_for_company(pool, company_id).await
}

pub async fn list_groups(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<PayrollGroup>> {
    payroll_groups::list_active(pool, company_id).await
}

/// Payslip figures for a run. `NotFound` if the run is not in the company —
/// without this the run id alone would expose another tenant's payroll.
pub async fn list_items(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
) -> AppResult<Vec<PayrollItem>> {
    if !payroll_runs::exists(pool, run_id, company_id).await? {
        return Err(AppError::NotFound("Payroll run not found".into()));
    }
    payroll_items::list_for_run(pool, run_id).await
}

/// Audit-trail rows attributable to a payroll run (the run itself plus item
/// edits that reference it). `NotFound` if the run is not in the company.
pub async fn list_run_audit_logs(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
) -> AppResult<Vec<AuditLogWithUser>> {
    if !payroll_runs::exists(pool, run_id, company_id).await? {
        return Err(AppError::NotFound("Payroll run not found".into()));
    }
    audit_reads::list_for_run(pool, company_id, run_id).await
}

/// Hard-delete a non-locked run and revert its staged entries/claims, in one transaction.
pub async fn delete_run(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let run = payroll_runs::get_for_company(pool, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))?;

    if run.status == "processing" {
        return Err(AppError::BadRequest(
            "Payroll run is currently processing and cannot be deleted".into(),
        ));
    }

    if matches!(
        run.status.as_str(),
        "pending_approval" | "approved" | "paid"
    ) || run.locked_at.is_some()
    {
        return Err(AppError::BadRequest(
            "Submitted, approved, or paid payroll runs are locked and cannot be deleted".into(),
        ));
    }

    // A later committed run's stored ytd_* and PCB annualisation were computed
    // from this run's figures; deleting it would leave those describing a run
    // that no longer exists.
    if payroll_reads::run_has_later_committed_run(pool, company_id, id).await? {
        return Err(AppError::BadRequest(
            "A later payroll run already includes these employees; delete that run first".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    payroll_entries::revert_for_run(&mut *tx, id, company_id, actor_id).await?;
    // By run id, not by period: a run legitimately reimburses claims incurred
    // before its own period, so a period-bounded revert would un-process claims
    // an earlier run already paid.
    claims::revert_for_run(&mut *tx, id, company_id).await?;
    payroll_item_details::delete_for_run(&mut *tx, id).await?;
    payroll_items::delete_for_run(&mut *tx, id).await?;
    // Re-checks status/lock atomically: the guard above ran outside this
    // transaction, so a concurrent submit or approve could have locked the run.
    if !payroll_runs::delete_if_unlocked(&mut *tx, id, company_id).await? {
        return Err(AppError::BadRequest(
            "Payroll run was submitted or locked while being deleted".into(),
        ));
    }
    tx.commit().await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "delete",
        "payroll_run",
        Some(id),
        Some(serde_json::to_value(&run).unwrap_or_default()),
        None,
        Some(&format!(
            "Deleted payroll run {} for {:02}/{}",
            id, run.period_month, run.period_year
        )),
        audit_meta,
    )
    .await;

    Ok(())
}

/// Edit a single payslip's PCB while the run is still `processed`, recomputing the
/// item's deductions/net and the run totals in one locked transaction.
pub async fn update_item_pcb(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    employee_id: Uuid,
    pcb_amount: i64,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<PayrollSummary> {
    if pcb_amount < 0 {
        return Err(AppError::BadRequest("PCB amount cannot be negative".into()));
    }

    let mut tx = pool.begin().await?;

    let run_row = payroll_runs::get_status_locked(&mut *tx, run_id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))?;

    if run_row.status != "processed" {
        return Err(AppError::BadRequest(
            "PCB can only be edited while the payroll run is processed and not yet approved".into(),
        ));
    }

    let has_later_run = payroll_reads::employee_has_later_run(
        &mut *tx,
        employee_id,
        company_id,
        run_row.period_year,
        run_row.period_month,
    )
    .await?;

    if has_later_run {
        return Err(AppError::BadRequest(
            "PCB cannot be edited because a later payroll run already exists for this employee"
                .into(),
        ));
    }

    let current = payroll_items::get_pcb_fields_locked(&mut *tx, run_id, employee_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Payroll item not found".into()))?;

    let old_pcb = current.pcb_amount;
    let delta = pcb_amount - old_pcb;
    let new_total_deductions = current.total_deductions + delta;
    let new_net_salary = current.net_salary - delta;
    let new_ytd_pcb = current.ytd_pcb + delta;

    // Mirrors `compute_payslip`, which fails the whole run rather than create a
    // negative net; an edit must not be able to reach one either. An operator
    // typing sen where they meant ringgit otherwise wrote a negative net that
    // survived submit, approve and pay, because the lifecycle transitions never
    // re-validate the figures. `net - (new - old) >= 0` is exactly
    // `new <= net + old`, so the message quotes that ceiling rather than making
    // the operator derive it. Nothing has been written at this point, so the
    // transaction rolls back untouched.
    if new_net_salary < 0 {
        return Err(AppError::BadRequest(format!(
            "PCB of {} sen would leave a negative net salary. The most this payslip can carry is {} sen.",
            pcb_amount,
            current.net_salary + current.pcb_amount
        )));
    }

    payroll_items::update_pcb(
        &mut *tx,
        run_id,
        employee_id,
        pcb_amount,
        new_total_deductions,
        new_net_salary,
        new_ytd_pcb,
    )
    .await?;

    // The stored breakdown is what the payslip PDF and the admin drawer render
    // from, so leaving it on the old PCB would make the lines stop summing to
    // `total_deductions` on a statutory-adjacent document. Same transaction and
    // the same `FOR UPDATE` lock the figures were read under.
    payroll_item_details::replace_pcb_line(&mut *tx, current.id, pcb_amount).await?;

    payroll_runs::bump_pcb_totals(&mut *tx, run_id, company_id, delta, actor_id).await?;

    tx.commit().await?;

    let summary = get_summary(pool, company_id, run_id).await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "update",
        "payroll_item",
        None,
        Some(serde_json::json!({
            "payroll_run_id": run_id,
            "employee_id": employee_id,
            "pcb_amount": old_pcb
        })),
        Some(serde_json::json!({
            "payroll_run_id": run_id,
            "employee_id": employee_id,
            "pcb_amount": pcb_amount
        })),
        Some("Updated payroll item PCB amount"),
        audit_meta,
    )
    .await;

    Ok(summary)
}

// ─── Payroll overview (operational dashboard) ───

/// `delta / base` as a percentage, `None` when the base is zero — a ratio to
/// zero reads as meaningful on a dashboard and is not.
fn pct_change(delta: i64, base: i64) -> Option<Decimal> {
    if base == 0 {
        return None;
    }
    Some((Decimal::from(delta) * Decimal::from(100) / Decimal::from(base)).round_dp(2))
}

/// Assemble the action queue: live data-quality flags plus lifecycle waits.
/// Each check is its own read-model query so the counts are SQL-true, and the
/// codes are shared with `PayrollDiagnostic` where the same condition also
/// blocks or warns inside a preview.
async fn build_action_queue(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<PayrollActionItem>> {
    const SAMPLE: i64 = 5;
    let mut items = Vec::new();

    let awaiting = overview_reads::runs_awaiting_action(pool, company_id).await?;
    for status in ["processed", "pending_approval", "approved"] {
        let runs: Vec<_> = awaiting.iter().filter(|r| r.status == status).collect();
        if runs.is_empty() {
            continue;
        }
        let (code, message) = match status {
            "processed" => (
                "runs_awaiting_submission",
                "Calculated but not yet submitted for approval",
            ),
            "pending_approval" => ("runs_awaiting_approval", "Awaiting payroll approval"),
            _ => ("runs_awaiting_payment", "Approved but not yet marked paid"),
        };
        items.push(PayrollActionItem {
            code: code.into(),
            severity: "info".into(),
            count: runs.len() as i64,
            message: format!("{}: {}", message, runs[0].payroll_group_name),
            detail: Some(runs[0].payroll_group_name.clone()),
            link: runs.first().map(|r| format!("/payroll/{}", r.id)),
            employees: Vec::new(),
        });
    }

    let unprocessed = overview_reads::unprocessed_entry_count(pool, company_id).await?;
    if unprocessed > 0 {
        items.push(PayrollActionItem {
            code: "unprocessed_entries".into(),
            severity: "info".into(),
            count: unprocessed,
            message: "Staged payroll entries not yet consumed by a run".into(),
            detail: None,
            link: Some("/payroll/entries".into()),
            employees: Vec::new(),
        });
    }

    // Data-quality flags: (read fn, diagnostic code, severity, message).
    // Codes match `PayrollDiagnostic.code` in payroll_engine where the same
    // condition exists there, so the UI can cross-link without prose matching.
    let checks: [(&str, &str, &str, &str); 5] = [
        (
            "missing_date_of_birth",
            "missing_date_of_birth",
            "blocking",
            "Active employees missing date of birth — statutory contributions cannot be rated",
        ),
        (
            "inactive_without_resignation_date",
            "inactive_without_resignation_date",
            "blocking",
            "Inactive employees with no resignation date — block processing until dated",
        ),
        (
            "missing_payroll_group",
            "missing_payroll_group",
            "warning",
            "Active employees not assigned to a payroll group — excluded from every run",
        ),
        (
            "missing_bank_account",
            "missing_bank_account",
            "warning",
            "Employees missing bank account details — cannot appear on a payment file",
        ),
        (
            "missing_statutory_ids",
            "missing_statutory_ids",
            "warning",
            "Employees missing EPF number or TIN — statutory filings will reject their rows",
        ),
    ];
    for (key, code, severity, message) in checks {
        let (count, employees) = match key {
            "missing_date_of_birth" => {
                overview_reads::missing_date_of_birth(pool, company_id, SAMPLE).await?
            }
            "inactive_without_resignation_date" => {
                overview_reads::inactive_without_resignation(pool, company_id, SAMPLE).await?
            }
            "missing_payroll_group" => {
                overview_reads::missing_payroll_group(pool, company_id, SAMPLE).await?
            }
            "missing_bank_account" => {
                overview_reads::missing_bank_account(pool, company_id, SAMPLE).await?
            }
            _ => overview_reads::missing_statutory_ids(pool, company_id, SAMPLE).await?,
        };
        if count == 0 {
            continue;
        }
        items.push(PayrollActionItem {
            code: code.into(),
            severity: severity.into(),
            count,
            message: message.into(),
            detail: None,
            link: Some("/employees".into()),
            employees,
        });
    }

    // Blocking first, then warnings, then informational waits.
    items.sort_by_key(|item| match item.severity.as_str() {
        "blocking" => 0,
        "warning" => 1,
        _ => 2,
    });
    Ok(items)
}

/// The operational payroll dashboard. Every figure is stored on committed runs
/// or read from live employee/approval records — nothing estimated.
pub async fn overview(pool: &PgPool, company_id: Uuid) -> AppResult<PayrollOverview> {
    // Newest-first; the trend is presented oldest-first.
    let mut periods = overview_reads::period_totals(pool, company_id).await?;
    let current_period = periods.first().cloned();
    let variance = match (periods.first(), periods.get(1)) {
        (Some(current), Some(previous)) => Some(PayrollPeriodVariance {
            period_year: current.period_year,
            period_month: current.period_month,
            previous_period_year: previous.period_year,
            previous_period_month: previous.period_month,
            gross_delta: current.total_gross - previous.total_gross,
            gross_change_pct: pct_change(
                current.total_gross - previous.total_gross,
                previous.total_gross,
            ),
            net_delta: current.total_net - previous.total_net,
            net_change_pct: pct_change(current.total_net - previous.total_net, previous.total_net),
            employer_cost_delta: current.total_employer_cost - previous.total_employer_cost,
            headcount_delta: current.employee_count - previous.employee_count,
        }),
        _ => None,
    };
    periods.reverse();
    let trend: Vec<PayrollPeriodTotals> = periods.into_iter().take(12).collect();

    let pipeline = overview_reads::status_counts(pool, company_id).await?;
    let departments = match &current_period {
        Some(period) => {
            report_reads::payroll_by_department(
                pool,
                company_id,
                period.period_year,
                period.period_month,
            )
            .await?
        }
        None => Vec::new(),
    };
    let mut recent_runs = payroll_runs::list_for_company(pool, company_id).await?;
    recent_runs.truncate(5);
    let action_queue = build_action_queue(pool, company_id).await?;

    Ok(PayrollOverview {
        generated_at: Utc::now(),
        current_period,
        variance,
        pipeline,
        trend,
        departments,
        recent_runs,
        action_queue,
    })
}

// ─── Payment file export ───

/// CSV of net pay per employee for an approved/paid run — the file a finance
/// user transcribes or uploads to the bank. The account number travels only
/// inside the file; the audit row records that an export happened, never the
/// numbers themselves.
pub async fn export_payment_file(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
    actor_user_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Vec<u8>> {
    use crate::services::csv_helpers::{neutralize_formula, sen_to_plain_rm};

    let run = payroll_runs::get_for_company(pool, run_id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))?;
    if !matches!(run.status.as_str(), "approved" | "paid") {
        return Err(AppError::BadRequest(
            "Payment file is only available for approved or paid payroll runs".into(),
        ));
    }

    let rows = payroll_reads::payment_file_rows(pool, run_id).await?;
    let mut wtr = csv::Writer::from_writer(Vec::new());
    wtr.write_record([
        "Employee No",
        "Employee Name",
        "Bank",
        "Account No",
        "Account Type",
        "Net Pay (RM)",
    ])
    .map_err(|e| AppError::Internal(format!("CSV error: {e}")))?;
    for row in &rows {
        wtr.write_record([
            neutralize_formula(&row.employee_number),
            neutralize_formula(&row.employee_name),
            neutralize_formula(row.bank_name.as_deref().unwrap_or("")),
            neutralize_formula(row.bank_account_number.as_deref().unwrap_or("")),
            neutralize_formula(row.bank_account_type.as_deref().unwrap_or("")),
            sen_to_plain_rm(row.net_salary),
        ])
        .map_err(|e| AppError::Internal(format!("CSV error: {e}")))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| AppError::Internal(format!("CSV error: {e}")))?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_user_id),
        "export_payment_file",
        "payroll_run",
        Some(run_id),
        None,
        Some(serde_json::json!({
            "period": format!("{:02}/{}", run.period_month, run.period_year),
            "employee_count": rows.len(),
        })),
        Some(&format!(
            "Exported payment file for {:02}/{} ({} employees)",
            run.period_month,
            run.period_year,
            rows.len()
        )),
        audit_meta,
    )
    .await;

    Ok(bytes)
}

// ─── Journal preview ───

/// Balanced journal preview for an approved/paid run, computed from the run's
/// stored totals. Writes nothing; account codes are fixed defaults until a
/// configurable chart of accounts exists.
///
/// Shape: DR gross wages + employer statutory expenses (allocated by
/// department), CR each statutory payable and net salaries payable.
/// `total_gross - total_net` equals employee-side deductions by construction
/// of `payroll_items`, so the preview balances when the run's committed
/// figures are internally consistent.
pub async fn journal_preview(
    pool: &PgPool,
    company_id: Uuid,
    run_id: Uuid,
) -> AppResult<JournalPreview> {
    let run = payroll_runs::get_for_company(pool, run_id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))?;
    if !matches!(run.status.as_str(), "approved" | "paid") {
        return Err(AppError::BadRequest(
            "Journal preview is only available for approved or paid payroll runs".into(),
        ));
    }

    let departments = payroll_reads::run_department_totals(pool, run_id).await?;
    let mut lines: Vec<JournalLine> = Vec::new();
    let mut push = |side: &str,
                    code: &str,
                    name: &str,
                    department: Option<String>,
                    amount: i64,
                    memo: Option<String>| {
        if amount != 0 {
            lines.push(JournalLine {
                side: side.into(),
                account_code: code.into(),
                account_name: name.into(),
                department,
                amount,
                memo,
            });
        }
    };
    macro_rules! debit {
        ($code:expr, $name:expr, $dept:expr, $amount:expr, $memo:expr $(,)?) => {
            push("debit", $code, $name, $dept, $amount, $memo)
        };
    }
    macro_rules! credit {
        ($code:expr, $name:expr, $amount:expr, $memo:expr $(,)?) => {
            push("credit", $code, $name, None, $amount, $memo)
        };
    }

    for dept in &departments {
        debit!(
            "5000",
            "Salaries & wages",
            Some(
                dept.department
                    .clone()
                    .unwrap_or_else(|| "Unallocated".into())
            ),
            dept.total_gross,
            Some(format!("{} employees", dept.employee_count)),
        );
    }
    debit!(
        "5010",
        "Employer EPF expense",
        None,
        run.total_epf_employer,
        None
    );
    debit!(
        "5020",
        "Employer SOCSO expense",
        None,
        run.total_socso_employer,
        None
    );
    debit!(
        "5030",
        "Employer EIS expense",
        None,
        run.total_eis_employer,
        None
    );

    credit!(
        "2100",
        "EPF payable",
        run.total_epf_employee + run.total_epf_employer,
        Some("Employee + employer portions".into())
    );
    credit!(
        "2110",
        "SOCSO payable",
        run.total_socso_employee + run.total_socso_employer,
        None
    );
    credit!(
        "2120",
        "EIS payable",
        run.total_eis_employee + run.total_eis_employer,
        None
    );
    credit!("2130", "PCB (MTD) payable", run.total_pcb, None);
    credit!("2140", "Zakat payable", run.total_zakat, None);
    credit!("2200", "Net salaries payable", run.total_net, None);

    let total_debits: i64 = lines
        .iter()
        .filter(|l| l.side == "debit")
        .map(|l| l.amount)
        .sum();
    let total_credits: i64 = lines
        .iter()
        .filter(|l| l.side == "credit")
        .map(|l| l.amount)
        .sum();

    Ok(JournalPreview {
        payroll_run_id: run_id,
        period_year: run.period_year,
        period_month: run.period_month,
        lines,
        total_debits,
        total_credits,
        balanced: total_debits == total_credits && !total_debits.is_zero(),
        notes: vec![
            "Account codes are fixed defaults — no configurable chart of accounts exists yet".into(),
            "Non-statutory employee deductions (loans, advances, claims offsets) are netted inside net salaries payable".into(),
            "Posting is manual: transcribe or export to the accounting system; nothing is written to a ledger".into(),
        ],
    })
}
