use aws_lc_rs::hmac;
use axum::http::{HeaderMap, header};
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};

const REFRESH_COOKIE_NAME: &str = "refresh_token";
const SECURE_REFRESH_COOKIE_NAME: &str = "__Secure-refresh_token";
const REFRESH_COOKIE_MAX_AGE: i64 = 30 * 24 * 60 * 60; // 30 days in seconds

const ATT_DEVICE_COOKIE_NAME: &str = "att_device";
const SECURE_ATT_DEVICE_COOKIE_NAME: &str = "__Secure-att_device";
const ATT_DEVICE_COOKIE_MAX_AGE: i64 = 365 * 24 * 60 * 60; // 1 year
const ATT_DEVICE_COOKIE_PATH: &str = "/api/attendance";
const ATT_DEVICE_HMAC_DOMAIN: &[u8] = b"attendance-device-cookie:";

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

/// Build Set-Cookie header(s) clearing the active refresh cookie and, on HTTPS
/// deployments, also flushing any legacy/tossed unprefixed cookie of the same name.
pub fn clear_refresh_cookie_headers(frontend_url: &str) -> Vec<(header::HeaderName, String)> {
    let mut out = vec![clear_refresh_cookie(frontend_url)];
    if is_https_frontend(frontend_url) {
        out.push((
            header::SET_COOKIE,
            format!("{REFRESH_COOKIE_NAME}=; HttpOnly; SameSite=Strict; Path=/api/auth; Max-Age=0"),
        ));
    }
    out
}

/// Extract the refresh token from the Cookie header. When served over HTTPS, only
/// `__Secure-refresh_token` is trusted — never falling back to an unprefixed
/// `refresh_token` that an HTTP MitM could have tossed.
pub fn extract_refresh_token(headers: &HeaderMap, frontend_url: &str) -> Option<String> {
    if is_https_frontend(frontend_url) {
        cookie_value(headers, SECURE_REFRESH_COOKIE_NAME)
    } else {
        cookie_value(headers, SECURE_REFRESH_COOKIE_NAME)
            .or_else(|| cookie_value(headers, REFRESH_COOKIE_NAME))
    }
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

/// Build Set-Cookie header(s) clearing the OAuth2 state binder cookie and, on HTTPS
/// deployments, also flushing any unprefixed `oauth2_binder` cookie.
pub fn clear_oauth_state_cookie_headers(frontend_url: &str) -> Vec<(header::HeaderName, String)> {
    let mut out = vec![clear_oauth_state_cookie(frontend_url)];
    if is_https_frontend(frontend_url) {
        out.push((
            header::SET_COOKIE,
            format!(
                "{OAUTH_STATE_COOKIE_NAME}=; HttpOnly; SameSite=Lax; Path={OAUTH_STATE_PATH}; Max-Age=0"
            ),
        ));
    }
    out
}

/// Extract the OAuth2 state binder from the Cookie header. Over HTTPS, only
/// `__Secure-oauth2_binder` is trusted.
pub fn extract_oauth_state_binder(headers: &HeaderMap, frontend_url: &str) -> Option<String> {
    if is_https_frontend(frontend_url) {
        cookie_value(headers, SECURE_OAUTH_STATE_COOKIE_NAME)
    } else {
        cookie_value(headers, SECURE_OAUTH_STATE_COOKIE_NAME)
            .or_else(|| cookie_value(headers, OAUTH_STATE_COOKIE_NAME))
    }
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
/// (`/auth/refresh`, `/auth/logout`): blocks `Sec-Fetch-Site: cross-site`,
/// rejects non-JSON simple POSTs, rejects mismatched `Origin` or `Referer`, and
/// fails closed on HTTPS deployments when origin verification headers are absent.
pub fn validate_cookie_csrf_origin(headers: &HeaderMap, frontend_url: &str) -> AppResult<()> {
    let sec_fetch_site = headers
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
        .map(str::trim);
    if let Some(sfs) = sec_fetch_site
        && sfs.eq_ignore_ascii_case("cross-site")
    {
        return Err(AppError::Forbidden("Cross-site request rejected".into()));
    }

    if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
        let ct = content_type
            .to_str()
            .map_err(|_| AppError::Forbidden("Invalid Content-Type header".into()))?;
        let media = ct.split(';').next().unwrap_or("").trim();
        if !media.eq_ignore_ascii_case("application/json") {
            return Err(AppError::Forbidden(
                "Requests to this endpoint must use application/json".into(),
            ));
        }
    }

    let expected = origin_tuple(frontend_url)
        .ok_or_else(|| AppError::Internal("Invalid FRONTEND_URL configuration".into()))?;

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
        return Ok(());
    }

    if is_https_frontend(frontend_url)
        && !sec_fetch_site.is_some_and(|s| s.eq_ignore_ascii_case("same-origin"))
    {
        return Err(AppError::Forbidden(
            "Origin verification required for HTTPS requests".into(),
        ));
    }

    Ok(())
}

fn att_device_cookie_name(frontend_url: &str) -> &'static str {
    if is_https_frontend(frontend_url) {
        SECURE_ATT_DEVICE_COOKIE_NAME
    } else {
        ATT_DEVICE_COOKIE_NAME
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut out, "{:02x}", b);
    }
    out
}

