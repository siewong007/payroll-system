use axum::{
    Json,
    body::Bytes,
    extract::{Multipart, Query, State},
    http::header,
    response::IntoResponse,
};
use chrono::Utc;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::{AuthUser, Permission};
use crate::core::error::{AppError, AppResult, multipart_error, payload_too_large};
use crate::models::backup::{CompanyBackup, ExportQuery, ImportResult};
use crate::services::backup_service;

/// Largest backup document this endpoint will accept. A backup carries every
/// payroll row a tenant has ever produced, so the export can genuinely reach
/// tens of megabytes — the point of stating it here is that
/// [`BACKUP_REQUEST_MAX_BYTES`] is derived from it and cannot drift.
pub const BACKUP_FILE_MAX_BYTES: usize = 100 * 1024 * 1024;

/// The request ceiling attached to `/admin/backup/import` in `routes/mod.rs`:
/// the file plus a megabyte of slack for the multipart envelope and the
/// `create_new` / `company_id` text fields. Without this the route inherited
/// axum's 2 MiB default and rejected any real backup before the handler ran.
pub const BACKUP_REQUEST_MAX_BYTES: usize = BACKUP_FILE_MAX_BYTES + 1024 * 1024;

/// A company backup carries payroll_runs/items/entries, salary_history and raw
/// employee rows (bank account, IC, TIN), and import overwrites those tables.
/// `Permission::ManageBackups` is granted to `super_admin` alone.
///
/// This previously required `ViewPayroll` *and* membership of
/// `super_admin`/`admin`. Since `admin` is deliberately excluded from
/// `ViewPayroll`, the `admin` branch was unreachable, and `payroll_admin` /
/// `finance` cleared the first check only to fail the second — so the effective
/// roster was already `super_admin` alone. That is now stated directly instead
/// of emerging from two checks that contradict each other.
fn require_backup_admin(auth: &AuthUser) -> AppResult<(Option<Uuid>, Uuid)> {
    auth.require_permission(Permission::ManageBackups)?;
    Ok((auth.0.company_id, auth.0.sub))
}

pub async fn export_company(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ExportQuery>,
) -> Result<impl IntoResponse, AppError> {
    let (user_company_id, _user_id) = require_backup_admin(&auth)?;

    let company_id = if auth.has_any_role(&["super_admin"]) {
        query.company_id.ok_or_else(|| {
            AppError::BadRequest("company_id query parameter is required for super_admin".into())
        })?
    } else {
        user_company_id.unwrap()
    };

    let backup = backup_service::export_company(&state.pool, company_id).await?;

    let json = serde_json::to_vec_pretty(&backup)
        .map_err(|e| AppError::Internal(format!("Serialization failed: {}", e)))?;

    let filename = format!(
        "backup_{}_{}.json",
        backup
            .metadata
            .source_company_name
            .replace(' ', "_")
            .replace(|c: char| !c.is_alphanumeric() && c != '_', ""),
        Utc::now().format("%Y%m%d_%H%M%S")
    );

    Ok((
        [
            (header::CONTENT_TYPE, "application/json".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        json,
    ))
}

pub async fn import_company(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<ImportResult>> {
    let (admin_company_id, user_id) = require_backup_admin(&auth)?;

    let mut file_data: Option<Bytes> = None;
    let mut requested_company_id: Option<Uuid> = None;
    let mut create_new = false;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| multipart_error(&e, "the upload", BACKUP_REQUEST_MAX_BYTES))?
    {
        if field.name() == Some("create_new") {
            let value = field
                .text()
                .await
                .map_err(|e| multipart_error(&e, "create_new", BACKUP_REQUEST_MAX_BYTES))?;
            create_new = value.trim().eq_ignore_ascii_case("true");
        } else if field.name() == Some("company_id") {
            let value = field
                .text()
                .await
                .map_err(|e| multipart_error(&e, "company_id", BACKUP_REQUEST_MAX_BYTES))?;
            requested_company_id = Some(
                Uuid::parse_str(value.trim())
                    .map_err(|_| AppError::BadRequest("company_id must be a valid UUID".into()))?,
            );
        } else if field.name() == Some("file") {
            let file_name = field.file_name().unwrap_or("upload").to_string();
            if !file_name.ends_with(".json") {
                return Err(AppError::BadRequest(
                    "Please upload a .json backup file.".into(),
                ));
            }

            let data = field
                .bytes()
                .await
                .map_err(|e| multipart_error(&e, "the file data", BACKUP_REQUEST_MAX_BYTES))?;

            // Second line of defence: the layer bounds the whole request, this
            // bounds the one part, so a caller cannot spend the envelope's
            // slack on the file itself.
            if data.len() > BACKUP_FILE_MAX_BYTES {
                return Err(payload_too_large("The backup file", BACKUP_FILE_MAX_BYTES));
            }

            // Kept as `Bytes` rather than copied into a `Vec`: the parse below
            // borrows it, and the copy put roughly three times the file in
            // memory at once on a host that does not have it to spare.
            file_data = Some(data);
        }
    }

    let data = file_data
        .ok_or_else(|| AppError::BadRequest("No file uploaded. Include a 'file' field.".into()))?;

    let backup: CompanyBackup = serde_json::from_slice(&data)
        .map_err(|e| AppError::BadRequest(format!("Invalid backup file: {}", e)))?;

    let target_company_id = if auth.has_any_role(&["super_admin"]) {
        if create_new {
            if requested_company_id.is_some() {
                return Err(AppError::BadRequest(
                    "Choose either an existing target company or create a new company, not both."
                        .into(),
                ));
            }
            None
        } else {
            Some(requested_company_id.ok_or_else(|| {
                AppError::BadRequest(
                    "Select an existing target company or explicitly choose to create a new one."
                        .into(),
                )
            })?)
        }
    } else {
        if create_new {
            return Err(AppError::Forbidden(
                "Only super admins can create a company from a backup.".into(),
            ));
        }
        // Unreachable while `ManageBackups` is super_admin-only, but an
        // `expect` here would become a panic the moment that grant widens.
        Some(admin_company_id.ok_or_else(|| {
            AppError::Forbidden("No company assigned for a company-scoped import".into())
        })?)
    };

    let result =
        backup_service::import_company(&state.pool, backup, target_company_id, user_id).await?;

    Ok(Json(result))
}
