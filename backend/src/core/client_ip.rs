//! Resolving the client IP of a request.
//!
//! Two deployments, two correct answers:
//!
//! - **Behind a trusted proxy** (CloudFront/ALB): the TCP peer is the proxy, so
//!   peer-IP keying collapses every client into one bucket — a fleet of kiosks
//!   rate-limits itself, and one attacker shares a bucket with real users.
//!   The client IP must come from `X-Forwarded-For` / `X-Real-IP`.
//!
//! - **Reached directly** (containers on a host, local dev): forwarded headers
//!   are attacker-controlled. Trusting them lets anyone bypass the login limiter
//!   by rotating a fake `X-Forwarded-For`, which is worse than a shared bucket.
//!
//! So this is not a guess we can make at runtime — the operator declares it via
//! `TRUST_PROXY_HEADERS`, and the default is the safe one (peer IP).
//!
//! This lives in one place because it has two callers that must not disagree:
//! the rate limiter (`core::rate_limit_key`) and the audit trail
//! (`AuditRequestMeta`). They previously had separate implementations, and the
//! audit one read the *left-most* `X-Forwarded-For` entry unconditionally —
//! i.e. the one value the client fully controls — so every recorded audit IP
//! was forgeable even while the limiter was correctly keying on the peer.

use std::net::IpAddr;

use axum::http::HeaderMap;

/// Right-most entry of `X-Forwarded-For`, i.e. the address appended by the
/// closest (trusted) proxy. Taking the left-most instead would read whatever
/// the client sent, which is exactly the value an attacker controls.
///
/// Only meaningful when a trusted proxy is the sole path to the API; callers
/// gate this on `trust_forwarded`.
pub fn forwarded_ip(headers: &HeaderMap) -> Option<IpAddr> {
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok())
        && let Some(ip) = xff
            .split(',')
            .rev()
            .filter_map(|part| part.trim().parse::<IpAddr>().ok())
            .next()
    {
        return Some(ip);
    }
    headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<IpAddr>().ok())
}

/// The client IP to attribute a request to.
///
/// Falls back to `peer` whenever forwarded headers are not trusted or not
/// present, so a direct-reachable deployment still records the truthful
/// address rather than nothing.
pub fn client_ip(
    headers: &HeaderMap,
    peer: Option<IpAddr>,
    trust_forwarded: bool,
) -> Option<IpAddr> {
    if trust_forwarded && let Some(ip) = forwarded_ip(headers) {
        return Some(ip);
    }
    peer
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers_with(xff: Option<&str>, real_ip: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(value) = xff {
            headers.insert("x-forwarded-for", value.parse().expect("header value"));
        }
        if let Some(value) = real_ip {
            headers.insert("x-real-ip", value.parse().expect("header value"));
        }
        headers
    }

    fn ip(value: &str) -> IpAddr {
        value.parse().expect("valid ip")
    }

    #[test]
    fn untrusted_mode_ignores_forwarded_headers() {
        let headers = headers_with(Some("1.2.3.4"), None);
        assert_eq!(
            client_ip(&headers, Some(ip("10.0.0.7")), false),
            Some(ip("10.0.0.7")),
            "a spoofable header must not override the peer address"
        );
    }

    #[test]
    fn trusted_mode_takes_the_proxy_appended_entry() {
        // The client sent "9.9.9.9"; the proxy appended the real peer.
        let headers = headers_with(Some("9.9.9.9, 203.0.113.10"), None);
        assert_eq!(
            client_ip(&headers, Some(ip("10.0.0.7")), true),
            Some(ip("203.0.113.10")),
            "must read the right-most entry, not the client-controlled left-most"
        );
    }

    #[test]
    fn trusted_mode_falls_back_to_peer_when_header_absent() {
        let headers = headers_with(None, None);
        assert_eq!(
            client_ip(&headers, Some(ip("10.0.0.7")), true),
            Some(ip("10.0.0.7"))
        );
    }

    #[test]
    fn x_real_ip_is_used_when_forwarded_for_is_absent() {
        let headers = headers_with(None, Some("198.51.100.4"));
        assert_eq!(
            client_ip(&headers, Some(ip("10.0.0.7")), true),
            Some(ip("198.51.100.4"))
        );
    }

    #[test]
    fn garbage_forwarded_entries_are_skipped_not_trusted() {
        let headers = headers_with(Some("not-an-ip, 203.0.113.10, also-garbage"), None);
        assert_eq!(
            client_ip(&headers, Some(ip("10.0.0.7")), true),
            Some(ip("203.0.113.10")),
            "unparseable entries must be skipped, right-to-left"
        );
    }

    #[test]
    fn nothing_to_go_on_yields_none() {
        let headers = headers_with(None, None);
        assert_eq!(client_ip(&headers, None, true), None);
    }
}
