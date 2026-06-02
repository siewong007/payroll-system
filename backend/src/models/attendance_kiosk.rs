use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct KioskCredential {
    pub id: Uuid,
    pub company_id: Uuid,
    pub label: String,
    pub token_prefix: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub last_used_ip: Option<String>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateKioskCredentialRequest {
    pub label: String,
}

#[derive(Debug, Serialize)]
pub struct CreateKioskCredentialResponse {
    pub credential: KioskCredential,
    pub secret: String,
    pub public_url: String,
}
