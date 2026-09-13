use chrono::{Duration, NaiveDate, Utc};
use uuid::Uuid;

use crate::core::auth::{AuthUser, Claims, JWT_AUDIENCE, JWT_ISSUER};
use crate::services::{payroll_engine, payroll_lifecycle_service};
use crate::tests::support::{
    seed_company, seed_employee, seed_payroll_group, seed_user, skip_if_no_db,
};

/// An `AuthUser` as the extractor would build it for `user_id` — the cancel
/// transition gates on the caller's permission set, not a bare user id.
fn auth_for(user_id: Uuid, company_id: Uuid, roles: &[&str]) -> AuthUser {
    AuthUser(
        Claims {
            sub: user_id,
            email: "person@example.test".into(),
            roles: roles.iter().map(|role| (*role).to_string()).collect(),
            company_id: Some(company_id),
            employee_id: None,
            sid: Uuid::new_v4(),
            exp: (Utc::now() + Duration::hours(1)).timestamp(),
            iat: Utc::now().timestamp(),
            iss: JWT_ISSUER.into(),
            aud: JWT_AUDIENCE.into(),
        },
        Vec::new(),
    )
}

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

#[tokio::test]
async fn cancel_processed_run_records_who_and_why_and_audits() {
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
    .expect("process payroll");

    let auth = auth_for(user_id, company_id, &["payroll_admin"]);
    let cancelled = payroll_lifecycle_service::cancel(
        &pool,
        company_id,
        run.id,
        &auth,
        Some("Wrong allowance table applied".into()),
        None,
    )
    .await
    .expect("cancel processed run");

    assert_eq!(cancelled.status, "cancelled");
    assert_eq!(cancelled.cancelled_by, Some(user_id));
    assert!(cancelled.cancelled_at.is_some());
    assert_eq!(
        cancelled.cancel_reason.as_deref(),
        Some("Wrong allowance table applied")
    );

    // Claims this run reimbursed are payable again.
    let paid_claims: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM claims c JOIN employees e ON c.employee_id = e.id
         WHERE e.company_id = $1 AND c.payroll_run_id IS NOT NULL AND c.status = 'processed'",
    )
    .bind(company_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(0);
    assert_eq!(paid_claims, 0);

    let actions: Vec<String> = sqlx::query_scalar(
        r#"SELECT action FROM audit_logs
        WHERE company_id = $1 AND entity_type = 'payroll_run' AND entity_id = $2"#,
    )
    .bind(company_id)
    .bind(run.id)
    .fetch_all(&pool)
    .await
    .expect("audit actions");
    assert!(actions.contains(&"cancel".to_string()));

    // The period is free again — cancellation releases the one-active-run slot.
    let rerun = payroll_engine::process_payroll(
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
    .expect("re-process after cancel");
    assert_eq!(rerun.status, "processed");
}

#[tokio::test]
async fn cancel_pending_approval_belongs_to_the_approver_not_the_preparer() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let _employee_id = seed_employee(&pool, company_id, Some(group_id), 400_000).await;
    let preparer_id = seed_user(&pool, company_id, "payroll_admin").await;
    let approver_id = seed_user(&pool, company_id, "finance").await;

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        8,
        NaiveDate::from_ymd_opt(2024, 9, 5).unwrap(),
        preparer_id,
        None,
        None,
    )
    .await
    .expect("process payroll");
    payroll_lifecycle_service::submit_for_approval(&pool, company_id, run.id, preparer_id, None)
        .await
        .expect("submit payroll");

    // The preparer cannot withdraw a run that is under review.
    let denied = payroll_lifecycle_service::cancel(
        &pool,
        company_id,
        run.id,
        &auth_for(preparer_id, company_id, &["payroll_admin"]),
        Some("reconsider".into()),
        None,
    )
    .await;
    assert!(denied.is_err(), "preparer must not cancel under review");

    // The approver can.
    let cancelled = payroll_lifecycle_service::cancel(
        &pool,
        company_id,
        run.id,
        &auth_for(approver_id, company_id, &["finance"]),
        Some("Payroll group mapped to wrong cost centre".into()),
        None,
    )
    .await
    .expect("approver cancels under review");
    assert_eq!(cancelled.status, "cancelled");
}

#[tokio::test]
async fn cancel_refuses_paid_and_cancel_requires_a_reason() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let _employee_id = seed_employee(&pool, company_id, Some(group_id), 400_000).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;
    let approver_id = seed_user(&pool, company_id, "finance").await;

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
    .expect("process payroll");
    payroll_lifecycle_service::submit_for_approval(&pool, company_id, run.id, user_id, None)
        .await
        .expect("submit payroll");
    payroll_lifecycle_service::approve(&pool, company_id, run.id, user_id, None)
        .await
        .expect("approve payroll");
    payroll_lifecycle_service::lock_as_paid(&pool, company_id, run.id, user_id, None)
        .await
        .expect("lock payroll");

    let auth = auth_for(approver_id, company_id, &["finance"]);
    let err = payroll_lifecycle_service::cancel(
        &pool,
        company_id,
        run.id,
        &auth,
        Some("too late".into()),
        None,
    )
    .await
    .expect_err("plain cancel must refuse a paid run");
    assert!(format!("{err}").contains("reverse"), "{err}");

    // Reason is mandatory at every cancellable status.
    let run2 = payroll_engine::process_payroll(
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
    .expect("process second payroll");
    let no_reason = payroll_lifecycle_service::cancel(
        &pool,
        company_id,
        run2.id,
        &auth_for(user_id, company_id, &["payroll_admin"]),
        None,
        None,
    )
    .await;
    assert!(no_reason.is_err(), "cancel without reason must fail");
    let blank_reason = payroll_lifecycle_service::cancel(
        &pool,
        company_id,
        run2.id,
        &auth_for(user_id, company_id, &["payroll_admin"]),
        Some("   ".into()),
        None,
    )
    .await;
    assert!(blank_reason.is_err(), "whitespace-only reason must fail");
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
    let finance_id = seed_user(&pool, company_id, "finance").await;

    let run = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        11,
        NaiveDate::from_ymd_opt(2024, 12, 5).unwrap(),
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

    // Reversal belongs to the paying role, not the preparer.
    let denied = payroll_lifecycle_service::reverse_paid_run(
        &pool,
        company_id,
        run.id,
        &auth_for(user_id, company_id, &["payroll_admin"]),
        Some("recall".into()),
        None,
    )
    .await;
    assert!(denied.is_err(), "preparer must not reverse a paid run");

    let reversed = payroll_lifecycle_service::reverse_paid_run(
        &pool,
        company_id,
        run.id,
        &auth_for(finance_id, company_id, &["finance"]),
        Some("bank file recalled".into()),
        None,
    )
    .await
    .expect("reverse");
    assert_eq!(reversed.status, "cancelled");
    assert_eq!(reversed.cancelled_by, Some(finance_id));
    assert_eq!(
        reversed.cancel_reason.as_deref(),
        Some("bank file recalled")
    );

    // The period is free again.
    let rerun = payroll_engine::process_payroll(
        &pool,
        company_id,
        group_id,
        2024,
        11,
        NaiveDate::from_ymd_opt(2024, 12, 6).unwrap(),
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
