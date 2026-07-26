use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::oauth2::{GoogleTokenResponse, GoogleUserInfo, LinkedAccount, OAuth2Account};
use crate::models::user::User;
use crate::repositories::reads::oauth2 as oauth2_reads;
use crate::repositories::{oauth2_accounts, oauth2_states, users};

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
    oauth2_states::delete_expired(pool).await?;
    oauth2_states::insert(pool, state, code_verifier, expires_at).await?;

    Ok(())
}

/// Consume and validate an OAuth2 state, returning the code verifier.
/// The state is deleted after retrieval (single-use).
pub async fn consume_oauth2_state(pool: &PgPool, state: &str) -> AppResult<String> {
    oauth2_states::consume(pool, state).await?.ok_or_else(|| {
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

/// Message shown when the user's authorization code is no longer usable. The
/// state is single-use and already consumed by this point, so restarting the
/// flow is genuinely the only way forward.
pub const STALE_GRANT_MESSAGE: &str =
    "Your Google sign-in took too long or was already used. Please try signing in again.";

/// Message shown when Google itself could not be reached or is failing.
pub const UPSTREAM_UNAVAILABLE_MESSAGE: &str =
    "Google sign-in is temporarily unavailable. Please try again in a moment.";

/// Classify a non-2xx response from Google's token endpoint.
///
/// Google reports failures with the RFC 6749 §5.2 error codes. Only some of them
/// describe a stale sign-in attempt the user can retry; the rest mean *this*
/// deployment's OAuth2 credentials are wrong, which no amount of retrying fixes
/// and which must not be echoed to the caller. Returning a blanket 500 for the
/// whole set told a user with an expired code that the server was broken.
pub fn classify_token_exchange_error(status: u16, body: &str) -> AppError {
    let code = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("error")?.as_str().map(str::to_owned))
        .unwrap_or_default();

    match code.as_str() {
        // The code expired, was already redeemed, or was issued for a different
        // redirect_uri. Routine, and the user's to resolve by starting over.
        "invalid_grant" => AppError::BadRequest(STALE_GRANT_MESSAGE.into()),

        // Our client_id/secret/scope registration is wrong. The user can do
        // nothing about it, so log the detail and return a generic 500.
        "invalid_client"
        | "unauthorized_client"
        | "invalid_scope"
        | "unsupported_grant_type"
        | "invalid_request" => AppError::Internal(format!(
            "Google OAuth2 is misconfigured (error={code}, status={status}): {body}"
        )),

        // No recognisable OAuth2 error code. A 5xx is Google having a bad day;
        // anything else is unexpected and worth surfacing in the logs.
        _ if (500..600).contains(&status) => {
            AppError::BadGateway(UPSTREAM_UNAVAILABLE_MESSAGE.into())
        }
        _ => AppError::Internal(format!(
            "Unexpected Google token exchange failure (status={status}): {body}"
        )),
    }
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
        .map_err(|e| {
            // Never reached Google at all — DNS, TLS or timeout.
            tracing::warn!("Google token exchange request failed: {}", e);
            AppError::BadGateway(UPSTREAM_UNAVAILABLE_MESSAGE.into())
        })?;

    let status = resp.status().as_u16();
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(classify_token_exchange_error(status, &body));
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
        .map_err(|e| {
            tracing::warn!("Google userinfo request failed: {}", e);
            AppError::BadGateway(UPSTREAM_UNAVAILABLE_MESSAGE.into())
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        // The token was minted seconds ago, so a rejection here is Google's
        // problem rather than the user's — but still not a bug in this service.
        return Err(if status.is_server_error() {
            AppError::BadGateway(UPSTREAM_UNAVAILABLE_MESSAGE.into())
        } else {
            AppError::Internal(format!(
                "Google userinfo request rejected (status={status}): {body}"
            ))
        });
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
    oauth2_accounts::find_by_provider_id(pool, provider, provider_user_id).await
}

/// Find a user by their email.
pub async fn find_user_by_email(pool: &PgPool, email: &str) -> AppResult<Option<User>> {
    users::find_active_by_email(pool, email).await
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

    oauth2_accounts::upsert(
        pool,
        user_id,
        provider,
        provider_user_id,
        provider_email,
        provider_name,
        avatar_url,
        access_token_hash.as_deref(),
        refresh_token_hash.as_deref(),
        token_expires_at,
    )
    .await
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

    oauth2_accounts::update_tokens(
        pool,
        &access_hash,
        refresh_hash.as_deref(),
        token_expires_at,
        provider,
        provider_user_id,
    )
    .await
}

/// Unlink an OAuth2 account from a user.
pub async fn unlink_oauth2_account(pool: &PgPool, user_id: Uuid, provider: &str) -> AppResult<()> {
    let rows = oauth2_accounts::delete_for_user(pool, user_id, provider).await?;

    if rows == 0 {
        return Err(AppError::NotFound("OAuth2 account not linked".into()));
    }
    Ok(())
}

/// List linked OAuth2 accounts for a user.
pub async fn list_linked_accounts(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<LinkedAccount>> {
    oauth2_accounts::list_for_user(pool, user_id).await
}

/// Find user by OAuth2 account, fetching full User row.
pub async fn find_user_by_oauth2(
    pool: &PgPool,
    provider: &str,
    provider_user_id: &str,
) -> AppResult<Option<User>> {
    oauth2_reads::find_user_by_oauth2(pool, provider, provider_user_id).await
}
