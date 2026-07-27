use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::company_location::{
    CompanyLocation, CreateLocationRequest, GeofenceCheckResult, UpdateLocationRequest,
};
use crate::repositories::{companies, company_locations};
use crate::services::audit_service::{self, AuditRequestMeta};

/// Radius applied when a location is created without an explicit one.
const DEFAULT_RADIUS_METERS: i32 = 200;

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

fn validate_coordinates(latitude: f64, longitude: f64) -> AppResult<()> {
    if !latitude.is_finite() || !(-90.0..=90.0).contains(&latitude) {
        return Err(AppError::BadRequest(
            "Latitude must be a finite value between -90 and 90".into(),
        ));
    }
    if !longitude.is_finite() || !(-180.0..=180.0).contains(&longitude) {
        return Err(AppError::BadRequest(
            "Longitude must be a finite value between -180 and 180".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_optional_coordinates(
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> AppResult<()> {
    match (latitude, longitude) {
        (None, None) => Ok(()),
        (Some(latitude), Some(longitude)) => validate_coordinates(latitude, longitude),
        _ => Err(AppError::BadRequest(
            "Latitude and longitude must be provided together".into(),
        )),
    }
}

// ─── CRUD ───

pub async fn list_locations(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<CompanyLocation>> {
    company_locations::list_for_company(pool, company_id).await
}

pub async fn create_location(
    pool: &PgPool,
    company_id: Uuid,
    req: &CreateLocationRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<CompanyLocation> {
    validate_coordinates(req.latitude, req.longitude)?;
    let radius = req.radius_meters.unwrap_or(DEFAULT_RADIUS_METERS);
    validate_radius(radius)?;

    let loc = company_locations::insert(
        pool,
        company_id,
        &req.name,
        req.latitude,
        req.longitude,
        radius,
    )
    .await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "create",
        "company_location",
        Some(loc.id),
        None,
        Some(serde_json::to_value(&loc).unwrap_or_default()),
        Some("Geofence location created"),
        audit_meta,
    )
    .await;

    Ok(loc)
}

pub async fn update_location(
    pool: &PgPool,
    company_id: Uuid,
    location_id: Uuid,
    req: &UpdateLocationRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<CompanyLocation> {
    let existing = company_locations::get(pool, location_id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Location not found".into()))?;

    let name = req.name.as_deref().unwrap_or(&existing.name);
    let lat = req.latitude.unwrap_or(existing.latitude);
    let lng = req.longitude.unwrap_or(existing.longitude);
    let radius = req.radius_meters.unwrap_or(existing.radius_meters);
    let active = req.is_active.unwrap_or(existing.is_active);

    validate_coordinates(lat, lng)?;
    validate_radius(radius)?;

    // Only when the edit actually deactivates — renaming an already-inactive
    // row must not trip the guard.
    if existing.is_active && !active {
        ensure_not_the_last_active_location(pool, company_id, location_id).await?;
    }

    let loc = company_locations::update(
        pool,
        location_id,
        company_id,
        name,
        lat,
        lng,
        radius,
        active,
    )
    .await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "update",
        "company_location",
        Some(loc.id),
        Some(serde_json::to_value(&existing).unwrap_or_default()),
        Some(serde_json::to_value(&loc).unwrap_or_default()),
        Some("Geofence location updated"),
        audit_meta,
    )
    .await;

    Ok(loc)
}

pub async fn delete_location(
    pool: &PgPool,
    company_id: Uuid,
    location_id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let existing = company_locations::get(pool, location_id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Location not found".into()))?;

    // Removing an already-inactive row cannot empty the active list, so it is
    // never the operation that arms the fail-closed path.
    if existing.is_active {
        ensure_not_the_last_active_location(pool, company_id, location_id).await?;
    }

    let rows = company_locations::delete(pool, location_id, company_id).await?;
    if rows == 0 {
        return Err(AppError::NotFound("Location not found".into()));
    }

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "delete",
        "company_location",
        Some(location_id),
        Some(serde_json::to_value(&existing).unwrap_or_default()),
        None,
        Some("Geofence location deleted"),
        audit_meta,
    )
    .await;

    Ok(())
}

/// Refuse an edit that would leave an armed geofence with nothing to evaluate
/// against. Conflict, not BadRequest: the request is well-formed, it is the
/// current state that forbids it.
///
/// Note the asymmetry this creates, which is what keeps a fail-closed geofence
/// from becoming a lockout incident: setting the mode back to 'none' is never
/// blocked, so an admin can always get *out*; getting *in* requires having
/// locations in the first place.
///
/// Only `enforce` is guarded. `warn` records the check-in either way and merely
/// flags it, so an empty location list there is noisy rather than unsafe — and
/// blocking a tidy-up an admin is entitled to make would be friction bought
/// with nothing. The evaluation path still fails closed for both modes.
async fn ensure_not_the_last_active_location(
    pool: &PgPool,
    company_id: Uuid,
    location_id: Uuid,
) -> AppResult<()> {
    if get_geofence_mode(pool, company_id).await? != "enforce" {
        return Ok(());
    }
    if company_locations::count_active_excluding(pool, company_id, Some(location_id)).await? == 0 {
        return Err(AppError::Conflict(
            "This is the only active office location and the geofence is enforcing check-ins against it. Add another location, or set the geofence mode to 'none' or 'warn', before removing this one.".into(),
        ));
    }
    Ok(())
}

// ─── Geofence Mode ───

pub async fn get_geofence_mode(pool: &PgPool, company_id: Uuid) -> AppResult<String> {
    let mode = companies::get_geofence_mode(pool, company_id).await?;
    Ok(mode.unwrap_or_else(|| "none".to_string()))
}

pub async fn set_geofence_mode(
    pool: &PgPool,
    company_id: Uuid,
    mode: &str,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    if !matches!(mode, "none" | "warn" | "enforce") {
        return Err(AppError::BadRequest(
            "Geofence mode must be 'none', 'warn', or 'enforce'".into(),
        ));
    }

    // This is the entry that used to let an admin arm enforcement against an
    // empty location list in one click — after which every check-in with
    // coordinates was accepted and recorded as inside a fence that did not
    // exist, while every GPS-less one was refused.
    if mode != "none" {
        let active = company_locations::count_active_excluding(pool, company_id, None).await?;
        if active == 0 {
            return Err(AppError::Conflict(format!(
                "Add at least one active office location before setting the geofence mode to '{mode}'."
            )));
        }
    }

    let old_mode = get_geofence_mode(pool, company_id).await?;

    companies::set_geofence_mode(pool, company_id, mode).await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "update",
        "geofence_mode",
        Some(company_id),
        Some(serde_json::json!({ "mode": old_mode })),
        Some(serde_json::json!({ "mode": mode })),
        Some("Geofence mode updated"),
        audit_meta,
    )
    .await;

    Ok(())
}

// ─── Geofence Check ───

/// Outcome of evaluating a point against a company's active locations.
///
/// `Unconfigured` is deliberately distinct from an evaluated miss. With zero
/// active locations there is no fence to be inside or outside of, and the two
/// callers want different behaviour: check-in must refuse (or flag), check-out
/// must never refuse. Collapsing both into `is_within: true` is what let an
/// employee check in from anywhere on earth and have the row recorded as
/// *inside* the fence, while an on-site employee whose browser denied GPS was
/// hard-rejected by the same request handler.
///
/// Lives here rather than in `models/` because it is service control flow, not
/// a wire shape — `GeofenceCheckResult` is internal too, with no client mirror.
pub enum GeofenceEvaluation {
    /// The company has no active locations; the fence could not be evaluated.
    Unconfigured,
    Evaluated(GeofenceCheckResult),
}

/// Check if a lat/lng point is within any of the company's active locations.
pub async fn check_geofence(
    pool: &PgPool,
    company_id: Uuid,
    lat: f64,
    lng: f64,
) -> AppResult<GeofenceEvaluation> {
    let locations = company_locations::list_active(pool, company_id).await?;

    if locations.is_empty() {
        return Ok(GeofenceEvaluation::Unconfigured);
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

    Ok(GeofenceEvaluation::Evaluated(GeofenceCheckResult {
        is_within,
        nearest_location: Some(nearest_name),
        distance_meters: Some(nearest_dist.round()),
    }))
}

/// A geofence mode is armed but the company has no active location to evaluate
/// against. An operator must see this: it means every enforced check-in is now
/// being refused for a configuration reason, not an employee one.
fn warn_unconfigured(company_id: Uuid, mode: &str) {
    tracing::error!(
        company_id = %company_id,
        mode,
        "geofence is armed but no active company locations are configured"
    );
}

/// Evaluate the geofence for a check-out without ever rejecting: returns
/// whether the record should be flagged as outside. A blocked check-out
/// becomes a stale open session only an admin can fix, so even in 'enforce'
/// mode an off-site (or GPS-less) check-out is recorded and flagged, not
/// refused — the flag ORs into `is_outside_geofence` for admin review.
pub async fn flag_geofence_for_checkout(
    pool: &PgPool,
    company_id: Uuid,
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> AppResult<bool> {
    validate_optional_coordinates(latitude, longitude)?;
    let mode = get_geofence_mode(pool, company_id).await?;
    if mode == "none" {
        return Ok(false);
    }
    let (lat, lng) = match (latitude, longitude) {
        (Some(lat), Some(lng)) => (lat, lng),
        _ => return Ok(true),
    };
    match check_geofence(pool, company_id, lat, lng).await? {
        // Flagged for review, never refused — that is the whole contract of
        // this function, and it is what keeps the fail-closed check-in change
        // from ever stranding somebody in an open session.
        GeofenceEvaluation::Unconfigured => {
            warn_unconfigured(company_id, &mode);
            Ok(true)
        }
        GeofenceEvaluation::Evaluated(result) => Ok(!result.is_within),
    }
}

/// Radius bounds shared by create and update. Extracted so both paths — and
/// their tests — cannot drift apart.
pub(crate) fn validate_radius(radius_meters: i32) -> AppResult<()> {
    if !(10..=10_000).contains(&radius_meters) {
        return Err(AppError::BadRequest(
            "Radius must be between 10 and 10,000 meters".into(),
        ));
    }
    Ok(())
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
    validate_optional_coordinates(latitude, longitude)?;
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

    let result = match check_geofence(pool, company_id, lat, lng).await? {
        GeofenceEvaluation::Evaluated(result) => result,
        // Fail closed: never record `is_outside_geofence = false` for a fence
        // that was never evaluated. The message is deliberately *not* the
        // "you are outside all approved office locations" one — that would send
        // an employee hunting for an office they are already standing in.
        GeofenceEvaluation::Unconfigured => {
            warn_unconfigured(company_id, &mode);
            if mode == "enforce" {
                return Err(AppError::BadRequest(
                    "Geofence enforcement is on but no approved office locations are configured. Ask an administrator.".into(),
                ));
            }
            return Ok(true); // warn mode — flagged for review
        }
    };

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

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_RADIUS_METERS, haversine_meters, validate_coordinates,
        validate_optional_coordinates, validate_radius,
    };
    use crate::core::error::AppError;

    // Kuala Lumpur landmarks, used as a real-world distance fixture.
    const KLCC: (f64, f64) = (3.157_64, 101.711_86);
    const KL_TOWER: (f64, f64) = (3.152_78, 101.703_33);

    #[test]
    fn haversine_is_zero_for_identical_points() {
        assert_eq!(haversine_meters(KLCC.0, KLCC.1, KLCC.0, KLCC.1), 0.0);
    }

    #[test]
    fn haversine_matches_a_known_city_distance() {
        // KLCC to KL Tower is roughly 1.1 km on the ground.
        let d = haversine_meters(KLCC.0, KLCC.1, KL_TOWER.0, KL_TOWER.1);
        assert!(
            (1_000.0..1_250.0).contains(&d),
            "expected ~1.1km between KLCC and KL Tower, got {d}"
        );
    }

    #[test]
    fn haversine_is_symmetric() {
        let forward = haversine_meters(KLCC.0, KLCC.1, KL_TOWER.0, KL_TOWER.1);
        let backward = haversine_meters(KL_TOWER.0, KL_TOWER.1, KLCC.0, KLCC.1);
        assert!((forward - backward).abs() < 1e-6);
    }

    #[test]
    fn haversine_resolves_office_scale_offsets() {
        // ~0.0001 degrees of latitude is ~11 m — smaller than the minimum
        // geofence radius, so the formula must not collapse it to zero.
        let d = haversine_meters(KLCC.0, KLCC.1, KLCC.0 + 0.000_1, KLCC.1);
        assert!((9.0..14.0).contains(&d), "expected ~11m, got {d}");
    }

    #[test]
    fn haversine_spans_the_antimeridian_without_wrapping_the_long_way() {
        // Two points 0.02 degrees apart either side of the 180th meridian are
        // ~2 km apart, not most of the way around the planet.
        let d = haversine_meters(0.0, 179.99, 0.0, -179.99);
        assert!(
            d < 3_000.0,
            "antimeridian distance should be short, got {d}"
        );
    }

    #[test]
    fn haversine_handles_antipodal_points() {
        // Half the Earth's circumference, ~20,015 km.
        let d = haversine_meters(0.0, 0.0, 0.0, 180.0);
        assert!(
            (20_000_000.0..20_040_000.0).contains(&d),
            "expected a half-circumference, got {d}"
        );
    }

    #[test]
    fn coordinates_accept_the_full_valid_range_including_the_poles() {
        for (lat, lng) in [
            (0.0, 0.0),
            (90.0, 180.0),
            (-90.0, -180.0),
            (3.157_64, 101.711_86),
        ] {
            assert!(
                validate_coordinates(lat, lng).is_ok(),
                "should accept {lat},{lng}"
            );
        }
    }

    #[test]
    fn coordinates_reject_out_of_range_and_non_finite_values() {
        for (lat, lng) in [
            (90.1, 0.0),
            (-90.1, 0.0),
            (0.0, 180.1),
            (0.0, -180.1),
            (f64::NAN, 0.0),
            (0.0, f64::NAN),
            (f64::INFINITY, 0.0),
            (0.0, f64::NEG_INFINITY),
        ] {
            assert!(
                matches!(validate_coordinates(lat, lng), Err(AppError::BadRequest(_))),
                "should reject {lat},{lng}"
            );
        }
    }

    #[test]
    fn optional_coordinates_allow_both_absent_but_not_one_of_two() {
        assert!(validate_optional_coordinates(None, None).is_ok());
        assert!(validate_optional_coordinates(Some(3.15), Some(101.71)).is_ok());

        // A half-supplied pair is a client bug: silently treating it as "no
        // location" would let a broken GPS read bypass the fence.
        assert!(matches!(
            validate_optional_coordinates(Some(3.15), None),
            Err(AppError::BadRequest(_))
        ));
        assert!(matches!(
            validate_optional_coordinates(None, Some(101.71)),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn optional_coordinates_still_range_check_a_supplied_pair() {
        assert!(matches!(
            validate_optional_coordinates(Some(91.0), Some(0.0)),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn radius_accepts_its_inclusive_bounds_and_rejects_just_outside() {
        assert!(validate_radius(10).is_ok());
        assert!(validate_radius(10_000).is_ok());
        assert!(validate_radius(DEFAULT_RADIUS_METERS).is_ok());

        for radius in [9, 10_001, 0, -1] {
            assert!(
                matches!(validate_radius(radius), Err(AppError::BadRequest(_))),
                "should reject radius {radius}"
            );
        }
    }

    #[test]
    fn default_radius_sits_inside_the_permitted_range() {
        // A default outside the bounds would make every unspecified-radius
        // create fail its own validation.
        assert!(validate_radius(DEFAULT_RADIUS_METERS).is_ok());
    }
}