fn hex_decode(raw: &str) -> Option<Vec<u8>> {
    if !raw.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(raw.len() / 2);
    let bytes = raw.as_bytes();
    for chunk in bytes.as_chunks::<2>().0 {
        let hi = (chunk[0] as char).to_digit(16)?;
        let lo = (chunk[1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
    }
    Some(out)
}

fn att_device_hmac_payload(device_id: Uuid) -> Vec<u8> {
    let mut msg = Vec::with_capacity(ATT_DEVICE_HMAC_DOMAIN.len() + 16);
    msg.extend_from_slice(ATT_DEVICE_HMAC_DOMAIN);
    msg.extend_from_slice(device_id.as_bytes());
    msg
}

fn sign_att_device_id(device_id: Uuid, jwt_secret: &str) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, jwt_secret.as_bytes());
    let payload = att_device_hmac_payload(device_id);
    let tag = hmac::sign(&key, &payload);
    format!("{device_id}.{}", hex_encode(tag.as_ref()))
}

fn verify_att_device_cookie(raw: &str, jwt_secret: &str) -> Option<Uuid> {
    let (id_str, sig_hex) = raw.split_once('.')?;
    let device_id = Uuid::parse_str(id_str).ok()?;
    let sig_bytes = hex_decode(sig_hex)?;
    let key = hmac::Key::new(hmac::HMAC_SHA256, jwt_secret.as_bytes());
    let payload = att_device_hmac_payload(device_id);
    hmac::verify(&key, &payload, &sig_bytes).ok()?;
    Some(device_id)
}

/// Extract a persistent signed browser device identifier cookie (`att_device` /
/// `__Secure-att_device`), or mint a new one if missing or invalid.
///
/// Intentionally NOT cleared on `/auth/logout` so repeated logins across
/// different accounts in the same browser remain linked for shared-device
/// buddy-punching detection.
pub fn extract_or_mint_attendance_device_cookie(
    headers: &HeaderMap,
    jwt_secret: &str,
    frontend_url: &str,
) -> (Uuid, Option<(header::HeaderName, String)>) {
    let existing = if is_https_frontend(frontend_url) {
        cookie_value(headers, SECURE_ATT_DEVICE_COOKIE_NAME)
    } else {
        cookie_value(headers, SECURE_ATT_DEVICE_COOKIE_NAME)
            .or_else(|| cookie_value(headers, ATT_DEVICE_COOKIE_NAME))
    };

    if let Some(raw) = existing
        && let Some(device_id) = verify_att_device_cookie(&raw, jwt_secret)
    {
        return (device_id, None);
    }

    let device_id = Uuid::new_v4();
    let signed = sign_att_device_id(device_id, jwt_secret);
    let header_value = format!(
        "{}={}; HttpOnly; SameSite=Lax; Path={}; Max-Age={}{}",
        att_device_cookie_name(frontend_url),
        signed,
        ATT_DEVICE_COOKIE_PATH,
        ATT_DEVICE_COOKIE_MAX_AGE,
        secure_flag(frontend_url),
    );
    (device_id, Some((header::SET_COOKIE, header_value)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attendance_device_cookie_round_trips_and_rejects_tampering() {
        let secret = "test-secret-key-for-attendance-cookie";
        let headers = HeaderMap::new();
        let (minted_id, set_cookie) =
            extract_or_mint_attendance_device_cookie(&headers, secret, "http://localhost:5173");
        let (hdr_name, hdr_val) = set_cookie.expect("new cookie should be minted");
        assert_eq!(hdr_name, header::SET_COOKIE);
        assert!(hdr_val.starts_with("att_device="));
        assert!(hdr_val.contains("HttpOnly"));
        assert!(hdr_val.contains("Path=/api/attendance"));
        assert!(!hdr_val.contains("; Secure"));

        let raw_pair = hdr_val.split(';').next().unwrap();
        let mut req_headers = HeaderMap::new();
        req_headers.insert(header::COOKIE, raw_pair.parse().unwrap());

        let (verified_id, second_set) =
            extract_or_mint_attendance_device_cookie(&req_headers, secret, "http://localhost:5173");
        assert_eq!(verified_id, minted_id);
        assert!(second_set.is_none());

        // Tampering with either UUID or signature must mint a new ID
        let tampered = format!("{}.{}", Uuid::new_v4(), "00".repeat(32));
        let mut bad_headers = HeaderMap::new();
        bad_headers.insert(
            header::COOKIE,
            format!("att_device={tampered}").parse().unwrap(),
        );
        let (re_minted_id, re_set) =
            extract_or_mint_attendance_device_cookie(&bad_headers, secret, "http://localhost:5173");
        assert_ne!(re_minted_id, minted_id);
        assert!(re_set.is_some());
    }

    #[test]
    fn attendance_device_cookie_uses_secure_prefix_on_https() {
        let secret = "test-secret-key-for-attendance-cookie";
        let headers = HeaderMap::new();
        let (minted_id, set_cookie) =
            extract_or_mint_attendance_device_cookie(&headers, secret, "https://app.example.com");
        let (_, hdr_val) = set_cookie.expect("secure cookie should be minted");
        assert!(hdr_val.starts_with("__Secure-att_device="));
        assert!(hdr_val.contains("; Secure"));

        // Unprefixed att_device must be ignored on HTTPS
        let unprefixed = format!("att_device={}", sign_att_device_id(minted_id, secret));
        let mut unprefixed_headers = HeaderMap::new();
        unprefixed_headers.insert(header::COOKIE, unprefixed.parse().unwrap());
        let (new_id, re_set) = extract_or_mint_attendance_device_cookie(
            &unprefixed_headers,
            secret,
            "https://app.example.com",
        );
        assert_ne!(new_id, minted_id);
        assert!(re_set.is_some());
    }
}
