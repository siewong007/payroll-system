use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DocumentCategory {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_active: Option<bool>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Document {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub file_name: String,
    pub file_url: String,
    pub file_size: Option<i64>,
    pub mime_type: Option<String>,
    pub status: String,
    pub issue_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub is_confidential: Option<bool>,
    pub tags: Option<String>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    // Joined fields
    pub employee_name: Option<String>,
    pub employee_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDocumentRequest {
    pub employee_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub file_name: String,
    pub file_url: String,
    pub file_size: Option<i64>,
    pub mime_type: Option<String>,
    pub issue_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub is_confidential: Option<bool>,
    pub tags: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDocumentRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub category_id: Option<Uuid>,
    pub status: Option<String>,
    pub issue_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub is_confidential: Option<bool>,
    pub tags: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDocumentCategoryRequest {
    pub name: String,
    pub description: Option<String>,
}
