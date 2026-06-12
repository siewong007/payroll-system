use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::document::{
    CreateDocumentCategoryRequest, CreateDocumentRequest, Document, DocumentCategory,
    UpdateDocumentRequest,
};
use crate::repositories::documents as document_repo;

#[allow(clippy::too_many_arguments)]
pub async fn list_documents(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Option<Uuid>,
    category_id: Option<Uuid>,
    status: Option<&str>,
    search: Option<&str>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<Document>, i64)> {
    let count =
        document_repo::count_filtered(pool, company_id, employee_id, category_id, status, search)
            .await?;

    let documents = document_repo::list_filtered(
        pool,
        company_id,
        employee_id,
        category_id,
        status,
        search,
        limit,
        offset,
    )
    .await?;

    Ok((documents, count))
}

pub async fn get_document(pool: &PgPool, id: Uuid, company_id: Uuid) -> AppResult<Document> {
    document_repo::get(pool, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Document not found".into()))
}

pub async fn create_document(
    pool: &PgPool,
    company_id: Uuid,
    req: CreateDocumentRequest,
    created_by: Uuid,
) -> AppResult<Document> {
    let id = Uuid::now_v7();
    document_repo::insert(pool, id, company_id, &req, created_by).await
}

pub async fn update_document(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
    req: UpdateDocumentRequest,
    updated_by: Uuid,
) -> AppResult<Document> {
    document_repo::update(pool, id, company_id, &req, updated_by)
        .await?
        .ok_or_else(|| AppError::NotFound("Document not found".into()))
}

pub async fn soft_delete_document(pool: &PgPool, id: Uuid, company_id: Uuid) -> AppResult<()> {
    // Fetch the file_url before deleting so we can remove the file from disk
    let file_url = document_repo::file_url(pool, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Document not found".into()))?;

    // Hard delete the record
    document_repo::delete(pool, id, company_id).await?;

    // Remove the file from disk if it exists
    if let Some(filename) = file_url.strip_prefix("/api/uploads/") {
        let file_path = std::path::Path::new("uploads").join(filename);
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    Ok(())
}

pub async fn list_categories(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<DocumentCategory>> {
    document_repo::list_categories(pool, company_id).await
}

pub async fn create_category(
    pool: &PgPool,
    company_id: Uuid,
    req: CreateDocumentCategoryRequest,
) -> AppResult<DocumentCategory> {
    document_repo::insert_category(pool, company_id, &req)
        .await
        .map_err(|e| {
            if let AppError::Database(sqlx::Error::Database(ref db_err)) = e
                && db_err.constraint() == Some("document_categories_company_id_name_key")
            {
                return AppError::Conflict("Category with this name already exists".into());
            }
            e
        })
}

pub async fn get_expiring_documents(
    pool: &PgPool,
    company_id: Uuid,
    days_ahead: i32,
) -> AppResult<Vec<Document>> {
    document_repo::expiring(pool, company_id, days_ahead).await
}
