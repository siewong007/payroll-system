use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, Response, StatusCode, header},
    response::IntoResponse,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, Permission};
use crate::core::cookie;
use crate::core::error::{AppError, AppResult};
use crate::core::extract::{ClientIp, ValidatedJson};
use crate::models::attendance::{
    AttendanceAnomaliesQuery, AttendanceAnomaliesReport, AttendanceExportQuery,
    AttendanceListQuery, AttendanceMethodResponse, AttendanceRecord, AttendanceRecordWithEmployee,
    AttendanceSecuritySettings, AttendanceSummaryItem, AttendanceSummaryQuery,
    CheckInFaceIdRequest, CheckInQrFaceIdRequest, CheckInQrRequest, CheckOutRequest,
    CreatePersonalQrRequest, DeviceChallengeRequest, DeviceChallengeResponse,
    EmployeeAttendanceDevice, EmployeeAttendanceDeviceWithEmployee, EnrollDeviceRequest,
    KioskCheckInRequest, KioskCheckInResponse, ManualAttendanceRequest, PaginatedAttendance,
    QrTokenResponse, RevokeDeviceRequest, SetAttendanceMethodRequest,
    SetCompanyAttendanceMethodRequest, UpdateAttendanceRecordRequest,
    UpdateAttendanceSecuritySettingsRequest,
};
use crate::models::attendance_kiosk::{
    CreateKioskCredentialRequest, CreateKioskCredentialResponse, KioskCredential,
};
use crate::models::user_company::StepUpConfirmRequest;
use crate::services::attendance_service::CheckInContext;
use crate::services::{
    attendance_device_service, attendance_service, audit_service::AuditRequestMeta,
};

#[derive(Debug, Deserialize)]
pub struct DeviceListQuery {
    pub status: Option<String>,
}

fn extract_device_cookie_and_ua(
    headers: &HeaderMap,
    state: &AppState,
) -> (Uuid, Option<(header::HeaderName, String)>, Option<String>) {
    let (cookie_id, set_cookie) = cookie::extract_or_mint_attendance_device_cookie(
        headers,
        &state.config.jwt_secret,
        &state.config.frontend_url,
    );
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(500).collect());
    (cookie_id, set_cookie, user_agent)
}

fn json_with_cookie<T: serde::Serialize>(
    body: T,
    set_cookie: Option<(header::HeaderName, String)>,
) -> axum::response::Response {
    let mut headers = HeaderMap::new();
    if let Some((name, val)) = set_cookie
        && let Ok(hv) = val.parse()
    {
        headers.insert(name, hv);
    }
    (headers, Json(body)).into_response()
}

// ─── Effective Method ───

/// Returns the effective attendance method for the caller's company, plus the
/// geofence mode and company timezone the clients need to behave well.
pub async fn get_attendance_method(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<AttendanceMethodResponse>> {
    let company_id = auth.company_id()?;

    let resp = attendance_service::get_attendance_bootstrap(&state.pool, company_id).await?;
    Ok(Json(resp))
}

// ─── Platform (super_admin only) ───

pub async fn get_platform_method(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_permission(Permission::ManagePlatformSettings)?;

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
    audit_meta: AuditRequestMeta,
    Json(req): Json<SetAttendanceMethodRequest>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_permission(Permission::ManagePlatformSettings)?;
    crate::handlers::auth::require_step_up(
        &state,
        &auth,
        req.actor_password.as_deref(),
        req.actor_totp_code.as_deref(),
        Some(&audit_meta),
    )
    .await?;

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
    audit_meta: AuditRequestMeta,
    Json(req): Json<SetCompanyAttendanceMethodRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::SetCompanyAttendanceMethod)?;

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
    audit_meta: AuditRequestMeta,
) -> AppResult<Json<QrTokenResponse>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::GenerateAttendanceQr)?;

    // Also works for super_admin when managing a company
    // None: the admin console is its own display surface, so regenerating here
    // does not retire tokens shown on the physical kiosks.
    let resp = attendance_service::generate_qr_token(
        &state.pool,
        company_id,
        &state.config.frontend_url,
        None,
    )
    .await?;

    // Audit under its own entity type (a token mint is not a credential
    // creation), and never persist the live token or scan URL — an audit
    // reader must not gain a usable check-in token.
    let _ = crate::services::audit_service::log_action_with_metadata(
        &state.pool,
        Some(company_id),
        Some(auth.0.sub),
        "create",
        "attendance_qr_token",
        None,
        None,
        Some(serde_json::json!({
            "expires_at": resp.expires_at,
            "ttl_seconds": resp.ttl_seconds,
            "issuer": "console",
        })),
        Some("Generated new attendance QR token"),
        Some(&audit_meta),
    )
    .await;

    Ok(Json(resp))
}

