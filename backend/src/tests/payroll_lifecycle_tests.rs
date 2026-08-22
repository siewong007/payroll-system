use chrono::NaiveDate;

use crate::services::{payroll_engine, payroll_lifecycle_service};
use crate::tests::support::{
    seed_company, seed_employee, seed_payroll_group, seed_user, skip_if_no_db,
};

#[tokio::test]
async fn payroll_lifecycle_submit_approve_and_lock() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let _employee_id = seed_employee(&pool, company_id, Some(group_id), 450_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

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
    .expect("process payroll");
    assert_eq!(run.status, "processed");

    let submitted =
        payroll_lifecycle_service::submit_for_approval(&pool, company_id, run.id, user_id, None)
            .await
            .expect("submit payroll");
    assert_eq!(submitted.status, "pending_approval");

    let approved = payroll_lifecycle_service::approve(&pool, company_id, run.id, user_id, None)
        .await
        .expect("approve payroll");
    assert_eq!(approved.status, "approved");
    assert_eq!(approved.approved_by, Some(user_id));
    assert!(approved.approved_at.is_some());

    let paid = payroll_lifecycle_service::lock_as_paid(&pool, company_id, run.id, user_id, None)
        .await
        .expect("lock payroll");
    assert_eq!(paid.status, "paid");
    assert_eq!(paid.locked_by, Some(user_id));
    assert!(paid.locked_at.is_some());

    let actions: Vec<String> = sqlx::query_scalar(
        r#"SELECT action
        FROM audit_logs
        WHERE company_id = $1 AND entity_type = 'payroll_run' AND entity_id = $2
        ORDER BY created_at"#,
    )
    .bind(company_id)
    .bind(run.id)
    .fetch_all(&pool)
    .await
    .expect("audit actions");

    assert!(actions.contains(&"submit_approval".to_string()));
    assert!(actions.contains(&"approve".to_string()));
    assert!(actions.contains(&"lock".to_string()));
}

#[tokio::test]
async fn payroll_lifecycle_return_for_changes_reopens_processed_run() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let _employee_id = seed_employee(&pool, company_id, Some(group_id), 400_000).await;
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
    .expect("process payroll");

    payroll_lifecycle_service::submit_for_approval(&pool, company_id, run.id, user_id, None)
        .await
        .expect("submit payroll");

    let reopened = payroll_lifecycle_service::return_for_changes(
        &pool,
        company_id,
        run.id,
        user_id,
        Some("PCB needs review".into()),
        None,
    )
    .await
    .expect("return payroll");

    assert_eq!(reopened.status, "processed");

    let reason: Option<String> = sqlx::query_scalar(
        r#"SELECT new_values->>'reason'
        FROM audit_logs
        WHERE company_id = $1
          AND entity_type = 'payroll_run'
          AND entity_id = $2
          AND action = 'return_changes'
        ORDER BY created_at DESC
        LIMIT 1"#,
    )
    .bind(company_id)
    .bind(run.id)
    .fetch_one(&pool)
    .await
    .expect("return reason");

    assert_eq!(reason.as_deref(), Some("PCB needs review"));
}

/// Cancellation frees the period for a re-run and releases what the run
/// consumed: claims return to approved, staged entries become unprocessed
/// (plan item 11).
#[tokio::test]
async fn cancelling_a_processed_run_releases_the_period_and_its_sources() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let _employee_id = seed_employee(&pool, company_id, Some(group_id), 450_000).await;
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
    .expect("process payroll");

    let cancelled = payroll_lifecycle_service::cancel_run(
        &pool,
        company_id,
        run.id,
        user_id,
        Some("wrong population".into()),
        None,
    )
    .await
    .expect("cancel");
    assert_eq!(cancelled.status, "cancelled");

    // Claims this run reimbursed are payable again…
    let paid_claims: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM claims c JOIN employees e ON c.employee_id = e.id
         WHERE e.company_id = $1 AND c.payroll_run_id IS NOT NULL AND c.status = 'processed'",
    )
    .bind(company_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(0);
    assert_eq!(paid_claims, 0);

    // …and the period accepts a new run (the unique index exempts 'cancelled').
    let rerun = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        5,
        NaiveDate::from_ymd_opt(2024, 6, 6).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("re-run after cancellation must be possible");
    assert_eq!(rerun.status, "processed");
    assert_eq!(rerun.employee_count, 1);
}

/// A paid run can only go back out through reversal, which releases its
/// sources in the same transaction as the status change; a plain cancel is
/// refused for paid runs.
#[tokio::test]
async fn only_reversal_can_void_a_paid_run() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let _employee_id = seed_employee(&pool, company_id, Some(group_id), 450_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

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
    .expect("process");
    payroll_lifecycle_service::submit_for_approval(&pool, company_id, run.id, user_id, None)
        .await
        .expect("submit");
    payroll_lifecycle_service::approve(&pool, company_id, run.id, user_id, None)
        .await
        .expect("approve");
    payroll_lifecycle_service::lock_as_paid(&pool, company_id, run.id, user_id, None)
        .await
        .expect("lock");

    let err = payroll_lifecycle_service::cancel_run(&pool, company_id, run.id, user_id, None, None)
        .await
        .expect_err("paid runs cannot simply be cancelled");
    assert!(format!("{err}").contains("reverse"), "{err}");

    let reversed = payroll_lifecycle_service::reverse_paid_run(
        &pool,
        company_id,
        run.id,
        user_id,
        Some("bank file recalled".into()),
        None,
    )
    .await
    .expect("reverse");
    assert_eq!(reversed.status, "cancelled");

    // The period is free again.
    let rerun = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        7,
        NaiveDate::from_ymd_opt(2024, 8, 6).unwrap(),
        user_id,
        None,
        None,
    )
    .await
    .expect("re-run after reversal");
    assert_eq!(rerun.employee_count, 1);

    // Both the reversal itself and the reason are on the trail.
    let has_reverse: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM audit_logs
           WHERE entity_type = 'payroll_run' AND entity_id = $1
             AND action = 'reverse'
             AND old_values->>'status' = 'paid'"#,
    )
    .bind(run.id)
    .fetch_one(&pool)
    .await
    .expect("audit lookup");
    assert_eq!(has_reverse, 1);
}
