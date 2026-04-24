use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, Response, StatusCode, header},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::attendance::{
    AttendanceExportQuery, AttendanceListQuery, AttendanceMethodResponse, AttendanceRecord,
    AttendanceRecordWithEmployee, AttendanceSummaryItem, AttendanceSummaryQuery,
    CheckInFaceIdRequest, CheckInQrRequest, CheckOutRequest, ManualAttendanceRequest,
    PaginatedAttendance, QrTokenResponse, SetAttendanceMethodRequest,
    SetCompanyAttendanceMethodRequest, UpdateAttendanceRecordRequest,
};
use crate::models::attendance_kiosk::KioskCredential;
use crate::services::{attendance_service, audit_service::AuditRequestMeta};

// ─── Effective Method ───

/// Returns the effective attendance method for the caller's company
pub async fn get_attendance_method(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<AttendanceMethodResponse>> {
    let company_id = auth.company_id()?;

    let resp = attendance_service::get_effective_method(&state.pool, company_id).await?;
    Ok(Json(resp))
}

// ─── Platform (super_admin only) ───

pub async fn get_platform_method(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_super_admin()?;

    let method = attendance_service::get_platform_attendance_method(&state.pool).await?;
    let allow_override = attendance_service::get_platform_allow_override(&state.pool).await?;

    Ok(Json(serde_json::json!({
        "method": method,
        "allow_company_override": allow_override,
    })))
}

pub async fn set_platform_method(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(req): Json<SetAttendanceMethodRequest>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_super_admin()?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);

    attendance_service::set_platform_attendance_method(
        &state.pool,
        &req.method,
        req.allow_company_override.unwrap_or(false),
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

// ─── Company override (admin) ───

pub async fn set_company_method(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(req): Json<SetCompanyAttendanceMethodRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;
    auth.require_company_admin()?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);

    attendance_service::set_company_attendance_method(
        &state.pool,
        company_id,
        req.method.as_deref(),
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

// ─── QR Code Generation (admin) ───

pub async fn generate_qr_token(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<QrTokenResponse>> {
    let company_id = auth.company_id()?;
    auth.require_attendance_qr_generator()?;

    // Also works for super_admin when managing a company
    let resp =
        attendance_service::generate_qr_token(&state.pool, company_id, &state.config.frontend_url)
            .await?;
    Ok(Json(resp))
}

// ─── Check In: QR ───

pub async fn check_in_qr(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CheckInQrRequest>,
) -> AppResult<Json<AttendanceRecord>> {
    let employee_id = auth.employee_id()?;
    let company_id = auth.company_id()?;

    // Verify method is QR
    let effective = attendance_service::get_effective_method(&state.pool, company_id).await?;
    if effective.method != "qr_code" {
        return Err(AppError::BadRequest(
            "QR code check-in is not enabled for this company".into(),
        ));
    }

    let record = attendance_service::check_in_qr(
        &state.pool,
        employee_id,
        company_id,
        &req.token,
        req.latitude,
        req.longitude,
    )
    .await?;

    Ok(Json(record))
}

// ─── Check In: Face ID ───

pub async fn check_in_face_id(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CheckInFaceIdRequest>,
) -> AppResult<Json<AttendanceRecord>> {
    let employee_id = auth.employee_id()?;
    let company_id = auth.company_id()?;

    // Verify method is face_id
    let effective = attendance_service::get_effective_method(&state.pool, company_id).await?;
    if effective.method != "face_id" {
        return Err(AppError::BadRequest(
            "Face ID check-in is not enabled for this company".into(),
        ));
    }

    // The front-end has already completed WebAuthn assertion (authentication_complete passkey flow).
    // Here we trust auth JWT — the user is already authenticated. Face ID is used as a UX signal.
    // For a stricter flow, you would verify the passkey assertion here via webauthn_rs.
    // Using the credential_id presence as a minimal server-side check.
    if req.credential_id.is_empty() {
        return Err(AppError::BadRequest(
            "Missing Face ID credential information".into(),
        ));
    }

    let record = attendance_service::check_in_face_id(
        &state.pool,
        employee_id,
        company_id,
        req.latitude,
        req.longitude,
    )
    .await?;

    Ok(Json(record))
}

// ─── Check Out ───

pub async fn check_out(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CheckOutRequest>,
) -> AppResult<Json<AttendanceRecord>> {
    let employee_id = auth.employee_id()?;
    let company_id = auth.company_id()?;

    let record = attendance_service::check_out(
        &state.pool,
        employee_id,
        company_id,
        req.latitude,
        req.longitude,
    )
    .await?;
    Ok(Json(record))
}

// ─── Today's Status (employee) ───

pub async fn my_today(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let employee_id = auth.employee_id()?;
    let company_id = auth.company_id()?;

    let record =
        attendance_service::get_today_checkin(&state.pool, employee_id, company_id).await?;

    Ok(Json(serde_json::json!({ "record": record })))
}

// ─── My Attendance History (employee portal) ───

pub async fn my_attendance(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<AttendanceListQuery>,
) -> AppResult<Json<PaginatedAttendance<AttendanceRecord>>> {
    let employee_id = auth.employee_id()?;

    let result = attendance_service::get_my_attendance(&state.pool, employee_id, &q).await?;
    Ok(Json(result))
}

// ─── Admin: List All Records ───

pub async fn list_attendance(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<AttendanceListQuery>,
) -> AppResult<Json<PaginatedAttendance<AttendanceRecordWithEmployee>>> {
    let company_id = auth.company_id()?;
    auth.require_non_employee()?;

    let result = attendance_service::list_attendance(&state.pool, company_id, &q).await?;
    Ok(Json(result))
}

// ─── Admin: Manual Entry ───

pub async fn manual_attendance(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(req): Json<ManualAttendanceRequest>,
) -> AppResult<Json<AttendanceRecord>> {
    let company_id = auth.company_id()?;
    auth.require_hr_admin()?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);

    let record = attendance_service::manual_attendance(
        &state.pool,
        company_id,
        req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(record))
}

// ─── Attendance Summary ───

pub async fn attendance_summary(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<AttendanceSummaryQuery>,
) -> AppResult<Json<Vec<AttendanceSummaryItem>>> {
    let company_id = auth.company_id()?;
    auth.require_non_employee()?;

    let items = attendance_service::get_attendance_summary(&state.pool, company_id, &q).await?;
    Ok(Json(items))
}

// ─── CSV Export ───

pub async fn export_attendance(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<AttendanceExportQuery>,
) -> AppResult<Response<Body>> {
    let company_id = auth.company_id()?;
    auth.require_non_employee()?;

    let csv = attendance_service::export_attendance_csv(&state.pool, company_id, &q).await?;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"attendance.csv\"",
        )
        .body(Body::from(csv))
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(response)
}

// ─── Admin: Edit/Correct Attendance Record ───

pub async fn update_attendance(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateAttendanceRecordRequest>,
) -> AppResult<Json<AttendanceRecord>> {
    let company_id = auth.company_id()?;
    auth.require_hr_admin()?;

    let record =
        attendance_service::update_attendance_record(&state.pool, company_id, id, &req, auth.0.sub)
            .await?;
    Ok(Json(record))
}

// ─── Kiosk Credentials (admin) ───

#[derive(Debug, Deserialize)]
pub struct CreateKioskCredentialRequest {
    pub label: String,
}

#[derive(Debug, serde::Serialize)]
pub struct CreateKioskCredentialResponse {
    pub credential: KioskCredential,
    /// Plaintext secret. Only returned at creation time — never persisted in plaintext
    /// and never returned by `list_kiosk_credentials`. Admin must copy it now.
    pub secret: String,
    pub public_url: String,
}

pub async fn create_kiosk_credential(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(req): Json<CreateKioskCredentialRequest>,
) -> AppResult<Json<CreateKioskCredentialResponse>> {
    let company_id = auth.company_id()?;
    auth.require_kiosk_admin()?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);

    let (credential, secret) = attendance_service::create_kiosk_credential(
        &state.pool,
        company_id,
        &req.label,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;

    let public_url = format!(
        "{}/kiosk/{}",
        state.config.frontend_url.trim_end_matches('/'),
        secret
    );

    Ok(Json(CreateKioskCredentialResponse {
        credential,
        secret,
        public_url,
    }))
}

pub async fn list_kiosk_credentials(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<KioskCredential>>> {
    let company_id = auth.company_id()?;
    auth.require_kiosk_admin()?;

    let creds = attendance_service::list_kiosk_credentials(&state.pool, company_id).await?;
    Ok(Json(creds))
}

pub async fn revoke_kiosk_credential(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;
    auth.require_kiosk_admin()?;
    let audit_meta = AuditRequestMeta::from_headers(&headers);

    attendance_service::revoke_kiosk_credential(
        &state.pool,
        id,
        company_id,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ─── Kiosk QR (PUBLIC — no AuthUser) ───

/// Pull the bearer secret out of the `Authorization` header. Accepts either
/// `Authorization: Kiosk <secret>` (preferred) or `Authorization: Bearer <secret>` so
/// existing axios setups that always send Bearer keep working without a special case
/// in the frontend interceptor.
fn extract_kiosk_secret(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let mut parts = raw.splitn(2, ' ');
    let scheme = parts.next()?;
    let value = parts.next()?.trim();
    if value.is_empty() {
        return None;
    }
    if scheme.eq_ignore_ascii_case("Kiosk") || scheme.eq_ignore_ascii_case("Bearer") {
        Some(value.to_string())
    } else {
        None
    }
}

/// Best-effort client IP for forensic audit. In prod the ALB/CloudFront sets
/// `X-Forwarded-For`; locally this returns None which is fine.
fn client_ip_string(headers: &HeaderMap) -> Option<String> {
    let xff = headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if xff.is_some() {
        return xff;
    }
    headers
        .get("x-real-ip")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Public endpoint. Reads the kiosk secret from the `Authorization` header,
/// validates it, and returns the same `QrTokenResponse` shape as the authenticated
/// `generate_qr_token`. NEVER log the presented secret.
pub async fn kiosk_qr(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<QrTokenResponse>> {
    let secret = extract_kiosk_secret(&headers)
        .ok_or_else(|| AppError::Unauthorized("Missing kiosk credential".into()))?;

    let ip = client_ip_string(&headers);

    let resp = attendance_service::generate_qr_via_kiosk(
        &state.pool,
        &secret,
        &state.config.frontend_url,
        ip.as_deref(),
    )
    .await?;

    Ok(Json(resp))
}
