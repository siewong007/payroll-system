//! Aggregating observations into proposable office networks.
//!
//! The grouping is the privacy boundary as much as the analytical one: the
//! underlying rows are employees' home and mobile addresses, and this is the
//! only shape in which they leave the database. Nothing here returns an
//! `employee_id`, so no endpoint built on it can accidentally expose "who was
//! seen where".

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::company_network::NetworkCandidate;

/// Candidate networks for one company, most corroborated first.
///
/// Excludes blocks already approved (there is nothing to propose) and blocks an
/// administrator has dismissed (proposing them again forever is how a dismissal
/// turns into an approval just to clear the badge).
pub async fn list_candidates(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<NetworkCandidate>> {
    let rows = sqlx::query_as!(
        NetworkCandidate,
        r#"SELECT o.network                          AS "network!",
                  o.prefix_len                       AS "prefix_len!",
                  COUNT(DISTINCT o.employee_id)      AS "distinct_employees!",
                  SUM(o.observation_count)::bigint   AS "observation_count!",
                  SUM(o.anchored_count)::bigint      AS "anchored_count!",
                  SUM(o.denied_count)::bigint        AS "denied_count!",
                  MIN(o.first_seen_at)               AS "first_seen_at!",
                  MAX(o.last_seen_at)                AS "last_seen_at!"
           FROM attendance_network_observations o
           WHERE o.company_id = $1
             AND NOT EXISTS (
                 SELECT 1 FROM company_networks n
                 WHERE n.company_id = o.company_id
                   AND n.network = o.network
                   AND n.prefix_len = o.prefix_len
             )
             AND NOT EXISTS (
                 SELECT 1 FROM attendance_network_dismissals d
                 WHERE d.company_id = o.company_id
                   AND d.network = o.network
                   AND d.prefix_len = o.prefix_len
                   AND d.expires_at > NOW()
             )
           GROUP BY o.network, o.prefix_len
           -- Denials first: a block turning people away is the one an
           -- administrator needs to see today, ahead of any slow-burn
           -- corroborated candidate.
           ORDER BY SUM(o.denied_count) DESC,
                    SUM(o.anchored_count) DESC,
                    COUNT(DISTINCT o.employee_id) DESC,
                    MAX(o.last_seen_at) DESC
           LIMIT 50"#,
        company_id
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// One candidate by block, for validating an approve/dismiss against the
/// evidence that actually exists rather than whatever the client posted.
pub async fn get_candidate(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    network: &str,
    prefix_len: i16,
) -> AppResult<Option<NetworkCandidate>> {
    let row = sqlx::query_as!(
        NetworkCandidate,
        r#"SELECT o.network                          AS "network!",
                  o.prefix_len                       AS "prefix_len!",
                  COUNT(DISTINCT o.employee_id)      AS "distinct_employees!",
                  SUM(o.observation_count)::bigint   AS "observation_count!",
                  SUM(o.anchored_count)::bigint      AS "anchored_count!",
                  SUM(o.denied_count)::bigint        AS "denied_count!",
                  MIN(o.first_seen_at)               AS "first_seen_at!",
                  MAX(o.last_seen_at)                AS "last_seen_at!"
           FROM attendance_network_observations o
           WHERE o.company_id = $1 AND o.network = $2 AND o.prefix_len = $3
           GROUP BY o.network, o.prefix_len"#,
        company_id,
        network,
        prefix_len
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}
