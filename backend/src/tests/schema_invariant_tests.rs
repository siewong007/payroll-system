use chrono::NaiveDate;

use crate::tests::support::{seed_company, seed_employee, skip_if_no_db};

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
