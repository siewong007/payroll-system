use axum::{
    extract::{Multipart, State},
    http::header,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::backup::{CompanyBackup, ImportResult};
use crate::services::backup_service;

fn require_admin(auth: &AuthUser) -> AppResult<(Uuid, Uuid)> {
    match auth.0.role.as_str() {
        "super_admin" | "admin" => {
            let company_id = auth
                .0
                .company_id
                .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;
            Ok((company_id, auth.0.sub))
        }
        _ => Err(AppError::Forbidden("Admin access required".into())),
    }
}

pub async fn export_company(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    let (company_id, _user_id) = require_admin(&auth)?;

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
    let (_company_id, user_id) = require_admin(&auth)?;

    let mut file_data: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to read upload: {}", e)))?
    {
        if field.name() == Some("file") {
            let file_name = field.file_name().unwrap_or("upload").to_string();
            if !file_name.ends_with(".json") {
                return Err(AppError::BadRequest(
                    "Please upload a .json backup file.".into(),
                ));
            }

            let data = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("Failed to read file data: {}", e)))?;

            if data.len() > 100 * 1024 * 1024 {
                return Err(AppError::BadRequest(
                    "File too large. Maximum size is 100MB.".into(),
                ));
            }

            file_data = Some(data.to_vec());
            break;
        }
    }

    let data = file_data
        .ok_or_else(|| AppError::BadRequest("No file uploaded. Include a 'file' field.".into()))?;

    let backup: CompanyBackup = serde_json::from_slice(&data)
        .map_err(|e| AppError::BadRequest(format!("Invalid backup file: {}", e)))?;

    let result = backup_service::import_company(&state.pool, backup, user_id).await?;

    Ok(Json(result))
}
