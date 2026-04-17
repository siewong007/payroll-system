use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::handlers::employee::PaginatedResponse;
use crate::models::document::{
    CreateDocumentCategoryRequest, CreateDocumentRequest, Document, DocumentCategory,
    UpdateDocumentRequest,
};
use crate::services::document_service;

#[derive(Debug, Deserialize)]
pub struct DocumentListQuery {
    pub employee_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ExpiringQuery {
    pub days: Option<i32>,
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<DocumentListQuery>,
) -> AppResult<Json<PaginatedResponse<Document>>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let (documents, total) = document_service::list_documents(
        &state.pool,
        company_id,
        query.employee_id,
        query.category_id,
        query.status.as_deref(),
        query.search.as_deref(),
        per_page,
        offset,
    )
    .await?;

    Ok(Json(PaginatedResponse {
        data: documents,
        total,
        page,
        per_page,
    }))
}

pub async fn get(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Document>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let doc = document_service::get_document(&state.pool, id, company_id).await?;
    Ok(Json(doc))
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateDocumentRequest>,
) -> AppResult<Json<Document>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let doc = document_service::create_document(&state.pool, company_id, req, auth.0.sub).await?;
    Ok(Json(doc))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateDocumentRequest>,
) -> AppResult<Json<Document>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let doc =
        document_service::update_document(&state.pool, id, company_id, req, auth.0.sub).await?;
    Ok(Json(doc))
}

pub async fn delete(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    document_service::soft_delete_document(&state.pool, id, company_id).await?;
    Ok(Json(serde_json::json!({"message": "Document deleted"})))
}

pub async fn list_categories(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<DocumentCategory>>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let categories = document_service::list_categories(&state.pool, company_id).await?;
    Ok(Json(categories))
}

pub async fn create_category(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateDocumentCategoryRequest>,
) -> AppResult<Json<DocumentCategory>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let cat = document_service::create_category(&state.pool, company_id, req).await?;
    Ok(Json(cat))
}

pub async fn expiring(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ExpiringQuery>,
) -> AppResult<Json<Vec<Document>>> {
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let days = query.days.unwrap_or(30);
    let docs = document_service::get_expiring_documents(&state.pool, company_id, days).await?;
    Ok(Json(docs))
}
