use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompanyLocation {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub radius_meters: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLocationRequest {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    /// Allowed radius in meters (default 200)
    pub radius_meters: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLocationRequest {
    pub name: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub radius_meters: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SetGeofenceModeRequest {
    /// "none", "warn", or "enforce"
    pub mode: String,
}

#[derive(Debug, Serialize)]
pub struct GeofenceCheckResult {
    pub is_within: bool,
    pub nearest_location: Option<String>,
    pub distance_meters: Option<f64>,
}
