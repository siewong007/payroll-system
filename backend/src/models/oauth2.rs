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

#[derive(Debug, Deserialize)]
pub struct OAuth2CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OAuth2AuthorizeRequest {
    /// `link` aims the Google redirect at the account-linking SPA route
    /// instead of the login callback.
    pub flow: Option<String>,
    pub turnstile_token: Option<String>,
}

#[derive(Deserialize, validator::Validate)]
pub struct LinkGoogleRequest {
    #[validate(length(min = 1, message = "code is required"))]
    pub code: String,
    #[validate(length(min = 1, message = "state is required"))]
    pub state: String,
    #[validate(length(min = 1, message = "password is required"))]
    pub password: String,
    #[serde(default)]
    pub totp_code: Option<String>,
}

impl std::fmt::Debug for LinkGoogleRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkGoogleRequest")
            .field("code", &"***")
            .field("state", &self.state)
            .field("password", &"***")
            .field("totp_code", &self.totp_code.as_ref().map(|_| "***"))
            .finish()
    }
}

#[derive(Deserialize, validator::Validate)]
pub struct UnlinkOAuth2Request {
    #[validate(length(min = 1, message = "password is required"))]
    pub password: String,
    #[serde(default)]
    pub totp_code: Option<String>,
}

impl std::fmt::Debug for UnlinkOAuth2Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnlinkOAuth2Request")
            .field("password", &"***")
            .field("totp_code", &self.totp_code.as_ref().map(|_| "***"))
            .finish()
    }
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
