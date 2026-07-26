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
pub struct TemplateQuery {
    pub letter_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EmailLogQuery {
    pub employee_id: Option<Uuid>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
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

/// An `email_logs` row without `body_html` — the shape every API response uses.
///
/// Letter bodies carry salary figures, disciplinary language and, for the
/// welcome letter, the account's initial credential, while the History table
/// renders six columns and none of them is the body. `SELECT *` is what let
/// `body_html` join the response silently when the column was added, so the
/// list query projects to this struct explicitly instead.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailLogSummary {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Option<Uuid>,
    pub template_id: Option<Uuid>,
    pub letter_type: String,
    pub recipient_email: String,
    pub recipient_name: Option<String>,
    pub subject: String,
    pub status: String,
    pub error_message: Option<String>,
    pub sent_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

impl From<EmailLog> for EmailLogSummary {
    fn from(log: EmailLog) -> Self {
        Self {
            id: log.id,
            company_id: log.company_id,
            employee_id: log.employee_id,
            template_id: log.template_id,
            letter_type: log.letter_type,
            recipient_email: log.recipient_email,
            recipient_name: log.recipient_name,
            subject: log.subject,
            status: log.status,
            error_message: log.error_message,
            sent_at: log.sent_at,
            created_at: log.created_at,
            created_by: log.created_by,
        }
    }
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
