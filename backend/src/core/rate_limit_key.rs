//! Rate-limiting key extraction.
//!
//! The decision of *which* address identifies a client — peer vs. forwarded
//! header — lives in `core::client_ip`, shared with the audit trail so the two
//! cannot drift apart. This module only adapts it to `tower_governor`.

use std::net::{IpAddr, SocketAddr};

use axum::extract::ConnectInfo;
use axum::http::Request;
use tower_governor::{errors::GovernorError, key_extractor::KeyExtractor};

use super::client_ip::client_ip;

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

impl KeyExtractor for ClientIpKeyExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        // No usable address means no key. Returning a placeholder would put
        // every such request in one shared bucket, which is the failure mode
        // this whole module exists to avoid.
        client_ip(req.headers(), peer_ip(req), self.trust_forwarded)
            .ok_or(GovernorError::UnableToExtractKey)
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
