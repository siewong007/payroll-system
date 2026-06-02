//! Data access for the `company_locations` table (geofence locations).

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::company_location::CompanyLocation;

pub async fn list_for_company(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<CompanyLocation>> {
    let locs = sqlx::query_as!(
        CompanyLocation,
        "SELECT * FROM company_locations WHERE company_id = $1 ORDER BY name",
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(locs)
}

pub async fn list_active(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<CompanyLocation>> {
    let locs = sqlx::query_as!(
        CompanyLocation,
        "SELECT * FROM company_locations WHERE company_id = $1 AND is_active = TRUE",
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(locs)
}

pub async fn get(
    executor: impl Executor<'_, Database = Postgres>,
    location_id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<CompanyLocation>> {
    let loc = sqlx::query_as!(
        CompanyLocation,
        "SELECT * FROM company_locations WHERE id = $1 AND company_id = $2",
        location_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(loc)
}

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    name: &str,
    latitude: f64,
    longitude: f64,
    radius_meters: i32,
) -> AppResult<CompanyLocation> {
    let loc = sqlx::query_as!(
        CompanyLocation,
        r#"INSERT INTO company_locations (company_id, name, latitude, longitude, radius_meters)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING *"#,
        company_id,
        name,
        latitude,
        longitude,
        radius_meters,
    )
    .fetch_one(executor)
    .await?;
    Ok(loc)
}

#[allow(clippy::too_many_arguments)]
pub async fn update(
    executor: impl Executor<'_, Database = Postgres>,
    location_id: Uuid,
    company_id: Uuid,
    name: &str,
    latitude: f64,
    longitude: f64,
    radius_meters: i32,
    is_active: bool,
) -> AppResult<CompanyLocation> {
    let loc = sqlx::query_as!(
        CompanyLocation,
        r#"UPDATE company_locations
           SET name = $3, latitude = $4, longitude = $5, radius_meters = $6,
               is_active = $7, updated_at = NOW()
           WHERE id = $1 AND company_id = $2
           RETURNING *"#,
        location_id,
        company_id,
        name,
        latitude,
        longitude,
        radius_meters,
        is_active,
    )
    .fetch_one(executor)
    .await?;
    Ok(loc)
}

pub async fn delete(
    executor: impl Executor<'_, Database = Postgres>,
    location_id: Uuid,
    company_id: Uuid,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        "DELETE FROM company_locations WHERE id = $1 AND company_id = $2",
        location_id,
        company_id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}
