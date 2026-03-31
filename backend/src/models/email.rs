use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailTemplate {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub letter_type: String,
    pub subject: String,
    pub body_html: String,
    pub is_active: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateEmailTemplateRequest {
    pub name: String,
    pub letter_type: String,
    pub subject: String,
    pub body_html: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEmailTemplateRequest {
    pub name: Option<String>,
    pub subject: Option<String>,
    pub body_html: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailLog {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Option<Uuid>,
    pub template_id: Option<Uuid>,
    pub letter_type: String,
    pub recipient_email: String,
    pub recipient_name: Option<String>,
    pub subject: String,
    pub body_html: String,
    pub status: String,
    pub error_message: Option<String>,
    pub sent_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct SendLetterRequest {
    pub employee_id: Option<Uuid>,
    pub recipient_email: Option<String>,
    pub recipient_name: Option<String>,
    pub letter_type: String,
    pub subject: String,
    pub body_html: String,
    pub template_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct PreviewLetterRequest {
    pub employee_id: Option<Uuid>,
    pub recipient_email: Option<String>,
    pub recipient_name: Option<String>,
    pub subject: String,
    pub body_html: String,
}

#[derive(Debug, Serialize)]
pub struct PreviewLetterResponse {
    pub subject: String,
    pub body_html: String,
    pub recipient_email: String,
    pub recipient_name: String,
}

/// The valid letter types
pub const LETTER_TYPES: &[&str] = &[
    "welcome",
    "offer",
    "appointment",
    "warning",
    "termination",
    "promotion",
    "general",
];

pub fn is_valid_letter_type(t: &str) -> bool {
    LETTER_TYPES.contains(&t)
}
