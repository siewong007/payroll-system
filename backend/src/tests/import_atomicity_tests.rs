//! Regression tests for `employee_import_service::confirm_import`.
//!
//! The bug these pin down: the import ran every row through one transaction
//! with no savepoint. A single failing INSERT put the connection into
//! Postgres' aborted-transaction state, every later row failed too, and
//! `COMMIT` on an aborted block is silently executed as ROLLBACK *and reported
//! as success*. The endpoint answered `{imported_count: N}` having written
//! nothing, then marked the session confirmed on a separate connection so it
//! could never be retried.

use uuid::Uuid;

use crate::models::employee_import::{
    ImportConfirmRequest, ImportRowRaw, ImportRowValidation, RowStatus,
};
use crate::repositories::bulk_import_sessions;
use crate::services::employee_import_service;
use crate::tests::support::{seed_company, seed_user, skip_if_no_db};

/// A row with every optional column null, so tests set only what they care about.
fn row(
    row_number: usize,
    employee_number: &str,
    payroll_group_id: Option<Uuid>,
) -> ImportRowValidation {
    ImportRowValidation {
        row_number,
        status: RowStatus::Valid,
        errors: Vec::new(),
        data: ImportRowRaw {
            row_number,
            employee_number: Some(employee_number.to_string()),
            full_name: Some(format!("Imported {employee_number}")),
            ic_number: None,
            passport_number: None,
            date_of_birth: Some("1990-01-01".into()),
            gender: None,
            nationality: None,
            race: None,
            residency_status: None,
            marital_status: None,
            email: None,
            phone: None,
            address_line1: None,
            address_line2: None,
            city: None,
            state: None,
            postcode: None,
            department: None,
            designation: None,
            cost_centre: None,
            branch: None,
            employment_type: None,
            date_joined: Some("2024-01-01".into()),
            probation_start: None,
            probation_end: None,
            basic_salary: Some("3000.00".into()),
            hourly_rate: None,
            daily_rate: None,
            bank_name: None,
            bank_account_number: None,
            bank_account_type: None,
            tax_identification_number: None,
            epf_number: None,
            socso_number: None,
            eis_number: None,
            working_spouse: None,
            num_children: None,
            epf_category: None,
            is_muslim: None,
            zakat_eligible: None,
            zakat_monthly_amount: None,
            ptptn_monthly_amount: None,
            tabung_haji_amount: None,
            payroll_group_id: payroll_group_id.map(|id| id.to_string()),
            salary_group: None,
        },
    }
}

async fn stage_session(
    pool: &sqlx::PgPool,
    company_id: Uuid,
    user_id: Uuid,
    rows: &[ImportRowValidation],
) -> Uuid {
    let session_id = Uuid::now_v7();
    bulk_import_sessions::insert_pending(
        pool,
        session_id,
        company_id,
        user_id,
        "test-import.csv",
        rows.len() as i32,
        rows.len() as i32,
        serde_json::to_value(rows).expect("serialize rows"),
    )
    .await
    .expect("stage import session");
    session_id
}

async fn employee_count(pool: &sqlx::PgPool, company_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM employees WHERE company_id = $1")
        .bind(company_id)
        .fetch_one(pool)
        .await
        .expect("count employees")
}

/// The reported count must match what is actually in the table.
///
/// Row 2 carries a well-formed but nonexistent `payroll_group_id`, which the
/// validator does not catch (it only checks that the value parses as a UUID)
/// but `employees_payroll_group_tenant_fkey` rejects. Before the fix this
/// returned `imported_count: 2` with zero rows committed.
#[tokio::test]
async fn a_failing_row_does_not_discard_the_rest_of_the_batch() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let rows = vec![
        row(1, "IMP-001", None),
        row(2, "IMP-002", Some(Uuid::now_v7())), // FK violation
        row(3, "IMP-003", None),
    ];
    let session_id = stage_session(&pool, company_id, user_id, &rows).await;

    let response = employee_import_service::confirm_import(
        &pool,
        company_id,
        user_id,
        ImportConfirmRequest {
            session_id,
            skip_invalid: true,
        },
        None,
    )
    .await
    .expect("confirm import");

    assert_eq!(response.imported_count, 2, "two rows were importable");
    assert_eq!(response.errors.len(), 1, "the bad row is reported");
    assert_eq!(
        employee_count(&pool, company_id).await,
        2,
        "the reported count must match the rows actually committed"
    );
}

