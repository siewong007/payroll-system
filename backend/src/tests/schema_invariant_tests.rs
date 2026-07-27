use chrono::NaiveDate;
use uuid::Uuid;

use crate::tests::support::{seed_company, seed_employee, seed_payroll_group, skip_if_no_db};

fn constraint_name(error: &sqlx::Error) -> Option<&str> {
    error.as_database_error().and_then(|db| db.constraint())
}

#[tokio::test]
async fn tenant_scoped_foreign_keys_reject_cross_company_rows() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_a = seed_company(&pool).await;
    let company_b = seed_company(&pool).await;
    let employee_b = seed_employee(&pool, company_b, None, 300_000).await;

    let claim_error = sqlx::query(
        r#"
        INSERT INTO claims (
            employee_id, company_id, title, amount, expense_date
        ) VALUES ($1, $2, 'Cross-tenant claim', 100, $3)
        "#,
    )
    .bind(employee_b)
    .bind(company_a)
    .bind(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
    .execute(&pool)
    .await
    .unwrap_err();

    assert_eq!(
        constraint_name(&claim_error),
        Some("claims_employee_tenant_fkey")
    );
}

#[tokio::test]
async fn companyless_junction_trigger_rejects_cross_tenant_leave_balance() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_a = seed_company(&pool).await;
    let company_b = seed_company(&pool).await;
    let employee_b = seed_employee(&pool, company_b, None, 300_000).await;
    let leave_type_a: uuid::Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO leave_types (company_id, name, default_days)
        VALUES ($1, 'Tenant invariant leave', 1)
        RETURNING id
        "#,
    )
    .bind(company_a)
    .fetch_one(&pool)
    .await
    .unwrap();

    let balance_error = sqlx::query(
        r#"
        INSERT INTO leave_balances (employee_id, leave_type_id, year)
        VALUES ($1, $2, 2026)
        "#,
    )
    .bind(employee_b)
    .bind(leave_type_a)
    .execute(&pool)
    .await
    .unwrap_err();

    assert_eq!(
        constraint_name(&balance_error),
        Some("leave_balances_same_company_check")
    );
}

#[tokio::test]
async fn legacy_prototype_statutory_datasets_cannot_be_verified() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let error = sqlx::query(
        r#"
        UPDATE statutory_rule_sets
        SET status = 'verified',
            source_url = 'https://example.invalid/not-official',
            source_version = 'invalid-test',
            source_sha256 = repeat('f', 64),
            verified_at = NOW()
        WHERE dataset_key = 'legacy-prototype-epf-2024'
        "#,
    )
    .execute(&pool)
    .await
    .unwrap_err();

    assert_eq!(
        constraint_name(&error),
        Some("statutory_rule_sets_legacy_never_verified_check")
    );
}

#[tokio::test]
async fn statutory_dataset_rejects_overlapping_wage_bands() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let rule_set_id: uuid::Uuid = sqlx::query_scalar(
        "SELECT id FROM statutory_rule_sets WHERE dataset_key = 'test-fixture-epf-2024'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let error = sqlx::query(
        r#"
        INSERT INTO epf_rates (
            rule_set_id, category, wage_from, wage_to,
            employee_contribution, employer_contribution, effective_from
        ) VALUES ($1, 'A', 105000, 115000, 1, 1, '2024-01-01')
        "#,
    )
    .bind(rule_set_id)
    .execute(&pool)
    .await
    .unwrap_err();

    assert_eq!(
        constraint_name(&error),
        Some("epf_rates_no_overlapping_bands")
    );
}

/// Every permission key stored on a group must still resolve to a `Permission`.
///
/// `user_group_permissions.permission` is free text by design — a CHECK
/// constraint would be a second copy of the enum needing a migration per
/// capability. That makes the service-layer validation the only gate, so this
/// asserts nothing has slipped past it: a stale key grants nothing while
/// looking, in the UI, exactly like a permission that does not work.
#[tokio::test]
async fn user_group_permissions_known_keys() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let stored: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT permission FROM user_group_permissions")
            .fetch_all(&pool)
            .await
            .expect("read stored group permissions");

    let known: Vec<&str> = crate::core::permission::Permission::ALL
        .iter()
        .map(|p| p.as_str())
        .collect();

    let unknown: Vec<&String> = stored
        .iter()
        .filter(|key| !known.contains(&key.as_str()))
        .collect();

    assert!(
        unknown.is_empty(),
        "group permission keys no longer in Permission::ALL: {unknown:?}"
    );
}

