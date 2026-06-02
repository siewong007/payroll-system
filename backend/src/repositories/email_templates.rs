//! Data access for the `email_templates` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::email::{CreateEmailTemplateRequest, EmailTemplate, UpdateEmailTemplateRequest};

pub async fn list(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    letter_type: Option<&str>,
) -> AppResult<Vec<EmailTemplate>> {
    let templates = sqlx::query_as!(
        EmailTemplate,
        r#"SELECT id, company_id, name, letter_type, subject, body_html,
            is_active AS "is_active?", created_at, updated_at, created_by, updated_by
        FROM email_templates
        WHERE company_id = $1 AND ($2::text IS NULL OR letter_type = $2)
        ORDER BY letter_type, name"#,
        company_id,
        letter_type,
    )
    .fetch_all(executor)
    .await?;
    Ok(templates)
}

pub async fn get(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<Option<EmailTemplate>> {
    let template = sqlx::query_as!(
        EmailTemplate,
        r#"SELECT id, company_id, name, letter_type, subject, body_html,
            is_active AS "is_active?", created_at, updated_at, created_by, updated_by
        FROM email_templates WHERE id = $1 AND company_id = $2"#,
        id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(template)
}

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    req: &CreateEmailTemplateRequest,
    created_by: Uuid,
) -> AppResult<EmailTemplate> {
    let template = sqlx::query_as!(
        EmailTemplate,
        r#"INSERT INTO email_templates (company_id, name, letter_type, subject, body_html, created_by, updated_by)
        VALUES ($1, $2, $3, $4, $5, $6, $6)
        RETURNING id, company_id, name, letter_type, subject, body_html,
            is_active AS "is_active?", created_at, updated_at, created_by, updated_by"#,
        company_id,
        req.name,
        req.letter_type,
        req.subject,
        req.body_html,
        created_by,
    )
    .fetch_one(executor)
    .await?;
    Ok(template)
}

/// Apply a partial template update; `None` if the template does not exist.
pub async fn update(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
    req: &UpdateEmailTemplateRequest,
    updated_by: Uuid,
) -> AppResult<Option<EmailTemplate>> {
    let template = sqlx::query_as!(
        EmailTemplate,
        r#"UPDATE email_templates SET
            name = COALESCE($3, name),
            subject = COALESCE($4, subject),
            body_html = COALESCE($5, body_html),
            is_active = COALESCE($6, is_active),
            updated_by = $7,
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING id, company_id, name, letter_type, subject, body_html,
            is_active AS "is_active?", created_at, updated_at, created_by, updated_by"#,
        id,
        company_id,
        req.name,
        req.subject,
        req.body_html,
        req.is_active,
        updated_by,
    )
    .fetch_optional(executor)
    .await?;
    Ok(template)
}

/// Delete a template, returning the number of rows removed (0 = not found).
pub async fn delete(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<u64> {
    let result = sqlx::query!(
        "DELETE FROM email_templates WHERE id = $1 AND company_id = $2",
        id,
        company_id,
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}
