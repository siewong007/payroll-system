use axum::{
    Json,
    extract::{Multipart, Query, State},
    http::header,
    response::IntoResponse,
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::backup::{CompanyBackup, ImportResult};
use crate::services::backup_service;

fn require_admin(auth: &AuthUser) -> AppResult<(Option<Uuid>, Uuid)> {
    if auth.has_any_role(&["super_admin"]) {
        return Ok((auth.0.company_id, auth.0.sub));
    }
    if auth.has_any_role(&["admin"]) {
        return Ok((Some(auth.company_id()?), auth.0.sub));
    }
    Err(AppError::Forbidden("Admin access required".into()))
}

#[derive(Deserialize)]
pub struct ExportQuery {
    pub company_id: Option<Uuid>,
}

pub async fn export_company(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ExportQuery>,
) -> Result<impl IntoResponse, AppError> {
    let (user_company_id, _user_id) = require_admin(&auth)?;

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
    let (_, user_id) = require_admin(&auth)?;

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