/// A group grants inside one company, so its members must belong to that
/// company — otherwise adding a user from company B would hand them company A's
/// capabilities the moment they switched context.
#[tokio::test]
async fn user_group_members_must_share_the_group_company() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_a = crate::tests::support::seed_company(&pool).await;
    let company_b = crate::tests::support::seed_company(&pool).await;
    let owner = crate::tests::support::seed_user(&pool, company_a, "admin").await;
    // A user whose only company is B.
    let outsider = crate::tests::support::seed_user(&pool, company_b, "admin").await;
    sqlx::query("INSERT INTO user_companies (user_id, company_id) VALUES ($1, $2)")
        .bind(outsider)
        .bind(company_b)
        .execute(&pool)
        .await
        .expect("link outsider to company B");

    let group_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO user_groups (company_id, name, created_by) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(company_a)
    .bind(format!("Group-{}", &uuid::Uuid::new_v4().to_string()[..8]))
    .bind(owner)
    .fetch_one(&pool)
    .await
    .expect("create group in company A");

    let error = sqlx::query("INSERT INTO user_group_members (group_id, user_id) VALUES ($1, $2)")
        .bind(group_id)
        .bind(outsider)
        .execute(&pool)
        .await
        .unwrap_err();

    assert_eq!(
        constraint_name(&error),
        Some("user_group_members_same_company_check")
    );
}

/// Overtime is derived from the hours it sits inside, so it can never exceed
/// them. Both application write paths compute `GREATEST(0, elapsed - shift)`
/// against `hours_worked = elapsed` and already satisfy this; the constraint is
/// what stops a future write path — or a hand-run UPDATE — reintroducing the
/// inflated figure that made a forgotten check-out pay most of a day.
#[tokio::test]
async fn overtime_hours_cannot_exceed_hours_worked() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 300_000).await;

    let error = sqlx::query(
        r#"
        INSERT INTO attendance_records (
            company_id, employee_id, check_in_at, check_out_at, method, status,
            hours_worked, overtime_hours
        ) VALUES ($1, $2, NOW() - INTERVAL '8 hours', NOW(), 'manual', 'present', 8, 14)
        "#,
    )
    .bind(company_id)
    .bind(employee_id)
    .execute(&pool)
    .await
    .unwrap_err();

    assert_eq!(
        constraint_name(&error),
        Some("attendance_records_overtime_within_hours_check")
    );
}

/// Declared overtime hours are multiplied by an hourly rate to stage a payroll
/// earning, so an unbounded value pays out arbitrary money and a negative one
/// stages a negative earning. The service rule is tighter — it caps against the
/// window the applicant declared, which the database cannot see — but this is
/// the outer bound that rule already implies, and the backstop for a write path
/// that forgets to call it.
#[tokio::test]
async fn overtime_hours_check_rejects_out_of_range_insert() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 300_000).await;

    for hours in ["0", "-50", "25"] {
        let error = sqlx::query(&format!(
            r#"
            INSERT INTO overtime_applications (
                employee_id, company_id, ot_date, start_time, end_time, hours, ot_type
            ) VALUES ($1, $2, $3, TIME '09:00', TIME '11:00', {hours}, 'normal')
            "#
        ))
        .bind(employee_id)
        .bind(company_id)
        .bind(NaiveDate::from_ymd_opt(2026, 3, 4).unwrap())
        .execute(&pool)
        .await
        .unwrap_err();

        assert_eq!(
            constraint_name(&error),
            Some("overtime_applications_hours_check"),
            "hours = {hours} should violate the bound"
        );
    }

    // The boundary itself is legal: a 24 h window is expressible (a shift whose
    // end time wraps all the way round), so 24 must not be rejected.
    sqlx::query(
        r#"
        INSERT INTO overtime_applications (
            employee_id, company_id, ot_date, start_time, end_time, hours, ot_type
        ) VALUES ($1, $2, $3, TIME '09:00', TIME '09:00', 24, 'normal')
        "#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(NaiveDate::from_ymd_opt(2026, 3, 5).unwrap())
    .execute(&pool)
    .await
    .expect("24 hours is the inclusive upper bound, not a violation");
}