// ─── Check In: QR ───

pub async fn check_in_qr(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    audit_meta: AuditRequestMeta,
    ClientIp(client_ip): ClientIp,
    Json(req): Json<CheckInQrRequest>,
) -> AppResult<axum::response::Response> {
    let employee_id = auth.employee_id()?;
    let company_id = auth.company_id()?;

    // Verify method is QR
    let effective = attendance_service::get_effective_method(&state.pool, company_id).await?;
    if effective.method != "qr_code" {
        return Err(AppError::BadRequest(
            "QR code check-in is not enabled for this company".into(),
        ));
    }

    let (cookie_id, set_cookie, user_agent) = extract_device_cookie_and_ua(&headers, &state);
    let ctx = CheckInContext {
        user_id: Some(auth.0.sub),
        session_id: Some(auth.0.sid),
        cookie_device_id: Some(cookie_id),
        user_agent,
        passkey_id: None,
        passkey_is_synced: false,
        device_proof: req.device_proof,
        photo_base64: req.photo_base64,
    };

    let record = attendance_service::check_in_qr_with_context(
        &state.pool,
        employee_id,
        company_id,
        &req.token,
        req.latitude,
        req.longitude,
        client_ip,
        &ctx,
    )
    .await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        &state.pool,
        Some(company_id),
        Some(auth.0.sub),
        "create",
        "attendance_record",
        Some(record.id),
        None,
        Some(serde_json::to_value(&record).unwrap_or_default()),
        Some("Employee checked in via QR code"),
        Some(&audit_meta),
    )
    .await;

    Ok(json_with_cookie(record, set_cookie))
}

// ─── Check In: Face ID (and QR + Face ID) ───

