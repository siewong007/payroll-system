//! Rate-limiting key extraction.
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

use std::net::{IpAddr, SocketAddr};

use axum::extract::ConnectInfo;
use axum::http::Request;
use tower_governor::{errors::GovernorError, key_extractor::KeyExtractor};

/// Extracts the client IP, optionally trusting proxy headers.
#[derive(Debug, Clone, Copy)]
pub struct ClientIpKeyExtractor {
    /// Whether `X-Forwarded-For` / `X-Real-IP` may be believed. Only set this
    /// when a trusted proxy is the sole path to the API and it overwrites (or
    /// appends to) those headers itself.
    trust_forwarded: bool,
}

impl ClientIpKeyExtractor {
    pub fn new(trust_forwarded: bool) -> Self {
        Self { trust_forwarded }
    }
}

fn peer_ip<T>(req: &Request<T>) -> Option<IpAddr> {
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip())
}

/// Right-most entry of `X-Forwarded-For`, i.e. the address appended by the
/// closest (trusted) proxy. Taking the left-most instead would read whatever
/// the client sent, which is exactly the value an attacker controls.
fn forwarded_ip<T>(req: &Request<T>) -> Option<IpAddr> {
    let headers = req.headers();
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

impl KeyExtractor for ClientIpKeyExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        if self.trust_forwarded
            && let Some(ip) = forwarded_ip(req)
        {
            return Ok(ip);
        }
        peer_ip(req).ok_or(GovernorError::UnableToExtractKey)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_with(xff: Option<&str>, peer: Option<&str>) -> Request<()> {
        let mut builder = Request::builder();
        if let Some(value) = xff {
            builder = builder.header("x-forwarded-for", value);
        }
        let mut req = builder.body(()).expect("build request");
        if let Some(addr) = peer {
            req.extensions_mut().insert(ConnectInfo(
                addr.parse::<SocketAddr>().expect("valid socket addr"),
            ));
        }
        req
    }

    #[test]
    fn untrusted_mode_ignores_forwarded_headers() {
        let extractor = ClientIpKeyExtractor::new(false);
        let req = request_with(Some("1.2.3.4"), Some("10.0.0.7:1234"));
        assert_eq!(
            extractor.extract(&req).unwrap(),
            "10.0.0.7".parse::<IpAddr>().unwrap(),
            "a spoofable header must not override the peer address"
        );
    }

    #[test]
    fn trusted_mode_takes_the_proxy_appended_entry() {
        let extractor = ClientIpKeyExtractor::new(true);
        // The client sent "9.9.9.9"; the proxy appended the real peer.
        let req = request_with(Some("9.9.9.9, 203.0.113.10"), Some("10.0.0.7:1234"));
        assert_eq!(
            extractor.extract(&req).unwrap(),
            "203.0.113.10".parse::<IpAddr>().unwrap(),
            "must read the right-most entry, not the client-controlled left-most"
        );
    }

    #[test]
    fn trusted_mode_falls_back_to_peer_when_header_absent() {
        let extractor = ClientIpKeyExtractor::new(true);
        let req = request_with(None, Some("10.0.0.7:1234"));
        assert_eq!(
            extractor.extract(&req).unwrap(),
            "10.0.0.7".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn missing_everything_is_an_error_not_a_shared_bucket() {
        let extractor = ClientIpKeyExtractor::new(true);
        let req = request_with(None, None);
        assert!(extractor.extract(&req).is_err());
    }
}
