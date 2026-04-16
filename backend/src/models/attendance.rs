use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Platform Setting ───

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PlatformSetting {
    pub key: String,
    pub value: String,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
}

// ─── QR Token ───

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AttendanceQrToken {
    pub id: Uuid,
    pub company_id: Uuid,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub used: bool,
    pub created_at: DateTime<Utc>,
}

// ─── Attendance Record ───

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AttendanceRecord {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub check_in_at: DateTime<Utc>,
    pub check_out_at: Option<DateTime<Utc>>,
    pub method: String,
    pub status: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub checkout_latitude: Option<f64>,
    pub checkout_longitude: Option<f64>,
    pub notes: Option<String>,
    pub qr_token_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub hours_worked: Option<rust_decimal::Decimal>,
    pub overtime_hours: Option<rust_decimal::Decimal>,
    pub is_outside_geofence: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Extended record that includes employee details for admin list views
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AttendanceRecordWithEmployee {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub employee_number: String,
    pub full_name: String,
    pub department: Option<String>,
    pub check_in_at: DateTime<Utc>,
    pub check_out_at: Option<DateTime<Utc>>,
    pub method: String,
    pub status: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub checkout_latitude: Option<f64>,
    pub checkout_longitude: Option<f64>,
    pub notes: Option<String>,
    pub hours_worked: Option<rust_decimal::Decimal>,
    pub overtime_hours: Option<rust_decimal::Decimal>,
    pub is_outside_geofence: Option<bool>,
    pub created_at: DateTime<Utc>,
}

// ─── Requests ───

#[derive(Debug, Deserialize)]
pub struct CheckInQrRequest {
    /// The raw token value from scanning the QR code
    pub token: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CheckInFaceIdRequest {
    /// Raw ID of the WebAuthn credential used (from webauthn assertion)
    pub credential_id: String,
    /// Client assertion data (base64url encoded)
    pub assertion: serde_json::Value,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CheckOutRequest {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ManualAttendanceRequest {
    pub employee_id: Uuid,
    pub check_in_at: DateTime<Utc>,
    pub check_out_at: Option<DateTime<Utc>>,
    pub status: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AttendanceListQuery {
    pub employee_id: Option<Uuid>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub status: Option<String>,
    pub method: Option<String>,
    /// Page number (1-based, default 1)
    pub page: Option<i64>,
    /// Items per page (default 50, max 200)
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedAttendance<T: Serialize> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAttendanceRecordRequest {
    pub check_in_at: Option<DateTime<Utc>>,
    pub check_out_at: Option<DateTime<Utc>>,
    pub status: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetAttendanceMethodRequest {
    /// "qr_code" or "face_id"
    pub method: String,
    /// whether admins can override the global setting
    pub allow_company_override: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SetCompanyAttendanceMethodRequest {
    /// "qr_code" or "face_id" or null to use platform default
    pub method: Option<String>,
}

/// Response returned when a QR token is generated
#[derive(Debug, Serialize)]
pub struct QrTokenResponse {
    pub token: String,
    pub expires_at: DateTime<Utc>,
    /// The full URL the QR code should encode (employee scans this)
    pub scan_url: String,
}

/// Response for the effective attendance method of a company
#[derive(Debug, Serialize)]
pub struct AttendanceMethodResponse {
    pub method: String,
    pub allow_company_override: bool,
    pub is_company_override: bool,
}
