use axum::{
    Json,
    extract::{Multipart, Query, State},
    http::header,
    response::IntoResponse,
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, Permission};
use crate::core::error::{AppError, AppResult, multipart_error, payload_too_large};
use crate::models::employee_import::{
    ImportConfirmRequest, ImportValidationResponse, TemplateQuery,
};
use crate::services::employee_import_service;

/// Largest spreadsheet this endpoint will accept. Well above the row cap the
/// validator enforces, so the size check is a guard rather than the policy.
pub const IMPORT_FILE_MAX_BYTES: usize = 20 * 1024 * 1024;

/// The request ceiling attached to `/employees/import/validate` in
/// `routes/mod.rs`: the file plus a megabyte for the multipart envelope. Until
/// this existed the route inherited axum's 2 MiB default, so a 3 MB XLSX well
/// inside the row cap failed as a malformed upload.
pub const IMPORT_REQUEST_MAX_BYTES: usize = IMPORT_FILE_MAX_BYTES + 1024 * 1024;

fn require_payroll_admin(auth: &AuthUser) -> AppResult<(Uuid, Uuid)> {
    auth.require_permission(Permission::ImportEmployees)?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;
    Ok((company_id, auth.0.sub))
}

pub async fn download_template(
    auth: AuthUser,
    Query(query): Query<TemplateQuery>,
) -> Result<impl IntoResponse, AppError> {
    require_payroll_admin(&auth)?;

    let format = query.format.as_deref().unwrap_or("xlsx");

    match format {
        "csv" => {
            let data = employee_import_service::generate_template_csv()?;
            Ok((
                [
                    (header::CONTENT_TYPE, "text/csv".to_string()),
                    (
                        header::CONTENT_DISPOSITION,
                        "attachment; filename=\"employee_import_template.csv\"".to_string(),
                    ),
                ],
                data,
            ))
        }
        _ => {
            let data = employee_import_service::generate_template_xlsx()?;
            Ok((
                [
                    (
                        header::CONTENT_TYPE,
                        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                            .to_string(),
                    ),
                    (
                        header::CONTENT_DISPOSITION,
                        "attachment; filename=\"employee_import_template.xlsx\"".to_string(),
                    ),
                ],
                data,
            ))
        }
    }
}

pub async fn validate_import(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<ImportValidationResponse>> {
    let (company_id, user_id) = require_payroll_admin(&auth)?;

    let mut file_data: Option<(String, Vec<u8>)> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| multipart_error(&e, "the upload", IMPORT_REQUEST_MAX_BYTES))?
    {
        if field.name() == Some("file") {
            let file_name = field.file_name().unwrap_or("upload").to_string();
            let data = field
                .bytes()
                .await
                .map_err(|e| multipart_error(&e, "the file data", IMPORT_REQUEST_MAX_BYTES))?;

            // Bounds the one part; the route layer bounds the whole request.
            if data.len() > IMPORT_FILE_MAX_BYTES {
                return Err(payload_too_large("The import file", IMPORT_FILE_MAX_BYTES));
            }

            file_data = Some((file_name, data.to_vec()));
            break;
        }
    }

    let (file_name, data) = file_data
        .ok_or_else(|| AppError::BadRequest("No file uploaded. Include a 'file' field.".into()))?;

    let is_xlsx = file_name.ends_with(".xlsx") || file_name.ends_with(".xls");
    let is_csv = file_name.ends_with(".csv");

    if !is_xlsx && !is_csv {
        return Err(AppError::BadRequest(
            "Unsupported file format. Please upload a .csv or .xlsx file.".into(),
        ));
    }

    let response = employee_import_service::validate_file(
        &state.pool,
        company_id,
        user_id,
        &file_name,
        &data,
        is_xlsx,
    )
    .await?;

    Ok(Json(response))
}

pub async fn confirm_import(
    State(state): State<AppState>,
    auth: AuthUser,
    audit_meta: crate::services::audit_service::AuditRequestMeta,
    Json(req): Json<ImportConfirmRequest>,
) -> AppResult<Json<crate::models::employee_import::ImportConfirmResponse>> {
    let (company_id, user_id) = require_payroll_admin(&auth)?;

    let response = employee_import_service::confirm_import(
        &state.pool,
        company_id,
        user_id,
        req,
        Some(&audit_meta),
    )
    .await?;

    Ok(Json(response))
}
