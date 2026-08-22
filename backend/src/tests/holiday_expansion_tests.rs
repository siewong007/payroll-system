//! Recurring holidays are expanded into the years and ranges that ask about
//! them (plan item 13). The auto-absent cron always honoured `is_recurring`;
//! these reads did not, so leave validation charged employees for days the
//! cron agreed they were never expected to work.

use crate::repositories::holidays;
use crate::tests::support::{seed_company, seed_user, skip_if_no_db};

#[tokio::test]
async fn recurring_holidays_appear_in_later_years_without_their_original() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let actor = seed_user(&pool, company_id, "hr_manager").await;

    holidays::insert(
        &pool,
        company_id,
        "Labour Day",
        chrono::NaiveDate::from_ymd_opt(2024, 5, 1).unwrap(),
        "public_holiday",
        None,
        true,
        None,
        actor,
    )
    .await
    .expect("seed recurring holiday");

    let year_2024 = holidays::list_for_year(&pool, company_id, 2024)
        .await
        .expect("list 2024");
    assert_eq!(year_2024.len(), 1);
    assert_eq!(year_2024[0].date.to_string(), "2024-05-01");

    // The virtual occurrence: same row identity, rewritten date.
    let year_2026 = holidays::list_for_year(&pool, company_id, 2026)
        .await
        .expect("list 2026");
    assert_eq!(year_2026.len(), 1, "original must NOT appear in 2026");
    assert_eq!(year_2026[0].date.to_string(), "2026-05-01");
    assert!(year_2026[0].is_recurring);

    // A span crossing several years sees one instance per year.
    let span = holidays::list_for_range(
        &pool,
        company_id,
        chrono::NaiveDate::from_ymd_opt(2027, 1, 1).unwrap(),
        chrono::NaiveDate::from_ymd_opt(2029, 12, 31).unwrap(),
    )
    .await
    .expect("list span");
    let dates: Vec<String> = span.iter().map(|h| h.date.to_string()).collect();
    assert_eq!(
        dates,
        vec!["2027-05-01", "2028-05-01", "2029-05-01"],
        "one occurrence per year inside the span"
    );
}

#[tokio::test]
async fn non_recurring_holidays_stay_confined_to_their_own_date() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let actor = seed_user(&pool, company_id, "hr_manager").await;

    holidays::insert(
        &pool,
        company_id,
        "One-off company day",
        chrono::NaiveDate::from_ymd_opt(2025, 7, 20).unwrap(),
        "company_holiday",
        None,
        false,
        None,
        actor,
    )
    .await
    .expect("seed one-off holiday");

    let year_2026 = holidays::list_for_year(&pool, company_id, 2026)
        .await
        .expect("list 2026");
    assert!(year_2026.is_empty(), "one-off must not recur");
}

#[tokio::test]
async fn february_twenty_ninth_recurrence_is_skipped_not_shifted() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let actor = seed_user(&pool, company_id, "hr_manager").await;

    holidays::insert(
        &pool,
        company_id,
        "Leap Day",
        chrono::NaiveDate::from_ymd_opt(2024, 2, 29).unwrap(),
        "public_holiday",
        None,
        true,
        None,
        actor,
    )
    .await
    .expect("seed leap-day holiday");

    let non_leap = holidays::list_for_year(&pool, company_id, 2025)
        .await
        .expect("list 2025");
    assert!(non_leap.is_empty(), "no silent shift to Feb 28 or Mar 1");

    let leap = holidays::list_for_year(&pool, company_id, 2028)
        .await
        .expect("list 2028");
    assert_eq!(leap[0].date.to_string(), "2028-02-29");
}
