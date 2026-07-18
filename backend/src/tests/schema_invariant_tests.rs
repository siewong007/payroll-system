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