/// The per-row error must not carry raw Postgres text (table, column and index
/// names) to the client.
#[tokio::test]
async fn row_errors_do_not_leak_database_internals() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let rows = vec![row(1, "LEAK-001", Some(Uuid::now_v7()))];
    let session_id = stage_session(&pool, company_id, user_id, &rows).await;

    let response = employee_import_service::confirm_import(
        &pool,
        company_id,
        user_id,
        ImportConfirmRequest {
            session_id,
            skip_invalid: true,
        },
        None,
    )
    .await
    .expect("confirm import");

    let message = &response.errors[0].errors[0].message;
    assert!(
        !message.contains("employees_payroll_group_tenant_fkey")
            && !message.to_lowercase().contains("violates"),
        "row error leaked the Postgres message: {message}"
    );
}

/// A successful import must be auditable, scoped to the company.
///
/// The old writer omitted `company_id`, and every audit read filters on it — so
/// the row existed but no API could ever return it.
#[tokio::test]
async fn a_successful_import_writes_a_company_scoped_audit_row() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let rows = vec![row(1, "AUD-001", None)];
    let session_id = stage_session(&pool, company_id, user_id, &rows).await;

    employee_import_service::confirm_import(
        &pool,
        company_id,
        user_id,
        ImportConfirmRequest {
            session_id,
            skip_invalid: true,
        },
        None,
    )
    .await
    .expect("confirm import");

    let audited: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_logs WHERE company_id = $1 AND action = 'bulk_import'",
    )
    .bind(company_id)
    .fetch_one(&pool)
    .await
    .expect("count audit rows");

    assert_eq!(
        audited, 1,
        "the import must be visible in the company's trail"
    );
}

/// Replaying a confirmed session must not import the file twice.
#[tokio::test]
async fn a_confirmed_session_cannot_be_replayed() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    let rows = vec![row(1, "REPLAY-001", None)];
    let session_id = stage_session(&pool, company_id, user_id, &rows).await;

    let request = || ImportConfirmRequest {
        session_id,
        skip_invalid: true,
    };

    employee_import_service::confirm_import(&pool, company_id, user_id, request(), None)
        .await
        .expect("first confirm succeeds");

    let replay =
        employee_import_service::confirm_import(&pool, company_id, user_id, request(), None).await;

    assert!(replay.is_err(), "a confirmed session must be rejected");
    assert_eq!(
        employee_count(&pool, company_id).await,
        1,
        "the replay must not have imported the file a second time"
    );
}

/// A failed import must leave the session replayable.
///
/// `mark_confirmed` used to run outside the transaction, so an import that
/// wrote nothing still burned the session and the operator's only recovery was
/// to re-upload the file — which the API gave them no reason to do.
#[tokio::test]
async fn a_rejected_import_leaves_the_session_pending() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let user_id = seed_user(&pool, company_id, "payroll_admin").await;

    // skip_invalid = false, so the FK violation aborts the whole import.
    let rows = vec![row(1, "FAIL-001", Some(Uuid::now_v7()))];
    let session_id = stage_session(&pool, company_id, user_id, &rows).await;

    let result = employee_import_service::confirm_import(
        &pool,
        company_id,
        user_id,
        ImportConfirmRequest {
            session_id,
            skip_invalid: false,
        },
        None,
    )
    .await;

    assert!(result.is_err(), "the import must fail");
    assert_eq!(
        employee_count(&pool, company_id).await,
        0,
        "nothing may be committed"
    );

    let status: String =
        sqlx::query_scalar("SELECT status FROM bulk_import_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(&pool)
            .await
            .expect("read session status");
    assert_eq!(status, "pending", "the session must stay retryable");
}