/// Start the WebAuthn assertion ceremony for a face-id or combined qr_and_face_id
/// check-in. The completing call is `check_in_face_id` or `check_in_qr_face_id`.
pub async fn check_in_face_id_begin(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<crate::models::attendance::FaceIdBeginResponse>> {
    // Same preconditions as the completing call, checked early for a clear error.
    auth.employee_id()?;
    let company_id = auth.company_id()?;

    let effective = attendance_service::get_effective_method(&state.pool, company_id).await?;
    if effective.method != "face_id" && effective.method != "qr_and_face_id" {
        return Err(AppError::BadRequest(
            "Face ID check-in is not enabled for this company".into(),
        ));
    }

    let resp = attendance_service::face_id_begin(&state.pool, &state.webauthn, auth.0.sub).await?;
    Ok(Json(resp))
}

pub async fn check_in_face_id(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    audit_meta: AuditRequestMeta,
    ClientIp(client_ip): ClientIp,
    Json(req): Json<CheckInFaceIdRequest>,
) -> AppResult<axum::response::Response> {
    let employee_id = auth.employee_id()?;
    let company_id = auth.company_id()?;

    // Verify method is face_id
    let effective = attendance_service::get_effective_method(&state.pool, company_id).await?;
    if effective.method != "face_id" {
        return Err(AppError::BadRequest(
            "Face ID check-in is not enabled for this company".into(),
        ));
    }

    // Verify the passkey assertion server-side against this user's registered
    // credentials. A record whose method claims biometric presence must not be
    // creatable with a bare JWT.
    let face_id_info = attendance_service::face_id_verify(
        &state.pool,
        &state.webauthn,
        auth.0.sub,
        req.challenge_id,
        &req.credential,
    )
    .await?;

    let (cookie_id, set_cookie, user_agent) = extract_device_cookie_and_ua(&headers, &state);
    let ctx = CheckInContext {
        user_id: Some(auth.0.sub),
        session_id: Some(auth.0.sid),
        cookie_device_id: Some(cookie_id),
        user_agent,
        passkey_id: Some(face_id_info.passkey_id),
        passkey_is_synced: face_id_info.is_synced,
        device_proof: req.device_proof,
        photo_base64: req.photo_base64,
    };

    let record = attendance_service::check_in_face_id_with_context(
        &state.pool,
        employee_id,
        company_id,
        req.latitude,
        req.longitude,
        client_ip,
        &ctx,
    )
    .await?;

    // Audit parity with the QR path.
    let _ = crate::services::audit_service::log_action_with_metadata(
        &state.pool,
        Some(company_id),
        Some(auth.0.sub),
        "create",
        "attendance_record",
        Some(record.id),
        None,
        Some(serde_json::to_value(&record).unwrap_or_default()),
        Some("Employee checked in via Face ID"),
        Some(&audit_meta),
    )
    .await;

    Ok(json_with_cookie(record, set_cookie))
}

pub async fn check_in_qr_face_id(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    audit_meta: AuditRequestMeta,
    ClientIp(client_ip): ClientIp,
    Json(req): Json<CheckInQrFaceIdRequest>,
) -> AppResult<axum::response::Response> {
    let employee_id = auth.employee_id()?;
    let company_id = auth.company_id()?;

    let effective = attendance_service::get_effective_method(&state.pool, company_id).await?;
    if effective.method != "qr_and_face_id" {
        return Err(AppError::BadRequest(
            "Combined QR + Face ID check-in is not enabled for this company".into(),
        ));
    }

    // Validate QR token first so an expired QR does not burn the WebAuthn challenge.
    attendance_service::validate_qr_token(&state.pool, &req.token, company_id).await?;

    let face_id_info = attendance_service::face_id_verify(
        &state.pool,
        &state.webauthn,
        auth.0.sub,
        req.challenge_id,
        &req.credential,
    )
    .await?;

    let (cookie_id, set_cookie, user_agent) = extract_device_cookie_and_ua(&headers, &state);
    let ctx = CheckInContext {
        user_id: Some(auth.0.sub),
        session_id: Some(auth.0.sid),
        cookie_device_id: Some(cookie_id),
        user_agent,
        passkey_id: Some(face_id_info.passkey_id),
        passkey_is_synced: face_id_info.is_synced,
        device_proof: req.device_proof,
        photo_base64: req.photo_base64,
    };

    let record = attendance_service::check_in_qr_face_id(
        &state.pool,
        employee_id,
        company_id,
        &req.token,
        req.latitude,
        req.longitude,
        client_ip,
        &ctx,
    )
    .await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        &state.pool,
        Some(company_id),
        Some(auth.0.sub),
        "create",
        "attendance_record",
        Some(record.id),
        None,
        Some(serde_json::to_value(&record).unwrap_or_default()),
        Some("Employee checked in via QR code and Face ID"),
        Some(&audit_meta),
    )
    .await;

    Ok(json_with_cookie(record, set_cookie))
}

// ─── Check Out ───

pub async fn check_out(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    audit_meta: AuditRequestMeta,
    ClientIp(client_ip): ClientIp,
    Json(req): Json<CheckOutRequest>,
) -> AppResult<axum::response::Response> {
    let employee_id = auth.employee_id()?;
    let company_id = auth.company_id()?;

    let (cookie_id, set_cookie, user_agent) = extract_device_cookie_and_ua(&headers, &state);
    let ctx = CheckInContext {
        user_id: Some(auth.0.sub),
        session_id: Some(auth.0.sid),
        cookie_device_id: Some(cookie_id),
        user_agent,
        passkey_id: None,
        passkey_is_synced: false,
        device_proof: req.device_proof,
        photo_base64: req.photo_base64,
    };

    let record = attendance_service::check_out_with_context(
        &state.pool,
        employee_id,
        company_id,
        req.latitude,
        req.longitude,
        client_ip,
        &ctx,
    )
    .await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        &state.pool,
        Some(company_id),
        Some(auth.0.sub),
        "update",
        "attendance_record",
        Some(record.id),
        None,
        Some(serde_json::to_value(&record).unwrap_or_default()),
        Some("Employee checked out"),
        Some(&audit_meta),
    )
    .await;

    Ok(json_with_cookie(record, set_cookie))
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
    let company_id = auth.company_id()?;

    let result =
        attendance_service::get_my_attendance(&state.pool, employee_id, company_id, &q).await?;
    Ok(Json(result))
}

// ─── Admin: List All Records ───

pub async fn list_attendance(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<AttendanceListQuery>,
) -> AppResult<Json<PaginatedAttendance<AttendanceRecordWithEmployee>>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ViewAttendance)?;

    let result = attendance_service::list_attendance(&state.pool, company_id, &q).await?;
    Ok(Json(result))
}

// ─── Admin: Manual Entry ───

