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
