//! Verification metadata for effective-dated statutory calculation rules.

use chrono::NaiveDate;
use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;
use crate::models::statutory::VerifiedRuleSet;

/// The verified rule sets covering a date, for recording on a payroll run.
///
/// Stored with the run so the figures stay explainable: the rule tables are
/// effective-dated and mutable, so without this a later import leaves no way to
/// say which schedule actually produced a historical payslip.
pub async fn verified_for_date(
    executor: impl Executor<'_, Database = Postgres>,
    effective_date: NaiveDate,
) -> AppResult<Vec<VerifiedRuleSet>> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            uuid::Uuid,
            String,
            Option<String>,
            Option<String>,
            NaiveDate,
            Option<NaiveDate>,
        ),
    >(
        r#"
        SELECT rule_code, id, dataset_key, source_version, source_sha256,
               effective_from, effective_to
        FROM statutory_rule_sets
        WHERE status = 'verified'
          AND effective_from <= $1
          AND (effective_to IS NULL OR effective_to >= $1)
        ORDER BY rule_code
        "#,
    )
    .bind(effective_date)
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(rule_code, id, dataset_key, source_version, source_sha256, from, to)| {
                VerifiedRuleSet {
                    rule_code,
                    rule_set_id: id,
                    dataset_key,
                    source_version,
                    source_sha256,
                    effective_from: from,
                    effective_to: to,
                }
            },
        )
        .collect())
}

/// Whether an independently verified rule set covers this domain and date.
pub async fn is_verified_for_date(
    executor: impl Executor<'_, Database = Postgres>,
    rule_code: &str,
    effective_date: NaiveDate,
) -> AppResult<bool> {
    let verified = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM statutory_rule_sets
            WHERE rule_code = $1
              AND status = 'verified'
              AND effective_from <= $2
              AND (effective_to IS NULL OR effective_to >= $2)
        )
        "#,
    )
    .bind(rule_code)
    .bind(effective_date)
    .fetch_one(executor)
    .await?;

    Ok(verified)
}

/// Required statutory domains that do not have a verified rule set for a date.
pub async fn missing_required_for_date(
    executor: impl Executor<'_, Database = Postgres>,
    effective_date: NaiveDate,
) -> AppResult<Vec<String>> {
    let missing = sqlx::query_scalar::<_, Vec<String>>(
        r#"
        WITH required(rule_code) AS (
            VALUES ('epf'::text), ('socso'::text), ('eis'::text), ('pcb'::text)
        )
        SELECT COALESCE(
            array_agg(required.rule_code ORDER BY required.rule_code),
            ARRAY[]::text[]
        )
        FROM required
        WHERE NOT EXISTS (
            SELECT 1
            FROM statutory_rule_sets rules
            WHERE rules.rule_code = required.rule_code
              AND rules.status = 'verified'
              AND rules.effective_from <= $1
              AND (rules.effective_to IS NULL OR rules.effective_to >= $1)
        )
        "#,
    )
    .bind(effective_date)
    .fetch_one(executor)
    .await?;

    Ok(missing)
}
