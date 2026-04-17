use axum::http::{HeaderMap, header};

const REFRESH_COOKIE_NAME: &str = "refresh_token";
const REFRESH_COOKIE_MAX_AGE: i64 = 30 * 24 * 60 * 60; // 30 days in seconds

/// Build a Set-Cookie header for the refresh token (httpOnly, Secure, SameSite=Strict).
pub fn set_refresh_cookie(token: &str, frontend_url: &str) -> (header::HeaderName, String) {
    let secure = frontend_url.starts_with("https");
    let secure_flag = if secure { "; Secure" } else { "" };

    let value = format!(
        "{}={}; HttpOnly; SameSite=Strict; Path=/api/auth; Max-Age={}{}",
        REFRESH_COOKIE_NAME, token, REFRESH_COOKIE_MAX_AGE, secure_flag,
    );

    (header::SET_COOKIE, value)
}

/// Build a Set-Cookie header that clears the refresh token cookie.
pub fn clear_refresh_cookie(frontend_url: &str) -> (header::HeaderName, String) {
    let secure = frontend_url.starts_with("https");
    let secure_flag = if secure { "; Secure" } else { "" };

    let value = format!(
        "{}=; HttpOnly; SameSite=Strict; Path=/api/auth; Max-Age=0{}",
        REFRESH_COOKIE_NAME, secure_flag,
    );

    (header::SET_COOKIE, value)
}

/// Extract the refresh token from the Cookie header.
pub fn extract_refresh_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|cookie| {
            let cookie = cookie.trim();
            if let Some(value) = cookie.strip_prefix(&format!("{}=", REFRESH_COOKIE_NAME))
                && !value.is_empty()
            {
                return Some(value.to_string());
            }
            None
        })
}
