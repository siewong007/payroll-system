//! Evidence for proposing an office network. Never an authorization input.
//!
//! One row per (company, employee, candidate block). The employee is part of
//! the key so "how many *different* people check in from here?" is answerable —
//! the single question that separates an office from one person's flat.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

/// Record that `employee_id` was seen on `network`, merging into the existing
/// row if there is one.
///
/// `anchored` marks an observation corroborated by something the employee does
/// not control (inside the geofence, or a QR token minted by a kiosk in the
/// building). It accumulates separately from the raw count because only the
/// anchored total may push a candidate over the proposal threshold.
pub async fn record(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Uuid,
    network: &str,
    prefix_len: i16,
    anchored: bool,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO attendance_network_observations
               (company_id, employee_id, network, prefix_len, observation_count, anchored_count)
           VALUES ($1, $2, $3, $4, 1, $5)
           ON CONFLICT (company_id, employee_id, network, prefix_len)
           DO UPDATE SET
               observation_count = attendance_network_observations.observation_count + 1,
               anchored_count = attendance_network_observations.anchored_count + $5,
               last_seen_at = NOW()"#,
        company_id,
        employee_id,
        network,
        prefix_len,
        i32::from(anchored),
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Forget everything learned about one block — used when it is approved (the
/// evidence has served its purpose) or dismissed.
pub async fn delete_for_network(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    network: &str,
    prefix_len: i16,
) -> AppResult<u64> {
    let result = sqlx::query!(
        r#"DELETE FROM attendance_network_observations
           WHERE company_id = $1 AND network = $2 AND prefix_len = $3"#,
        company_id,
        network,
        prefix_len
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

/// Retention purge. These rows are employees' home and mobile addresses, so
/// they are kept only as long as they can still inform a proposal.
pub async fn purge_older_than(
    executor: impl Executor<'_, Database = Postgres>,
    days: i32,
) -> AppResult<u64> {
    let result = sqlx::query!(
        r#"DELETE FROM attendance_network_observations
           WHERE last_seen_at < NOW() - make_interval(days => $1)"#,
        days
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

// ─── Dismissals ───

pub async fn dismiss(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    network: &str,
    prefix_len: i16,
    dismissed_by: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO attendance_network_dismissals
               (company_id, network, prefix_len, dismissed_by)
           VALUES ($1, $2, $3, $4)
           ON CONFLICT (company_id, network, prefix_len)
           DO UPDATE SET dismissed_by = $4, dismissed_at = NOW()"#,
        company_id,
        network,
        prefix_len,
        dismissed_by
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Clear a dismissal so the block can be proposed again — the escape hatch for
/// an administrator who dismissed the office by mistake.
pub async fn undismiss(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    network: &str,
    prefix_len: i16,
) -> AppResult<u64> {
    let result = sqlx::query!(
        r#"DELETE FROM attendance_network_dismissals
           WHERE company_id = $1 AND network = $2 AND prefix_len = $3"#,
        company_id,
        network,
        prefix_len
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}
