use axum::{
    Json,
    extract::{Multipart, Path, Query, State},
};
use serde::Deserialize;
use uuid::Uuid;

use chrono::Datelike;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::employee::Employee;
use crate::models::portal::*;
use crate::services::portal_service;

fn get_employee_id(auth: &AuthUser) -> AppResult<Uuid> {
    auth.0
        .employee_id
        .ok_or_else(|| AppError::Forbidden("No employee profile linked".into()))
}

fn get_company_id(auth: &AuthUser) -> AppResult<Uuid> {
    auth.0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))
}

// ─── Profile ───

pub async fn get_profile(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Employee>> {
    let employee_id = get_employee_id(&auth)?;
    let employee = portal_service::get_my_profile(&state.pool, employee_id).await?;
    Ok(Json(employee))
}

// ─── Payslips ───

pub async fn list_payslips(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<MyPayslip>>> {
    let employee_id = get_employee_id(&auth)?;
    let payslips = portal_service::get_my_payslips(&state.pool, employee_id).await?;
    Ok(Json(payslips))
}

// ─── Leave ───

pub async fn leave_types(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<LeaveType>>> {
    let company_id = get_company_id(&auth)?;
    let types = portal_service::get_leave_types(&state.pool, company_id).await?;
    Ok(Json(types))
}

#[derive(Debug, Deserialize)]
pub struct LeaveBalanceQuery {
    pub year: Option<i32>,
}

pub async fn leave_balances(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<LeaveBalanceQuery>,
) -> AppResult<Json<Vec<LeaveBalanceWithType>>> {
    let employee_id = get_employee_id(&auth)?;
    let year = q.year.unwrap_or(2026);
    let balances = portal_service::get_leave_balances(&state.pool, employee_id, year).await?;
    Ok(Json(balances))
}

pub async fn leave_requests(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<LeaveRequest>>> {
    let employee_id = get_employee_id(&auth)?;
    let requests = portal_service::get_leave_requests(&state.pool, employee_id).await?;
    Ok(Json(requests))
}

pub async fn create_leave(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateLeaveRequest>,
) -> AppResult<Json<LeaveRequest>> {
    let employee_id = get_employee_id(&auth)?;
    let company_id = get_company_id(&auth)?;
    let leave =
        portal_service::create_leave_request(&state.pool, employee_id, company_id, req).await?;
    Ok(Json(leave))
}

pub async fn cancel_leave(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let employee_id = get_employee_id(&auth)?;
    portal_service::cancel_leave_request(&state.pool, employee_id, id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn delete_leave(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let employee_id = get_employee_id(&auth)?;
    portal_service::delete_leave_request(&state.pool, employee_id, id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

// ─── Claims ───

#[derive(Debug, Deserialize)]
pub struct ClaimQuery {
    pub status: Option<String>,
}

pub async fn list_claims(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<ClaimQuery>,
) -> AppResult<Json<Vec<Claim>>> {
    let employee_id = get_employee_id(&auth)?;
    let claims = portal_service::get_claims(&state.pool, employee_id, q.status.as_deref()).await?;
    Ok(Json(claims))
}

pub async fn create_claim(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateClaimRequest>,
) -> AppResult<Json<Claim>> {
    let employee_id = get_employee_id(&auth)?;
    let company_id = get_company_id(&auth)?;
    let claim = portal_service::create_claim(&state.pool, employee_id, company_id, req).await?;
    Ok(Json(claim))
}

pub async fn submit_claim(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Claim>> {
    let employee_id = get_employee_id(&auth)?;
    let claim = portal_service::submit_claim(&state.pool, employee_id, id).await?;
    Ok(Json(claim))
}

pub async fn cancel_claim(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let employee_id = get_employee_id(&auth)?;
    portal_service::cancel_claim(&state.pool, employee_id, id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn delete_claim(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let employee_id = get_employee_id(&auth)?;
    portal_service::delete_claim(&state.pool, employee_id, id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

// ─── Overtime ───

pub async fn list_overtime(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<crate::models::portal::OvertimeApplication>>> {
    let employee_id = get_employee_id(&auth)?;
    let apps = portal_service::get_overtime_applications(&state.pool, employee_id).await?;
    Ok(Json(apps))
}

pub async fn create_overtime(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<crate::models::portal::CreateOvertimeRequest>,
) -> AppResult<Json<crate::models::portal::OvertimeApplication>> {
    let employee_id = get_employee_id(&auth)?;
    let company_id = get_company_id(&auth)?;
    let app =
        portal_service::create_overtime_application(&state.pool, employee_id, company_id, req)
            .await?;
    Ok(Json(app))
}

pub async fn cancel_overtime(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let employee_id = get_employee_id(&auth)?;
    portal_service::cancel_overtime_application(&state.pool, employee_id, id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn delete_overtime(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let employee_id = get_employee_id(&auth)?;
    portal_service::delete_overtime_application(&state.pool, employee_id, id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

// ─── File Upload ───

const MAX_UPLOAD_SIZE: usize = 10 * 1024 * 1024; // 10 MB
const ALLOWED_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "pdf", "doc", "docx", "xls", "xlsx",
];

pub async fn upload_file(
    _auth: AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<serde_json::Value>> {
    let field = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart data: {}", e)))?
        .ok_or_else(|| AppError::BadRequest("No file provided".into()))?;

    let original_name = field
        .file_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "upload".to_string());

    let ext = original_name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();

    if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::BadRequest(format!(
            "File type .{} is not allowed. Allowed: {}",
            ext,
            ALLOWED_EXTENSIONS.join(", ")
        )));
    }

    let data = field
        .bytes()
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to read file: {}", e)))?;

    if data.len() > MAX_UPLOAD_SIZE {
        return Err(AppError::BadRequest(format!(
            "File too large. Maximum size is {} MB",
            MAX_UPLOAD_SIZE / 1024 / 1024
        )));
    }

    // Validate file content matches claimed extension (magic number check)
    if !validate_magic_bytes(&data, &ext) {
        return Err(AppError::BadRequest(
            "File content does not match its extension".into(),
        ));
    }

    // Save to uploads directory
    let upload_dir = std::path::Path::new("uploads");
    tokio::fs::create_dir_all(upload_dir)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create upload dir: {}", e)))?;

    let stored_name = format!(
        "{}_{}.{}",
        Uuid::new_v4(),
        sanitize_filename(&original_name),
        ext
    );
    let file_path = upload_dir.join(&stored_name);

    tokio::fs::write(&file_path, &data)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to save file: {}", e)))?;

    let file_url = format!("/api/uploads/{}", stored_name);

    Ok(Json(serde_json::json!({
        "url": file_url,
        "file_name": original_name,
        "size": data.len(),
    })))
}

// ─── Team Calendar & Holidays ───

#[derive(Debug, Deserialize)]
pub struct TeamCalendarQuery {
    pub year: Option<i32>,
    pub month: Option<u32>,
}

pub async fn team_calendar(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<TeamCalendarQuery>,
) -> AppResult<Json<Vec<portal_service::TeamLeaveEntry>>> {
    let employee_id = get_employee_id(&auth)?;
    let company_id = get_company_id(&auth)?;
    let now = chrono::Utc::now();
    let year = q.year.unwrap_or(now.year());
    let month = q.month.unwrap_or(now.month());
    let entries =
        portal_service::get_team_calendar(&state.pool, employee_id, company_id, year, month)
            .await?;
    Ok(Json(entries))
}

pub async fn my_teams(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<crate::models::team::Team>>> {
    let employee_id = get_employee_id(&auth)?;
    let teams = crate::services::team_service::get_employee_teams(&state.pool, employee_id).await?;
    Ok(Json(teams))
}

pub async fn portal_holidays(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<TeamCalendarQuery>,
) -> AppResult<Json<Vec<crate::models::calendar::Holiday>>> {
    let company_id = get_company_id(&auth)?;
    let year = q.year.unwrap_or(chrono::Utc::now().year());
    let holidays =
        crate::services::calendar_service::get_holidays(&state.pool, company_id, year).await?;
    Ok(Json(holidays))
}

// ─── Leave Calendar Export (.ics) ───

pub async fn export_leave_ics(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<LeaveBalanceQuery>,
) -> Result<axum::response::Response, AppError> {
    use axum::body::Body;
    use axum::http::{Response, StatusCode, header};

    let employee_id = get_employee_id(&auth)?;
    let year = q.year.unwrap_or(chrono::Utc::now().year());

    let leaves = sqlx::query_as::<_, LeaveRequest>(
        r#"SELECT lr.id, lr.employee_id, lr.company_id, lr.leave_type_id,
            lr.start_date, lr.end_date, lr.days, lr.reason, lr.status,
            lr.reviewed_by, lr.reviewed_at, lr.review_notes,
            lr.attachment_url, lr.attachment_name,
            lr.created_at, lr.updated_at,
            lt.name as leave_type_name
        FROM leave_requests lr
        JOIN leave_types lt ON lr.leave_type_id = lt.id
        WHERE lr.employee_id = $1 AND lr.status = 'approved'
        AND EXTRACT(YEAR FROM lr.start_date) = $2
        ORDER BY lr.start_date"#,
    )
    .bind(employee_id)
    .bind(year)
    .fetch_all(&state.pool)
    .await?;

    let mut ics = String::from(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//PayrollMY//Leave Calendar//EN\r\nCALSCALE:GREGORIAN\r\n",
    );

    for lr in &leaves {
        let uid = lr.id;
        let summary = lr.leave_type_name.as_deref().unwrap_or("Leave");
        let dtstart = lr.start_date.format("%Y%m%d");
        // DTEND in VEVENT DATE type is exclusive, so add 1 day
        let dtend = lr
            .end_date
            .succ_opt()
            .map(|d| d.format("%Y%m%d").to_string())
            .unwrap_or_default();
        let description = lr.reason.as_deref().unwrap_or("");

        ics.push_str(&format!(
            "BEGIN:VEVENT\r\nUID:{uid}@payrollmy\r\nDTSTART;VALUE=DATE:{dtstart}\r\nDTEND;VALUE=DATE:{dtend}\r\nSUMMARY:{summary}\r\nDESCRIPTION:{description}\r\nSTATUS:CONFIRMED\r\nEND:VEVENT\r\n"
        ));
    }

    ics.push_str("END:VCALENDAR\r\n");

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"leave-{}.ics\"", year),
        )
        .body(Body::from(ics))
        .unwrap())
}

// ─── Payslip PDF Download ───

pub async fn download_payslip_pdf(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<axum::response::Response, AppError> {
    use axum::body::Body;
    use axum::http::{Response, StatusCode, header};

    let employee_id = get_employee_id(&auth)?;
    let bytes =
        crate::services::payslip_pdf_service::generate_payslip_pdf(&state.pool, id, employee_id)
            .await?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"payslip.pdf\"",
        )
        .body(Body::from(bytes))
        .unwrap())
}

fn validate_magic_bytes(data: &[u8], claimed_ext: &str) -> bool {
    match claimed_ext {
        "pdf" => data.starts_with(b"%PDF"),
        "jpg" | "jpeg" => data.starts_with(&[0xFF, 0xD8, 0xFF]),
        "png" => data.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
        "gif" => data.starts_with(b"GIF8"),
        "webp" => data.len() >= 12 && &data[8..12] == b"WEBP",
        "doc" | "xls" => data.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]),
        "docx" | "xlsx" => data.starts_with(&[0x50, 0x4B, 0x03, 0x04]),
        _ => false,
    }
}

fn sanitize_filename(name: &str) -> String {
    let stem = name
        .rsplit('.')
        .skip(1)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(".");
    stem.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(50)
        .collect()
}

pub async fn serve_upload(
    Path(filename): Path<String>,
) -> Result<axum::response::Response, AppError> {
    use axum::body::Body;
    use axum::http::{Response, StatusCode, header};

    // Prevent directory traversal
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(AppError::BadRequest("Invalid filename".into()));
    }

    let file_path = std::path::Path::new("uploads").join(&filename);

    let data = tokio::fs::read(&file_path)
        .await
        .map_err(|_| AppError::NotFound("File not found".into()))?;

    let content_type = match filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "application/octet-stream",
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from(data))
        .unwrap())
}