/// Seed a leave type so an `attachment_url` can be attached to a leave request.
async fn seed_leave_type(pool: &sqlx::PgPool, company_id: uuid::Uuid) -> uuid::Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO leave_types (company_id, name, default_days)
        VALUES ($1, $2, 1)
        RETURNING id
        "#,
    )
    .bind(company_id)
    .bind(format!(
        "Upload guard leave {}",
        &uuid::Uuid::new_v4().to_string()[..8]
    ))
    .fetch_one(pool)
    .await
    .expect("create leave type")
}

/// The three `*_url` columns that get joined onto a filesystem path must not be
/// able to hold a traversal payload.
///
/// `core::upload_path::validate_file_url` is the real gate; these CHECKs are the
/// backstop for a future write path that forgets to call it — which is exactly
/// how the original defect arose, with four sinks and one ad-hoc guard between
/// them.
#[tokio::test]
async fn upload_url_columns_reject_traversal_payloads() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company = seed_company(&pool).await;
    let employee = seed_employee(&pool, company, None, 300_000).await;
    let leave_type = seed_leave_type(&pool, company).await;

    let document_error = sqlx::query(
        r#"
        INSERT INTO documents (company_id, title, file_name, file_url)
        VALUES ($1, 'Traversal', 'key.pem', '/api/uploads//etc/ssl/private/key.pem')
        "#,
    )
    .bind(company)
    .execute(&pool)
    .await
    .unwrap_err();

    assert_eq!(
        constraint_name(&document_error),
        Some("documents_file_url_no_traversal")
    );

    let leave_error = sqlx::query(
        r#"
        INSERT INTO leave_requests (
            employee_id, company_id, leave_type_id, start_date, end_date, days, attachment_url
        ) VALUES ($1, $2, $3, $4, $4, 1, '/api/uploads/../../app/.env')
        "#,
    )
    .bind(employee)
    .bind(company)
    .bind(leave_type)
    .bind(NaiveDate::from_ymd_opt(2026, 3, 2).unwrap())
    .execute(&pool)
    .await
    .unwrap_err();

    assert_eq!(
        constraint_name(&leave_error),
        Some("leave_requests_attachment_url_no_traversal")
    );

    let claim_error = sqlx::query(
        r#"
        INSERT INTO claims (
            employee_id, company_id, title, amount, expense_date, receipt_url
        ) VALUES ($1, $2, 'Traversal', 100, $3, '/api/uploads/..\..\app\.env')
        "#,
    )
    .bind(employee)
    .bind(company)
    .bind(NaiveDate::from_ymd_opt(2026, 3, 2).unwrap())
    .execute(&pool)
    .await
    .unwrap_err();

    assert_eq!(
        constraint_name(&claim_error),
        Some("claims_receipt_url_no_traversal")
    );
}

/// The constraint must not reject anything the application legitimately writes:
/// a stored upload, an external link, or no attachment at all.
#[tokio::test]
async fn upload_url_columns_accept_stored_uploads_external_links_and_null() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company = seed_company(&pool).await;
    let employee = seed_employee(&pool, company, None, 300_000).await;
    let leave_type = seed_leave_type(&pool, company).await;

    for url in [
        "/api/uploads/0198f2c4-1f3a-7c21-9b0e-2f6a1c8d4e55_offer_letter.pdf",
        "https://example.com/handbook.pdf",
    ] {
        sqlx::query(
            r#"
            INSERT INTO documents (company_id, title, file_name, file_url)
            VALUES ($1, 'Permitted', 'permitted.pdf', $2)
            "#,
        )
        .bind(company)
        .bind(url)
        .execute(&pool)
        .await
        .unwrap_or_else(|e| panic!("documents rejected permitted file_url {url:?}: {e}"));
    }

    sqlx::query(
        r#"
        INSERT INTO leave_requests (
            employee_id, company_id, leave_type_id, start_date, end_date, days, attachment_url
        ) VALUES ($1, $2, $3, $4, $4, 1, NULL)
        "#,
    )
    .bind(employee)
    .bind(company)
    .bind(leave_type)
    .bind(NaiveDate::from_ymd_opt(2026, 3, 3).unwrap())
    .execute(&pool)
    .await
    .expect("leave_requests rejected a NULL attachment_url");

    sqlx::query(
        r#"
        INSERT INTO claims (
            employee_id, company_id, title, amount, expense_date, receipt_url
        ) VALUES ($1, $2, 'Permitted', 100, $3, '/api/uploads/receipt.png')
        "#,
    )
    .bind(employee)
    .bind(company)
    .bind(NaiveDate::from_ymd_opt(2026, 3, 3).unwrap())
    .execute(&pool)
    .await
    .expect("claims rejected a permitted receipt_url");
}

