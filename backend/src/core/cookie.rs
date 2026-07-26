use axum::http::{HeaderMap, header};

const REFRESH_COOKIE_NAME: &str = "refresh_token";
const REFRESH_COOKIE_MAX_AGE: i64 = 30 * 24 * 60 * 60; // 30 days in seconds

/// Browser-held secret that binds an OAuth2 `state` to the tab that started the
/// flow. Only its sha256 travels in the authorization URL.
const OAUTH_STATE_COOKIE_NAME: &str = "oauth2_binder";
/// Matches `oauth2_service::STATE_EXPIRY_MINUTES` — the cookie and the stored
/// state row must die together, or one outlives the other's protection.
const OAUTH_STATE_MAX_AGE: i64 = 10 * 60;
/// Narrow enough that the binder is never attached to any other request.
const OAUTH_STATE_PATH: &str = "/api/auth/oauth2";

fn secure_flag(frontend_url: &str) -> &'static str {
    if frontend_url.starts_with("https") {
        "; Secure"
    } else {
        ""
    }
}

/// Read one cookie by name from a `Cookie` header, treating an empty value as
/// absent (a cleared cookie is still sent by some clients until it expires).
fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|cookie| {
            let cookie = cookie.trim();
            if let Some(value) = cookie.strip_prefix(&format!("{}=", name))
                && !value.is_empty()
            {
                return Some(value.to_string());
            }
            None
        })
}

/// Build a Set-Cookie header for the refresh token (httpOnly, Secure, SameSite=Strict).
pub fn set_refresh_cookie(token: &str, frontend_url: &str) -> (header::HeaderName, String) {
    let value = format!(
        "{}={}; HttpOnly; SameSite=Strict; Path=/api/auth; Max-Age={}{}",
        REFRESH_COOKIE_NAME,
        token,
        REFRESH_COOKIE_MAX_AGE,
        secure_flag(frontend_url),
    );

    (header::SET_COOKIE, value)
}

/// Build a Set-Cookie header that clears the refresh token cookie.
pub fn clear_refresh_cookie(frontend_url: &str) -> (header::HeaderName, String) {
    let value = format!(
        "{}=; HttpOnly; SameSite=Strict; Path=/api/auth; Max-Age=0{}",
        REFRESH_COOKIE_NAME,
        secure_flag(frontend_url),
    );

    (header::SET_COOKIE, value)
}

/// Extract the refresh token from the Cookie header.
pub fn extract_refresh_token(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, REFRESH_COOKIE_NAME)
}

/// Build a Set-Cookie header carrying the OAuth2 state binder.
///
/// `SameSite=Lax`, not `Strict` as the refresh cookie uses: the Google callback
/// arrives as a top-level cross-site GET navigation from accounts.google.com,
/// and a Strict cookie is not sent with it — the binding would fail for every
/// legitimate sign-in.
pub fn set_oauth_state_cookie(binder: &str, frontend_url: &str) -> (header::HeaderName, String) {
    let value = format!(
        "{}={}; HttpOnly; SameSite=Lax; Path={}; Max-Age={}{}",
        OAUTH_STATE_COOKIE_NAME,
        binder,
        OAUTH_STATE_PATH,
        OAUTH_STATE_MAX_AGE,
        secure_flag(frontend_url),
    );

    (header::SET_COOKIE, value)
}

/// Build a Set-Cookie header that burns the OAuth2 state binder. Every exit from
/// the callback clears it, including the failure paths — otherwise a failed
/// attempt leaves a live binder for the remainder of its TTL.
pub fn clear_oauth_state_cookie(frontend_url: &str) -> (header::HeaderName, String) {
    let value = format!(
        "{}=; HttpOnly; SameSite=Lax; Path={}; Max-Age=0{}",
        OAUTH_STATE_COOKIE_NAME,
        OAUTH_STATE_PATH,
        secure_flag(frontend_url),
    );

    (header::SET_COOKIE, value)
}

/// Extract the OAuth2 state binder from the Cookie header.
pub fn extract_oauth_state_binder(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, OAUTH_STATE_COOKIE_NAME)
}
