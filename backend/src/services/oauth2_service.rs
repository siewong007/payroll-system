use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::oauth2::{GoogleTokenResponse, GoogleUserInfo, LinkedAccount, OAuth2Account};
use crate::models::user::User;

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";

/// Build the Google OAuth2 authorization URL.
pub fn google_authorize_url(client_id: &str, redirect_uri: &str, state: &str) -> String {
    format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&access_type=offline&prompt=consent",
        GOOGLE_AUTH_URL,
        urlencoding::encode(client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode("openid email profile"),
        urlencoding::encode(state),
    )
}

/// Exchange an authorization code for tokens with Google.
pub async fn google_exchange_code(
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
) -> AppResult<GoogleTokenResponse> {
    let client = reqwest::Client::new();
    let resp = client
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("code", code),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Google token exchange failed: {}", e)))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!(
            "Google token exchange error: {}",
            body
        )));
    }

    resp.json::<GoogleTokenResponse>()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse Google token response: {}", e)))
}

/// Fetch user info from Google using an access token.
pub async fn google_user_info(access_token: &str) -> AppResult<GoogleUserInfo> {
    let client = reqwest::Client::new();
    let resp = client
        .get(GOOGLE_USERINFO_URL)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Google userinfo request failed: {}", e)))?;

    if !resp.status().is_success() {
        return Err(AppError::Internal("Failed to fetch Google user info".into()));
    }

    resp.json::<GoogleUserInfo>()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse Google user info: {}", e)))
}

/// Find an existing OAuth2 account link.
pub async fn find_oauth2_account(
    pool: &PgPool,
    provider: &str,
    provider_user_id: &str,
) -> AppResult<Option<OAuth2Account>> {
    let account = sqlx::query_as::<_, OAuth2Account>(
        "SELECT * FROM oauth2_accounts WHERE provider = $1 AND provider_user_id = $2",
    )
    .bind(provider)
    .bind(provider_user_id)
    .fetch_optional(pool)
    .await?;
    Ok(account)
}

/// Find a user by their email.
pub async fn find_user_by_email(pool: &PgPool, email: &str) -> AppResult<Option<User>> {
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = $1 AND is_active = TRUE",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

/// Link an OAuth2 account to an existing user.
pub async fn link_oauth2_account(
    pool: &PgPool,
    user_id: Uuid,
    provider: &str,
    provider_user_id: &str,
    provider_email: Option<&str>,
    provider_name: Option<&str>,
    avatar_url: Option<&str>,
) -> AppResult<OAuth2Account> {
    let account = sqlx::query_as::<_, OAuth2Account>(
        r#"INSERT INTO oauth2_accounts (user_id, provider, provider_user_id, provider_email, provider_name, avatar_url)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (provider, provider_user_id) DO UPDATE SET
            provider_email = EXCLUDED.provider_email,
            provider_name = EXCLUDED.provider_name,
            avatar_url = EXCLUDED.avatar_url,
            updated_at = NOW()
        RETURNING *"#,
    )
    .bind(user_id)
    .bind(provider)
    .bind(provider_user_id)
    .bind(provider_email)
    .bind(provider_name)
    .bind(avatar_url)
    .fetch_one(pool)
    .await?;
    Ok(account)
}

/// Unlink an OAuth2 account from a user.
pub async fn unlink_oauth2_account(
    pool: &PgPool,
    user_id: Uuid,
    provider: &str,
) -> AppResult<()> {
    let result = sqlx::query(
        "DELETE FROM oauth2_accounts WHERE user_id = $1 AND provider = $2",
    )
    .bind(user_id)
    .bind(provider)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("OAuth2 account not linked".into()));
    }
    Ok(())
}

/// List linked OAuth2 accounts for a user.
pub async fn list_linked_accounts(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<LinkedAccount>> {
    let accounts = sqlx::query_as::<_, LinkedAccount>(
        r#"SELECT id, provider, provider_email, provider_name, avatar_url, created_at as linked_at
        FROM oauth2_accounts WHERE user_id = $1
        ORDER BY created_at"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(accounts)
}

/// Find user by OAuth2 account, fetching full User row.
pub async fn find_user_by_oauth2(pool: &PgPool, provider: &str, provider_user_id: &str) -> AppResult<Option<User>> {
    let user = sqlx::query_as::<_, User>(
        r#"SELECT u.* FROM users u
        JOIN oauth2_accounts oa ON u.id = oa.user_id
        WHERE oa.provider = $1 AND oa.provider_user_id = $2 AND u.is_active = TRUE"#,
    )
    .bind(provider)
    .bind(provider_user_id)
    .fetch_optional(pool)
    .await?;
    Ok(user)
}