pub async fn manual_attendance(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Json(req): Json<ManualAttendanceRequest>,
) -> AppResult<Json<AttendanceRecord>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendance)?;

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
    auth.require_permission(Permission::ViewAttendance)?;

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
    auth.require_permission(Permission::ViewAttendance)?;

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
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateAttendanceRecordRequest>,
) -> AppResult<Json<AttendanceRecord>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendance)?;

    let record = attendance_service::update_attendance_record(
        &state.pool,
        company_id,
        id,
        &req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(record))
}

// ─── Admin: Auto-Absent Backfill ───

/// Run the auto-absent marking for one past local date. Idempotent (dates
/// that already have records are skipped), so re-running is safe. Covers the
/// case where the daily job was down during its window.
pub async fn absent_run(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Json(req): Json<crate::models::attendance::AbsentRunRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendance)?;

    let marked =
        attendance_service::mark_absent_for_company_date(&state.pool, company_id, req.date).await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        &state.pool,
        Some(company_id),
        Some(auth.0.sub),
        "create",
        "attendance_record",
        None,
        None,
        Some(serde_json::json!({ "date": req.date, "marked": marked })),
        Some("Auto-absent marking run manually"),
        Some(&audit_meta),
    )
    .await;

    Ok(Json(
        serde_json::json!({ "marked": marked, "date": req.date }),
    ))
}

// ─── Kiosk Credentials (admin) ───

pub async fn create_kiosk_credential(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Json(req): Json<CreateKioskCredentialRequest>,
) -> AppResult<Json<CreateKioskCredentialResponse>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageKiosks)?;
    crate::handlers::auth::require_step_up(
        &state,
        &auth,
        req.actor_password.as_deref(),
        req.actor_totp_code.as_deref(),
        Some(&audit_meta),
    )
    .await?;

    let (credential, secret) = attendance_service::create_kiosk_credential(
        &state.pool,
        company_id,
        &req.label,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;

    let public_url = format!(
        "{}/kiosk#{}",
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
    auth.require_permission(Permission::ManageKiosks)?;

    let creds = attendance_service::list_kiosk_credentials(&state.pool, company_id).await?;
    Ok(Json(creds))
}

pub async fn revoke_kiosk_credential(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<StepUpConfirmRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageKiosks)?;
    crate::handlers::auth::require_step_up(
        &state,
        &auth,
        Some(&req.actor_password),
        req.actor_totp_code.as_deref(),
        Some(&audit_meta),
    )
    .await?;

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

/// Public endpoint. Reads the kiosk secret from the `Authorization` header,
/// validates it, and returns the same `QrTokenResponse` shape as the authenticated
/// `generate_qr_token`. NEVER log the presented secret.
pub async fn kiosk_qr(
    State(state): State<AppState>,
    audit_meta: AuditRequestMeta,
    headers: HeaderMap,
) -> AppResult<Json<QrTokenResponse>> {
    let secret = extract_kiosk_secret(&headers)
        .ok_or_else(|| AppError::Unauthorized("Missing kiosk credential".into()))?;

    // `last_used_ip` is forensic evidence about an unauthenticated surface, so
    // it resolves through the same `core::client_ip` path as the audit trail
    // and the rate limiter. The local helper this replaced read the left-most
    // `X-Forwarded-For` entry unconditionally — the one value the caller sets.
    let ip = audit_meta.ip_address.as_deref();

    let (resp, _company_id) = attendance_service::generate_qr_via_kiosk(
        &state.pool,
        &secret,
        &state.config.frontend_url,
        ip,
    )
    .await?;

    // No audit row per mint: an always-on kiosk refreshes ~288×/day, which
    // drowned the audit trail (mislabeled as credential creations, carrying
    // the live token). The credential's last_used_at/last_used_ip heartbeat —
    // updated by generate_qr_via_kiosk — is the kiosk activity record.

    Ok(Json(resp))
}

// ─── Anti-Buddy-Punching: Bound Devices, Security Settings, Anomalies, Personal QR ───

pub async fn create_device_challenge(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<DeviceChallengeRequest>,
) -> AppResult<Json<DeviceChallengeResponse>> {
    let employee_id = auth.employee_id()?;
    let company_id = auth.company_id()?;
    let resp = attendance_device_service::issue_challenge(
        &state.pool,
        company_id,
        auth.0.sub,
        employee_id,
        &req.purpose,
    )
    .await?;
    Ok(Json(resp))
}

pub async fn enroll_device(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    audit_meta: AuditRequestMeta,
    Json(req): Json<EnrollDeviceRequest>,
) -> AppResult<axum::response::Response> {
    let employee_id = auth.employee_id()?;
    let company_id = auth.company_id()?;
    let (cookie_id, set_cookie, user_agent) = extract_device_cookie_and_ua(&headers, &state);
    let device = attendance_device_service::enroll_device(
        &state.pool,
        company_id,
        employee_id,
        auth.0.sub,
        &req,
        Some(cookie_id),
        user_agent.as_deref(),
        Some(&audit_meta),
    )
    .await?;
    Ok(json_with_cookie(device, set_cookie))
}

pub async fn my_devices(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<EmployeeAttendanceDevice>>> {
    let employee_id = auth.employee_id()?;
    let company_id = auth.company_id()?;
    let devices =
        attendance_device_service::list_my_devices(&state.pool, company_id, employee_id).await?;
    Ok(Json(devices))
}

pub async fn list_company_devices(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<DeviceListQuery>,
) -> AppResult<Json<Vec<EmployeeAttendanceDeviceWithEmployee>>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ViewAttendance)?;
    let devices = attendance_device_service::list_company_devices(
        &state.pool,
        company_id,
        q.status.as_deref(),
    )
    .await?;
    Ok(Json(devices))
}

