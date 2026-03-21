use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct OAuth2Account {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub provider_user_id: String,
    pub provider_email: Option<String>,
    pub provider_name: Option<String>,
    pub avatar_url: Option<String>,
    pub access_token_hash: Option<String>,
    pub refresh_token_hash: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Google's userinfo response
#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
    pub sub: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub picture: Option<String>,
}

/// Google's token exchange response
#[derive(Debug, Deserialize)]
pub struct GoogleTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
}

/// Request body for OAuth2 callback
#[derive(Debug, Deserialize)]
pub struct OAuth2CallbackRequest {
    pub code: String,
    pub state: Option<String>,
}

/// Response for OAuth2 providers listing
#[derive(Debug, Serialize)]
pub struct OAuth2ProviderInfo {
    pub provider: String,
    pub enabled: bool,
    pub authorize_url: Option<String>,
}

/// Response showing linked OAuth2 accounts for a user
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LinkedAccount {
    pub id: Uuid,
    pub provider: String,
    pub provider_email: Option<String>,
    pub provider_name: Option<String>,
    pub avatar_url: Option<String>,
    pub linked_at: DateTime<Utc>,
}
