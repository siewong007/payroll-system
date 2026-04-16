use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::company_location::{CompanyLocation, CreateLocationRequest, GeofenceCheckResult, UpdateLocationRequest};

/// Haversine distance in meters between two lat/lng points
fn haversine_meters(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6_371_000.0; // Earth radius in meters
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    R * c
}

// ─── CRUD ───

pub async fn list_locations(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<CompanyLocation>> {
    let locs = sqlx::query_as::<_, CompanyLocation>(
        "SELECT * FROM company_locations WHERE company_id = $1 ORDER BY name",
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;
    Ok(locs)
}

pub async fn create_location(
    pool: &PgPool,
    company_id: Uuid,
    req: &CreateLocationRequest,
) -> AppResult<CompanyLocation> {
    let radius = req.radius_meters.unwrap_or(200);
    if radius < 10 || radius > 10_000 {
        return Err(AppError::BadRequest(
            "Radius must be between 10 and 10,000 meters".into(),
        ));
    }

    let loc = sqlx::query_as::<_, CompanyLocation>(
        r#"INSERT INTO company_locations (company_id, name, latitude, longitude, radius_meters)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING *"#,
    )
    .bind(company_id)
    .bind(&req.name)
    .bind(req.latitude)
    .bind(req.longitude)
    .bind(radius)
    .fetch_one(pool)
    .await?;
    Ok(loc)
}

pub async fn update_location(
    pool: &PgPool,
    company_id: Uuid,
    location_id: Uuid,
    req: &UpdateLocationRequest,
) -> AppResult<CompanyLocation> {
    let existing = sqlx::query_as::<_, CompanyLocation>(
        "SELECT * FROM company_locations WHERE id = $1 AND company_id = $2",
    )
    .bind(location_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Location not found".into()))?;

    let name = req.name.as_deref().unwrap_or(&existing.name);
    let lat = req.latitude.unwrap_or(existing.latitude);
    let lng = req.longitude.unwrap_or(existing.longitude);
    let radius = req.radius_meters.unwrap_or(existing.radius_meters);
    let active = req.is_active.unwrap_or(existing.is_active);

    if radius < 10 || radius > 10_000 {
        return Err(AppError::BadRequest(
            "Radius must be between 10 and 10,000 meters".into(),
        ));
    }

    let loc = sqlx::query_as::<_, CompanyLocation>(
        r#"UPDATE company_locations
           SET name = $3, latitude = $4, longitude = $5, radius_meters = $6,
               is_active = $7, updated_at = NOW()
           WHERE id = $1 AND company_id = $2
           RETURNING *"#,
    )
    .bind(location_id)
    .bind(company_id)
    .bind(name)
    .bind(lat)
    .bind(lng)
    .bind(radius)
    .bind(active)
    .fetch_one(pool)
    .await?;
    Ok(loc)
}

pub async fn delete_location(pool: &PgPool, company_id: Uuid, location_id: Uuid) -> AppResult<()> {
    let result = sqlx::query(
        "DELETE FROM company_locations WHERE id = $1 AND company_id = $2",
    )
    .bind(location_id)
    .bind(company_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Location not found".into()));
    }
    Ok(())
}

// ─── Geofence Mode ───

pub async fn get_geofence_mode(pool: &PgPool, company_id: Uuid) -> AppResult<String> {
    let mode: Option<String> = sqlx::query_scalar(
        "SELECT geofence_mode FROM companies WHERE id = $1",
    )
    .bind(company_id)
    .fetch_optional(pool)
    .await?;
    Ok(mode.unwrap_or_else(|| "none".to_string()))
}

pub async fn set_geofence_mode(pool: &PgPool, company_id: Uuid, mode: &str) -> AppResult<()> {
    if !matches!(mode, "none" | "warn" | "enforce") {
        return Err(AppError::BadRequest(
            "Geofence mode must be 'none', 'warn', or 'enforce'".into(),
        ));
    }

    sqlx::query("UPDATE companies SET geofence_mode = $1 WHERE id = $2")
        .bind(mode)
        .bind(company_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ─── Geofence Check ───

/// Check if a lat/lng point is within any of the company's active locations.
/// Returns the check result with nearest location info.
pub async fn check_geofence(
    pool: &PgPool,
    company_id: Uuid,
    lat: f64,
    lng: f64,
) -> AppResult<GeofenceCheckResult> {
    let locations = sqlx::query_as::<_, CompanyLocation>(
        "SELECT * FROM company_locations WHERE company_id = $1 AND is_active = TRUE",
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;

    if locations.is_empty() {
        // No locations configured → treat as within
        return Ok(GeofenceCheckResult {
            is_within: true,
            nearest_location: None,
            distance_meters: None,
        });
    }

    let mut nearest_name = String::new();
    let mut nearest_dist = f64::MAX;
    let mut is_within = false;

    for loc in &locations {
        let dist = haversine_meters(lat, lng, loc.latitude, loc.longitude);
        if dist < nearest_dist {
            nearest_dist = dist;
            nearest_name = loc.name.clone();
        }
        if dist <= loc.radius_meters as f64 {
            is_within = true;
        }
    }

    Ok(GeofenceCheckResult {
        is_within,
        nearest_location: Some(nearest_name),
        distance_meters: Some(nearest_dist.round()),
    })
}

/// Validate geofence and return whether the record should be flagged.
/// Returns Err if enforce mode and outside fence.
/// Returns Ok(true) if outside fence (warn mode), Ok(false) if inside or no check.
pub async fn validate_geofence(
    pool: &PgPool,
    company_id: Uuid,
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> AppResult<bool> {
    let mode = get_geofence_mode(pool, company_id).await?;
    if mode == "none" {
        return Ok(false); // not flagged
    }

    let (lat, lng) = match (latitude, longitude) {
        (Some(lat), Some(lng)) => (lat, lng),
        _ => {
            // No location provided
            if mode == "enforce" {
                return Err(AppError::BadRequest(
                    "Location is required for check-in. Please enable location services.".into(),
                ));
            }
            return Ok(true); // flagged in warn mode
        }
    };

    let result = check_geofence(pool, company_id, lat, lng).await?;

    if !result.is_within {
        if mode == "enforce" {
            let msg = match (result.nearest_location, result.distance_meters) {
                (Some(name), Some(dist)) => format!(
                    "You are {:.0}m from '{}'. Please check in from an approved office location.",
                    dist, name
                ),
                _ => "You are outside all approved office locations.".to_string(),
            };
            return Err(AppError::BadRequest(msg));
        }
        // warn mode — flag it
        return Ok(true);
    }

    Ok(false) // within fence, not flagged
}