pub async fn approve_device(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
) -> AppResult<Json<EmployeeAttendanceDevice>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendance)?;
    let device = attendance_device_service::approve_device(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(device))
}

pub async fn revoke_device(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Path(id): Path<Uuid>,
    Json(req): Json<RevokeDeviceRequest>,
) -> AppResult<Json<EmployeeAttendanceDevice>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendance)?;
    let device = attendance_device_service::revoke_device(
        &state.pool,
        company_id,
        id,
        auth.0.sub,
        req.reason.as_deref(),
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(device))
}

pub async fn get_security_settings(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<AttendanceSecuritySettings>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ViewAttendance)?;
    let settings =
        attendance_device_service::get_security_settings(&state.pool, company_id).await?;
    Ok(Json(settings))
}

pub async fn update_security_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: AuditRequestMeta,
    Json(req): Json<UpdateAttendanceSecuritySettingsRequest>,
) -> AppResult<Json<AttendanceSecuritySettings>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ManageAttendance)?;
    let settings = attendance_device_service::update_security_settings(
        &state.pool,
        company_id,
        &req,
        auth.0.sub,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(settings))
}

pub async fn get_anomalies(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<AttendanceAnomaliesQuery>,
) -> AppResult<Json<AttendanceAnomaliesReport>> {
    let company_id = auth.company_id()?;
    auth.require_permission(Permission::ViewAttendance)?;
    let report = attendance_service::get_attendance_anomalies(&state.pool, company_id, &q).await?;
    Ok(Json(report))
}

pub async fn create_personal_qr(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(req): Json<CreatePersonalQrRequest>,
) -> AppResult<axum::response::Response> {
    let employee_id = auth.employee_id()?;
    let company_id = auth.company_id()?;
    let (cookie_id, set_cookie, user_agent) = extract_device_cookie_and_ua(&headers, &state);
    let ctx = CheckInContext {
        user_id: Some(auth.0.sub),
        session_id: Some(auth.0.sid),
        cookie_device_id: Some(cookie_id),
        user_agent,
        passkey_id: None,
        passkey_is_synced: false,
        device_proof: None,
        photo_base64: None,
    };
    let resp = attendance_service::create_personal_qr_token(
        &state.pool,
        company_id,
        employee_id,
        auth.0.sub,
        &req,
        &ctx,
    )
    .await?;
    Ok(json_with_cookie(resp, set_cookie))
}

pub async fn kiosk_check_in(
    State(state): State<AppState>,
    audit_meta: AuditRequestMeta,
    headers: HeaderMap,
    Json(req): Json<KioskCheckInRequest>,
) -> AppResult<Json<KioskCheckInResponse>> {
    let secret = extract_kiosk_secret(&headers)
        .ok_or_else(|| AppError::Unauthorized("Missing kiosk credential".into()))?;
    let ip = audit_meta.ip_address.as_deref();
    let resp = attendance_service::kiosk_camera_check_in(
        &state.pool,
        &secret,
        &req,
        ip,
        Some(&audit_meta),
    )
    .await?;
    Ok(Json(resp))
}
