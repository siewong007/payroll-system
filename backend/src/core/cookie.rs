use axum::http::{HeaderMap, header};

use crate::core::error::{AppError, AppResult};

const REFRESH_COOKIE_NAME: &str = "refresh_token";
const SECURE_REFRESH_COOKIE_NAME: &str = "__Secure-refresh_token";
const REFRESH_COOKIE_MAX_AGE: i64 = 30 * 24 * 60 * 60; // 30 days in seconds

/// Browser-held secret that binds an OAuth2 `state` to the tab that started the
/// flow. Only its sha256 travels in the authorization URL.
const OAUTH_STATE_COOKIE_NAME: &str = "oauth2_binder";
const SECURE_OAUTH_STATE_COOKIE_NAME: &str = "__Secure-oauth2_binder";
/// Matches `oauth2_service::STATE_EXPIRY_MINUTES` — the cookie and the stored
/// state row must die together, or one outlives the other's protection.
const OAUTH_STATE_MAX_AGE: i64 = 10 * 60;
/// Narrow enough that the binder is never attached to any other request.
const OAUTH_STATE_PATH: &str = "/api/auth/oauth2";

fn is_https_frontend(frontend_url: &str) -> bool {
    frontend_url.to_ascii_lowercase().starts_with("https://")
}

fn secure_flag(frontend_url: &str) -> &'static str {
    if is_https_frontend(frontend_url) {
        "; Secure"
    } else {
        ""
    }
}

fn refresh_cookie_name(frontend_url: &str) -> &'static str {
    if is_https_frontend(frontend_url) {
        SECURE_REFRESH_COOKIE_NAME
    } else {
        REFRESH_COOKIE_NAME
    }
}

fn oauth_state_cookie_name(frontend_url: &str) -> &'static str {
    if is_https_frontend(frontend_url) {
        SECURE_OAUTH_STATE_COOKIE_NAME
    } else {
        OAUTH_STATE_COOKIE_NAME
    }
}

/// Read one cookie by name from a `Cookie` header, treating an empty value as
/// absent (a cleared cookie is still sent by some clients until it expires).
fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|header_str| header_str.split(';'))
        .find_map(|cookie| {
            let cookie = cookie.trim();
            cookie
                .strip_prefix(&prefix)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

/// Build a Set-Cookie header for the refresh token (httpOnly, Secure + `__Secure-` prefix on HTTPS, SameSite=Strict).
pub fn set_refresh_cookie(token: &str, frontend_url: &str) -> (header::HeaderName, String) {
    let value = format!(
        "{}={}; HttpOnly; SameSite=Strict; Path=/api/auth; Max-Age={}{}",
        refresh_cookie_name(frontend_url),
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
        refresh_cookie_name(frontend_url),
        secure_flag(frontend_url),
    );

    (header::SET_COOKIE, value)
}

/// Extract the refresh token from the Cookie header (`__Secure-refresh_token` first, then `refresh_token`).
pub fn extract_refresh_token(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, SECURE_REFRESH_COOKIE_NAME)
        .or_else(|| cookie_value(headers, REFRESH_COOKIE_NAME))
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
        oauth_state_cookie_name(frontend_url),
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
        oauth_state_cookie_name(frontend_url),
        OAUTH_STATE_PATH,
        secure_flag(frontend_url),
    );

    (header::SET_COOKIE, value)
}

/// Extract the OAuth2 state binder from the Cookie header (`__Secure-oauth2_binder` first, then `oauth2_binder`).
pub fn extract_oauth_state_binder(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, SECURE_OAUTH_STATE_COOKIE_NAME)
        .or_else(|| cookie_value(headers, OAUTH_STATE_COOKIE_NAME))
}

fn origin_tuple(raw: &str) -> Option<(String, String, Option<u16>)> {
    let url = url::Url::parse(raw.trim()).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    Some((
        url.scheme().to_ascii_lowercase(),
        host,
        url.port_or_known_default(),
    ))
}

/// Defense-in-depth CSRF origin validation for cookie-authenticated endpoints
/// (`/auth/refresh`, `/auth/logout`): blocks `Sec-Fetch-Site: cross-site` and
/// rejects any mismatched `Origin` or `Referer` against `frontend_url`.
pub fn validate_cookie_csrf_origin(headers: &HeaderMap, frontend_url: &str) -> AppResult<()> {
    if let Some(sec_fetch_site) = headers
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        && sec_fetch_site.eq_ignore_ascii_case("cross-site")
    {
        return Err(AppError::Forbidden("Cross-site request rejected".into()));
    }

    let Some(expected) = origin_tuple(frontend_url) else {
        return Ok(());
    };

    if let Some(origin_raw) = headers.get(header::ORIGIN) {
        let origin_str = origin_raw
            .to_str()
            .map_err(|_| AppError::Forbidden("Invalid Origin header".into()))?;
        let Some(actual) = origin_tuple(origin_str) else {
            return Err(AppError::Forbidden("Invalid Origin header".into()));
        };
        if actual != expected {
            return Err(AppError::Forbidden("Untrusted request origin".into()));
        }
        return Ok(());
    }

    if let Some(referer_raw) = headers.get(header::REFERER) {
        let referer_str = referer_raw
            .to_str()
            .map_err(|_| AppError::Forbidden("Invalid Referer header".into()))?;
        let Some(actual) = origin_tuple(referer_str) else {
            return Err(AppError::Forbidden("Invalid Referer header".into()));
        };
        if actual != expected {
            return Err(AppError::Forbidden("Untrusted request referer".into()));
        }
    }

    Ok(())
}
