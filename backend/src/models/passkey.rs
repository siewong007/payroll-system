use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use webauthn_rs::prelude::{
    CreationChallengeResponse, PublicKeyCredential, RegisterPublicKeyCredential,
    RequestChallengeResponse,
};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PasskeyCredential {
    pub id: Uuid,
    pub user_id: Uuid,
    pub credential_name: String,
    pub credential_json: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct PasskeyInfo {
    pub id: Uuid,
    pub credential_name: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct RenamePasskeyRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct RegistrationBeginResponse {
    pub challenge_id: Uuid,
    pub options: CreationChallengeResponse,
}

#[derive(Deserialize)]
pub struct RegistrationCompleteRequest {
    pub challenge_id: Uuid,
    pub credential: RegisterPublicKeyCredential,
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct AuthBeginRequest {
    pub email: String,
}

#[derive(Serialize)]
pub struct AuthBeginResponse {
    pub challenge_id: Uuid,
    pub options: RequestChallengeResponse,
}

#[derive(Deserialize)]
pub struct AuthCompleteRequest {
    pub challenge_id: Uuid,
    pub credential: PublicKeyCredential,
}

#[derive(Serialize)]
pub struct DiscoverableAuthBeginResponse {
    pub challenge_id: Uuid,
    pub options: RequestChallengeResponse,
}

#[derive(Deserialize)]
pub struct CheckPasskeyRequest {
    pub email: String,
}

pub struct ConsumedChallenge {
    pub user_id: Option<Uuid>,
    pub state_json: serde_json::Value,
}
