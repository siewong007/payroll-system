//! The approved-network allow-list.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::company_network::CompanyNetwork;

pub async fn list_for_company(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<CompanyNetwork>> {
    let rows = sqlx::query_as!(
        CompanyNetwork,
        r#"SELECT id, company_id, label, network, prefix_len, is_active, approved_by,
                  approved_at, learned_from_observation, created_at, updated_at
           FROM company_networks
           WHERE company_id = $1
           ORDER BY is_active DESC, label"#,
        company_id
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Only the rows the check-in path may match against.
pub async fn list_active(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<CompanyNetwork>> {
    let rows = sqlx::query_as!(
        CompanyNetwork,
        r#"SELECT id, company_id, label, network, prefix_len, is_active, approved_by,
                  approved_at, learned_from_observation, created_at, updated_at
           FROM company_networks
           WHERE company_id = $1 AND is_active = TRUE
           ORDER BY label"#,
        company_id
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn get(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<CompanyNetwork>> {
    let row = sqlx::query_as!(
        CompanyNetwork,
        r#"SELECT id, company_id, label, network, prefix_len, is_active, approved_by,
                  approved_at, learned_from_observation, created_at, updated_at
           FROM company_networks
           WHERE id = $1 AND company_id = $2"#,
        id,
        company_id
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    label: &str,
    network: &str,
    prefix_len: i16,
    approved_by: Uuid,
    learned_from_observation: bool,
) -> AppResult<CompanyNetwork> {
    let row = sqlx::query_as!(
        CompanyNetwork,
        r#"INSERT INTO company_networks
               (company_id, label, network, prefix_len, approved_by, learned_from_observation)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id, company_id, label, network, prefix_len, is_active, approved_by,
                     approved_at, learned_from_observation, created_at, updated_at"#,
        company_id,
        label,
        network,
        prefix_len,
        approved_by,
        learned_from_observation,
    )
    .fetch_one(executor)
    .await?;
    Ok(row)
}

pub async fn update(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    label: &str,
    is_active: bool,
) -> AppResult<CompanyNetwork> {
    let row = sqlx::query_as!(
        CompanyNetwork,
        r#"UPDATE company_networks
           SET label = $3, is_active = $4, updated_at = NOW()
           WHERE id = $1 AND company_id = $2
           RETURNING id, company_id, label, network, prefix_len, is_active, approved_by,
                     approved_at, learned_from_observation, created_at, updated_at"#,
        id,
        company_id,
        label,
        is_active,
    )
    .fetch_one(executor)
    .await?;
    Ok(row)
}

pub async fn delete(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<u64> {
    let result = sqlx::query!(
        "DELETE FROM company_networks WHERE id = $1 AND company_id = $2",
        id,
        company_id
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

/// Whether a block is already on the allow-list, active or not.
///
/// Used to keep a dormant duplicate from being re-approved as a second row,
/// which would leave two entries that must both be deactivated to withdraw
/// trust from one network.
pub async fn exists(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    network: &str,
    prefix_len: i16,
) -> AppResult<bool> {
    let found = sqlx::query_scalar!(
        r#"SELECT EXISTS(
               SELECT 1 FROM company_networks
               WHERE company_id = $1 AND network = $2 AND prefix_len = $3
           ) AS "exists!""#,
        company_id,
        network,
        prefix_len
    )
    .fetch_one(executor)
    .await?;
    Ok(found)
}
