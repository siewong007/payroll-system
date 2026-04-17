use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::oauth2::{GoogleTokenResponse, GoogleUserInfo, LinkedAccount, OAuth2Account};
use crate::models::user::User;

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";

/// PKCE state expiry in minutes.
const STATE_EXPIRY_MINUTES: i64 = 10;

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Generate a cryptographically random PKCE code verifier (43-128 chars, base64url).
pub fn generate_code_verifier() -> String {
    // Use 32 random bytes → 43 base64url chars
    let bytes: [u8; 32] = {
        let u1 = Uuid::new_v4();
        let u2 = Uuid::new_v4();
        let mut buf = [0u8; 32];
        buf[..16].copy_from_slice(u1.as_bytes());
        buf[16..].copy_from_slice(u2.as_bytes());
        buf
    };
    base64url_encode(&bytes)
}

/// Compute S256 code challenge from a code verifier.
pub fn compute_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    base64url_encode(&hash)
}

/// Base64url encode without padding (per RFC 7636).
fn base64url_encode(bytes: &[u8]) -> String {
    let encoded = bytes
        .chunks(3)
        .flat_map(|chunk| {
            let mut buf = [0u8; 4];
            let len = chunk.len();
            let mut val: u32 = 0;
            for (i, &b) in chunk.iter().enumerate() {
                val |= (b as u32) << (16 - 8 * i);
            }
            let chars: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            buf[0] = chars[(val >> 18 & 0x3F) as usize];
            buf[1] = chars[(val >> 12 & 0x3F) as usize];
            buf[2] = if len > 1 {
                chars[(val >> 6 & 0x3F) as usize]
            } else {
                b'='
            };
            buf[3] = if len > 2 {
                chars[(val & 0x3F) as usize]
            } else {
                b'='
            };
            buf
        })
        .collect::<Vec<u8>>();
    String::from_utf8(encoded)
        .unwrap()
        .replace('+', "-")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_string()
}

/// Store OAuth2 state + PKCE code verifier in the database.
pub async fn store_oauth2_state(pool: &PgPool, state: &str, code_verifier: &str) -> AppResult<()> {
    let expires_at = Utc::now() + Duration::minutes(STATE_EXPIRY_MINUTES);

    // Clean up expired states opportunistically
    sqlx::query("DELETE FROM oauth2_states WHERE expires_at < NOW()")
        .execute(pool)
        .await?;

    sqlx::query("INSERT INTO oauth2_states (state, code_verifier, expires_at) VALUES ($1, $2, $3)")
        .bind(state)
        .bind(code_verifier)
        .bind(expires_at)
        .execute(pool)
        .await?;

    Ok(())
}

/// Consume and validate an OAuth2 state, returning the code verifier.
/// The state is deleted after retrieval (single-use).
pub async fn consume_oauth2_state(pool: &PgPool, state: &str) -> AppResult<String> {
    let row: Option<(String,)> = sqlx::query_as(
        "DELETE FROM oauth2_states WHERE state = $1 AND expires_at > NOW() RETURNING code_verifier",
    )
    .bind(state)
    .fetch_optional(pool)
    .await?;

    row.map(|r| r.0).ok_or_else(|| {
        AppError::BadRequest("Invalid or expired OAuth2 state. Please try signing in again.".into())
    })
}

/// Build the Google OAuth2 authorization URL with PKCE.
pub fn google_authorize_url(
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> String {
    format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&access_type=offline&prompt=consent&code_challenge={}&code_challenge_method=S256",
        GOOGLE_AUTH_URL,
        urlencoding::encode(client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode("openid email profile"),
        urlencoding::encode(state),
        urlencoding::encode(code_challenge),
    )
}

/// Exchange an authorization code for tokens with Google (with PKCE code_verifier).
pub async fn google_exchange_code(
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
    code_verifier: &str,
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
            ("code_verifier", code_verifier),
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
        return Err(AppError::Internal(
            "Failed to fetch Google user info".into(),
        ));
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
    let user =
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1 AND is_active = TRUE")
            .bind(email)
            .fetch_optional(pool)
            .await?;
    Ok(user)
}

/// Link an OAuth2 account to an existing user (with optional token storage).
#[allow(clippy::too_many_arguments)]
pub async fn link_oauth2_account(
    pool: &PgPool,
    user_id: Uuid,
    provider: &str,
    provider_user_id: &str,
    provider_email: Option<&str>,
    provider_name: Option<&str>,
    avatar_url: Option<&str>,
    access_token: Option<&str>,
    refresh_token: Option<&str>,
    token_expires_in: Option<i64>,
) -> AppResult<OAuth2Account> {
    let access_token_hash = access_token.map(hash_token);
    let refresh_token_hash = refresh_token.map(hash_token);
    let token_expires_at = token_expires_in.map(|secs| Utc::now() + Duration::seconds(secs));

    let account = sqlx::query_as::<_, OAuth2Account>(
        r#"INSERT INTO oauth2_accounts (user_id, provider, provider_user_id, provider_email, provider_name, avatar_url, access_token_hash, refresh_token_hash, token_expires_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (provider, provider_user_id) DO UPDATE SET
            provider_email = EXCLUDED.provider_email,
            provider_name = EXCLUDED.provider_name,
            avatar_url = EXCLUDED.avatar_url,
            access_token_hash = COALESCE(EXCLUDED.access_token_hash, oauth2_accounts.access_token_hash),
            refresh_token_hash = COALESCE(EXCLUDED.refresh_token_hash, oauth2_accounts.refresh_token_hash),
            token_expires_at = COALESCE(EXCLUDED.token_expires_at, oauth2_accounts.token_expires_at),
            updated_at = NOW()
        RETURNING *"#,
    )
    .bind(user_id)
    .bind(provider)
    .bind(provider_user_id)
    .bind(provider_email)
    .bind(provider_name)
    .bind(avatar_url)
    .bind(access_token_hash)
    .bind(refresh_token_hash)
    .bind(token_expires_at)
    .fetch_one(pool)
    .await?;
    Ok(account)
}

/// Update stored Google tokens for an existing OAuth2 account after login.
pub async fn update_oauth2_tokens(
    pool: &PgPool,
    provider: &str,
    provider_user_id: &str,
    access_token: &str,
    refresh_token: Option<&str>,
    expires_in: Option<i64>,
) -> AppResult<()> {
    let access_hash = hash_token(access_token);
    let refresh_hash = refresh_token.map(hash_token);
    let token_expires_at = expires_in.map(|secs| Utc::now() + Duration::seconds(secs));

    sqlx::query(
        r#"UPDATE oauth2_accounts SET
            access_token_hash = $1,
            refresh_token_hash = COALESCE($2, refresh_token_hash),
            token_expires_at = COALESCE($3, token_expires_at),
            updated_at = NOW()
        WHERE provider = $4 AND provider_user_id = $5"#,
    )
    .bind(&access_hash)
    .bind(&refresh_hash)
    .bind(token_expires_at)
    .bind(provider)
    .bind(provider_user_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Unlink an OAuth2 account from a user.
pub async fn unlink_oauth2_account(pool: &PgPool, user_id: Uuid, provider: &str) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM oauth2_accounts WHERE user_id = $1 AND provider = $2")
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
pub async fn find_user_by_oauth2(
    pool: &PgPool,
    provider: &str,
    provider_user_id: &str,
) -> AppResult<Option<User>> {
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
