//! Data access for the `documents` and `document_categories` tables. Single-row
//! reads/writes join `employees` only to denormalize the uploader's name onto the
//! `Document` model.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::document::{
    CreateDocumentCategoryRequest, CreateDocumentRequest, Document, DocumentCategory,
    UpdateDocumentRequest,
};

/// Count of non-deleted documents for a company.
pub async fn count_active(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM documents WHERE company_id = $1 AND deleted_at IS NULL"#,
        company_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}

/// Count of documents matching the library filters (employee/category/status/search).
pub async fn count_filtered(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Option<Uuid>,
    category_id: Option<Uuid>,
    status: Option<&str>,
    search: Option<&str>,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM documents
        WHERE company_id = $1 AND deleted_at IS NULL
        AND ($2::uuid IS NULL OR employee_id = $2)
        AND ($3::uuid IS NULL OR category_id = $3)
        AND ($4::text IS NULL OR status::text = $4)
        AND ($5::text IS NULL OR title ILIKE '%' || $5 || '%' OR file_name ILIKE '%' || $5 || '%')"#,
        company_id,
        employee_id,
        category_id,
        status,
        search,
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}

/// One page of documents matching the library filters, newest first.
#[allow(clippy::too_many_arguments)]
pub async fn list_filtered(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    employee_id: Option<Uuid>,
    category_id: Option<Uuid>,
    status: Option<&str>,
    search: Option<&str>,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<Document>> {
    let documents = sqlx::query_as!(
        Document,
        r#"SELECT
            d.id, d.company_id, d.employee_id, d.category_id, d.title, d.description,
            d.file_name, d.file_url, d.file_size, d.mime_type, d.status::text AS "status!", d.issue_date, d.expiry_date,
            d.is_confidential, d.tags, d.deleted_at, d.created_at, d.updated_at, d.created_by, d.updated_by,
            e.full_name AS "employee_name?", e.employee_number AS "employee_number?"
        FROM documents d
        LEFT JOIN employees e ON d.employee_id = e.id
        WHERE d.company_id = $1 AND d.deleted_at IS NULL
        AND ($2::uuid IS NULL OR d.employee_id = $2)
        AND ($3::uuid IS NULL OR d.category_id = $3)
        AND ($4::text IS NULL OR d.status::text = $4)
        AND ($5::text IS NULL OR d.title ILIKE '%' || $5 || '%' OR d.file_name ILIKE '%' || $5 || '%')
        ORDER BY d.created_at DESC
        LIMIT $6 OFFSET $7"#,
        company_id,
        employee_id,
        category_id,
        status,
        search,
        limit,
        offset,
    )
    .fetch_all(executor)
    .await?;
    Ok(documents)
}

/// A single non-deleted document (with uploader name), or `None`.
pub async fn get(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<Document>> {
    let doc = sqlx::query_as!(
        Document,
        r#"SELECT
            d.id, d.company_id, d.employee_id, d.category_id, d.title, d.description,
            d.file_name, d.file_url, d.file_size, d.mime_type, d.status::text AS "status!", d.issue_date, d.expiry_date,
            d.is_confidential, d.tags, d.deleted_at, d.created_at, d.updated_at, d.created_by, d.updated_by,
            e.full_name AS "employee_name?", e.employee_number AS "employee_number?"
        FROM documents d
        LEFT JOIN employees e ON d.employee_id = e.id
        WHERE d.id = $1 AND d.company_id = $2 AND d.deleted_at IS NULL"#,
        id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(doc)
}

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    req: &CreateDocumentRequest,
    created_by: Uuid,
) -> AppResult<Document> {
    let doc = sqlx::query_as!(
        Document,
        r#"WITH new_doc AS (
            INSERT INTO documents (
                id, company_id, employee_id, category_id, title, description,
                file_name, file_url, file_size, mime_type,
                issue_date, expiry_date, is_confidential, tags, created_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            RETURNING *
        )
        SELECT
            nd.id AS "id!", nd.company_id AS "company_id!", nd.employee_id, nd.category_id, nd.title AS "title!", nd.description,
            nd.file_name AS "file_name!", nd.file_url AS "file_url!", nd.file_size, nd.mime_type, nd.status::text AS "status!", nd.issue_date, nd.expiry_date,
            nd.is_confidential, nd.tags, nd.deleted_at, nd.created_at AS "created_at!", nd.updated_at AS "updated_at!", nd.created_by, nd.updated_by,
            e.full_name AS "employee_name?", e.employee_number AS "employee_number?"
        FROM new_doc nd
        LEFT JOIN employees e ON nd.employee_id = e.id"#,
        id,
        company_id,
        req.employee_id,
        req.category_id,
        req.title,
        req.description,
        req.file_name,
        req.file_url,
        req.file_size,
        req.mime_type,
        req.issue_date,
        req.expiry_date,
        req.is_confidential,
        req.tags,
        created_by,
    )
    .fetch_one(executor)
    .await?;
    Ok(doc)
}

/// Apply a partial update to a non-deleted document; `None` if it does not exist.
pub async fn update(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    req: &UpdateDocumentRequest,
    updated_by: Uuid,
) -> AppResult<Option<Document>> {
    let doc = sqlx::query_as!(
        Document,
        r#"WITH updated AS (
            UPDATE documents SET
                title = COALESCE($3, title),
                description = COALESCE($4, description),
                category_id = COALESCE($5, category_id),
                status = COALESCE($6::text::document_status, status),
                issue_date = COALESCE($7, issue_date),
                expiry_date = COALESCE($8, expiry_date),
                is_confidential = COALESCE($9, is_confidential),
                tags = COALESCE($10, tags),
                updated_by = $11,
                updated_at = NOW()
            WHERE id = $1 AND company_id = $2 AND deleted_at IS NULL
            RETURNING *
        )
        SELECT
            u.id AS "id!", u.company_id AS "company_id!", u.employee_id, u.category_id, u.title AS "title!", u.description,
            u.file_name AS "file_name!", u.file_url AS "file_url!", u.file_size, u.mime_type, u.status::text AS "status!", u.issue_date, u.expiry_date,
            u.is_confidential, u.tags, u.deleted_at, u.created_at AS "created_at!", u.updated_at AS "updated_at!", u.created_by, u.updated_by,
            e.full_name AS "employee_name?", e.employee_number AS "employee_number?"
        FROM updated u
        LEFT JOIN employees e ON u.employee_id = e.id"#,
        id,
        company_id,
        req.title,
        req.description,
        req.category_id,
        req.status,
        req.issue_date,
        req.expiry_date,
        req.is_confidential,
        req.tags,
        updated_by,
    )
    .fetch_optional(executor)
    .await?;
    Ok(doc)
}

/// The stored file_url of a non-deleted document, or `None` (for cleanup on delete).
pub async fn file_url(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<String>> {
    let file_url = sqlx::query_scalar!(
        "SELECT file_url FROM documents WHERE id = $1 AND company_id = $2 AND deleted_at IS NULL",
        id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(file_url)
}

pub async fn delete(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM documents WHERE id = $1 AND company_id = $2",
        id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn list_categories(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
) -> AppResult<Vec<DocumentCategory>> {
    let cats = sqlx::query_as!(
        DocumentCategory,
        "SELECT * FROM document_categories WHERE company_id = $1 AND is_active = TRUE ORDER BY name",
        company_id,
    )
    .fetch_all(executor)
    .await?;
    Ok(cats)
}

/// Insert a category. The unique (company_id, name) violation surfaces as
/// `AppError::Database`; the service maps it to a conflict.
pub async fn insert_category(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    req: &CreateDocumentCategoryRequest,
) -> AppResult<DocumentCategory> {
    let cat = sqlx::query_as!(
        DocumentCategory,
        r#"INSERT INTO document_categories (company_id, name, description)
        VALUES ($1, $2, $3)
        RETURNING *"#,
        company_id,
        req.name,
        req.description,
    )
    .fetch_one(executor)
    .await?;
    Ok(cat)
}

/// Non-deleted, non-archived documents expiring within `days_ahead` days.
pub async fn expiring(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    days_ahead: i32,
) -> AppResult<Vec<Document>> {
    let docs = sqlx::query_as!(
        Document,
        r#"SELECT
            d.id, d.company_id, d.employee_id, d.category_id, d.title, d.description,
            d.file_name, d.file_url, d.file_size, d.mime_type, d.status::text AS "status!", d.issue_date, d.expiry_date,
            d.is_confidential, d.tags, d.deleted_at, d.created_at, d.updated_at, d.created_by, d.updated_by,
            e.full_name AS "employee_name?", e.employee_number AS "employee_number?"
        FROM documents d
        LEFT JOIN employees e ON d.employee_id = e.id
        WHERE d.company_id = $1 AND d.deleted_at IS NULL
        AND d.expiry_date IS NOT NULL
        AND d.expiry_date <= CURRENT_DATE + $2::int * INTERVAL '1 day'
        AND d.status::text != 'archived'
        ORDER BY d.expiry_date ASC"#,
        company_id,
        days_ahead,
    )
    .fetch_all(executor)
    .await?;
    Ok(docs)
}
