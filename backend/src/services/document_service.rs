use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::core::upload_path;
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
    company_wide: Option<bool>,
    category_id: Option<Uuid>,
    status: Option<&str>,
    search: Option<&str>,
    include_confidential: bool,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<Document>, i64)> {
    let count = document_repo::count_filtered(
        pool,
        company_id,
        employee_id,
        company_wide,
        category_id,
        status,
        search,
        include_confidential,
    )
    .await?;

    let documents = document_repo::list_filtered(
        pool,
        company_id,
        employee_id,
        company_wide,
        category_id,
        status,
        search,
        include_confidential,
        limit,
        offset,
    )
    .await?;

    Ok((documents, count))
}

pub async fn get_document(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
    can_view_confidential: bool,
) -> AppResult<Document> {
    let doc = document_repo::get(pool, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Document not found".into()))?;
    if doc.is_confidential == Some(true) && !can_view_confidential {
        return Err(AppError::NotFound("Document not found".into()));
    }
    Ok(doc)
}

pub async fn create_document(
    pool: &PgPool,
    company_id: Uuid,
    req: CreateDocumentRequest,
    created_by: Uuid,
    access: &crate::models::upload::UploadAccess,
) -> AppResult<Document> {
    let is_confidential = req.is_confidential == Some(true);
    if is_confidential && !access.can_view_confidential {
        return Err(AppError::Forbidden(
            "Managing employees or payroll permission required for confidential documents".into(),
        ));
    }
    // `file_url` is free text from the client and ends up joined onto a
    // filesystem path by the delete and backup paths, so the shape is settled
    // here, before it can be persisted. `UpdateDocumentRequest` carries no
    // `file_url`, so this is the only write-time entry point for the column.
    upload_path::validate_file_url(&req.file_url)?;
    crate::services::upload_service::ensure_upload_attach_authorized(
        pool,
        access,
        Some(&req.file_url),
        is_confidential,
    )
    .await?;

    let id = Uuid::now_v7();
    document_repo::insert(pool, id, company_id, &req, created_by).await
}

pub async fn update_document(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
    req: UpdateDocumentRequest,
    updated_by: Uuid,
    can_view_confidential: bool,
) -> AppResult<Document> {
    let existing = document_repo::get(pool, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Document not found".into()))?;
    if existing.is_confidential == Some(true) && !can_view_confidential {
        return Err(AppError::NotFound("Document not found".into()));
    }
    if req.is_confidential == Some(true) && !can_view_confidential {
        return Err(AppError::Forbidden(
            "Managing employees or payroll permission required for confidential documents".into(),
        ));
    }

    document_repo::update(pool, id, company_id, &req, updated_by)
        .await?
        .ok_or_else(|| AppError::NotFound("Document not found".into()))
}

pub async fn soft_delete_document(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
    can_view_confidential: bool,
) -> AppResult<()> {
    let existing = document_repo::get(pool, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Document not found".into()))?;
    if existing.is_confidential == Some(true) && !can_view_confidential {
        return Err(AppError::NotFound("Document not found".into()));
    }
    // Fetch the file_url before deleting so we can remove the file from disk
    let file_url = existing.file_url;

    // Hard delete the record
    document_repo::delete(pool, id, company_id).await?;

    // Only unlink the file from disk when no other claim, leave, document, or
    // company logo still references it.
    match crate::repositories::reads::uploads::find_references(pool, company_id, &file_url).await {
        Ok(remaining_refs) if remaining_refs.is_empty() => {
            match upload_path::local_upload_path(&file_url) {
                Ok(Some(file_path)) => {
                    let _ = tokio::fs::remove_file(&file_path).await;
                    if let Some(stored_name) = file_url.strip_prefix(upload_path::UPLOAD_URL_PREFIX)
                    {
                        let _ = crate::repositories::uploaded_files::delete_by_stored_name(
                            pool,
                            stored_name,
                        )
                        .await;
                    }
                }
                // An external http(s) link — nothing of ours to unlink.
                Ok(None) => {}
                Err(_) => tracing::warn!(
                    %file_url,
                    "refusing to unlink a document whose file_url is not a safe upload path"
                ),
            }
        }
        Ok(_) => {}
        Err(err) => {
            tracing::warn!(
                %file_url,
                error = %err,
                "skipping upload cleanup because reference lookup failed"
            );
        }
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
    include_confidential: bool,
) -> AppResult<Vec<Document>> {
    document_repo::expiring(pool, company_id, days_ahead, include_confidential).await
}
