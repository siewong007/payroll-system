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

#[derive(Deserialize)]
pub struct CreateKioskCredentialRequest {
    pub label: String,
    #[serde(default)]
    pub actor_password: Option<String>,
    #[serde(default)]
    pub actor_totp_code: Option<String>,
}

impl std::fmt::Debug for CreateKioskCredentialRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateKioskCredentialRequest")
            .field("label", &self.label)
            .field(
                "actor_password",
                &self.actor_password.as_ref().map(|_| "***"),
            )
            .field(
                "actor_totp_code",
                &self.actor_totp_code.as_ref().map(|_| "***"),
            )
            .finish()
    }
}

#[derive(Debug, Serialize)]
pub struct CreateKioskCredentialResponse {
    pub credential: KioskCredential,
    pub secret: String,
    pub public_url: String,
}