// ─── Company teardown (R1-H5) ───

/// Every table holding a NO ACTION foreign key into `companies` must be named in
/// `companies::delete_company_data`, or `DELETE /api/admin/companies/{id}` raises
/// 23503 and returns an opaque 500 for any tenant that ever wrote such a row.
///
/// Reflecting over `pg_constraint` rather than restating a list is the point: a
/// table added by a later migration joins this assertion automatically. `'a'` is
/// NO ACTION and `'r'` is RESTRICT — both block the parent delete. Anything
/// CASCADE or SET NULL is the database's problem, not the wipe order's.
#[tokio::test]
async fn the_company_wipe_order_covers_every_blocking_foreign_key() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let wipe_source = include_str!("../repositories/companies.rs");

    let blocking: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT src.relname::text
        FROM pg_constraint c
        JOIN pg_class src ON src.oid = c.conrelid
        JOIN pg_class tgt ON tgt.oid = c.confrelid
        WHERE c.contype = 'f'
          AND c.confdeltype IN ('a', 'r')
          AND tgt.relname = 'companies'
          AND src.relname <> 'companies'
        ORDER BY 1
        "#,
    )
    .fetch_all(&pool)
    .await
    .expect("pg_constraint is readable");

    assert!(
        !blocking.is_empty(),
        "no blocking FKs found — the query is wrong, not the schema"
    );

    // Two ways to stop a row blocking the parent delete, and both count. Most
    // tables are deleted outright; `users` outlives its company and instead has
    // its nullable `company_id` cleared, because an account can belong to more
    // than one tenant.
    let missing: Vec<&String> = blocking
        .iter()
        .filter(|table| {
            !wipe_source.contains(&format!("DELETE FROM {table} "))
                && !wipe_source.contains(&format!("UPDATE {table} SET company_id = NULL"))
        })
        .collect();

    assert!(
        missing.is_empty(),
        "these tables block a company delete but are absent from companies::delete_company_data: {missing:?}"
    );
}

/// The regression itself: a tenant that has run payroll and sent an email could
/// not be deleted at all. `payroll_item_details` and `tp3_records` were the two
/// tables missing from the wipe order, and both are only reachable once real
/// payroll data exists — which is why a fixture-light test never caught it.
#[tokio::test]
async fn a_company_that_has_run_payroll_can_still_be_deleted() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let company_id = seed_company(&pool).await;
    let group_id = seed_payroll_group(&pool, company_id).await;
    let employee_id = seed_employee(&pool, company_id, Some(group_id), 300_000).await;

    let run_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO payroll_runs (company_id, payroll_group_id, period_year, period_month,
                                     period_start, period_end, pay_date)
           VALUES ($1, $2, 2026, 1, $3, $4, $4) RETURNING id"#,
    )
    .bind(company_id)
    .bind(group_id)
    .bind(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
    .bind(NaiveDate::from_ymd_opt(2026, 1, 31).unwrap())
    .fetch_one(&pool)
    .await
    .expect("payroll run seeds");

    let item_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO payroll_items (payroll_run_id, employee_id, basic_salary, gross_salary,
                                      total_deductions, net_salary)
           VALUES ($1, $2, 300000, 300000, 0, 300000) RETURNING id"#,
    )
    .bind(run_id)
    .bind(employee_id)
    .fetch_one(&pool)
    .await
    .expect("payroll item seeds");

    sqlx::query(
        r#"INSERT INTO payroll_item_details (payroll_item_id, category, item_type, description, amount)
           VALUES ($1, 'earning', 'basic', 'Basic Salary', 300000)"#,
    )
    .bind(item_id)
    .execute(&pool)
    .await
    .expect("breakdown line seeds");

    sqlx::query(
        r#"INSERT INTO tp3_records (employee_id, company_id, tax_year)
           VALUES ($1, $2, 2026)"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("tp3 record seeds");

    let mut conn = pool.acquire().await.expect("connection");
    crate::repositories::companies::delete_cascade(&mut conn, company_id)
        .await
        .expect("a tenant with payroll history must be deletable");
    drop(conn);

    let survivors: i64 = sqlx::query_scalar("SELECT count(*) FROM companies WHERE id = $1")
        .bind(company_id)
        .fetch_one(&pool)
        .await
        .expect("count is readable");
    assert_eq!(survivors, 0, "the company row itself must be gone");
}
