//! Rate-limiting key extraction.
//!
//! The decision of *which* address identifies a client — peer vs. forwarded
//! header — lives in `core::client_ip`, shared with the audit trail so the two
//! cannot drift apart. This module only adapts it to `tower_governor`.

use std::fmt::Write;
use std::net::{IpAddr, SocketAddr};

use axum::extract::ConnectInfo;
use axum::http::Request;
use sha2::{Digest, Sha256};
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

/// Keys on the caller's session when there is one, falling back to the client
/// IP.
///
/// IP keying alone is the wrong shape for authenticated endpoints. A whole
/// office behind one NAT shares a bucket, so a colleague's bulk import can
/// throttle everyone else; meanwhile a single authenticated user can spread
/// abuse across as many addresses as they can reach. Expensive authenticated
/// work — payroll runs, bulk imports, uploads, outbound mail — belongs to the
/// session that requested it.
///
/// The key is a hash of the bearer token, not its `sub` claim: extracting a
/// verified user id would mean decoding the JWT here, ahead of the `AuthUser`
/// extractor that exists to do exactly that. Hashing needs no secret and no
/// trust — an attacker can only ever change their own bucket by presenting a
/// different token, and a token they cannot mint fails auth downstream anyway.
/// The hash also keeps the raw credential out of the limiter's key store.
#[derive(Debug, Clone, Copy)]
pub struct SessionOrIpKeyExtractor {
    trust_forwarded: bool,
}

impl SessionOrIpKeyExtractor {
    pub fn new(trust_forwarded: bool) -> Self {
        Self { trust_forwarded }
    }
}

fn bearer_token<T>(req: &Request<T>) -> Option<&str> {
    req.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

impl KeyExtractor for SessionOrIpKeyExtractor {
    type Key = String;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        if let Some(token) = bearer_token(req) {
            let mut hasher = Sha256::new();
            hasher.update(token.as_bytes());
            let digest = hasher.finalize();
            // Half the digest is ample to keep distinct sessions in distinct
            // buckets; this is a partitioning key, not a security boundary.
            let mut key = String::with_capacity(2 + 32);
            key.push_str("s:");
            for byte in digest.iter().take(16) {
                let _ = write!(key, "{byte:02x}");
            }
            return Ok(key);
        }
        client_ip(req.headers(), peer_ip(req), self.trust_forwarded)
            .map(|ip| format!("i:{ip}"))
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

    fn request_with_token(token: Option<&str>, peer: Option<&str>) -> Request<()> {
        let mut builder = Request::builder();
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
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
    fn two_sessions_from_one_address_get_separate_buckets() {
        let extractor = SessionOrIpKeyExtractor::new(false);
        let a = extractor
            .extract(&request_with_token(Some("token-a"), Some("10.0.0.7:1")))
            .unwrap();
        let b = extractor
            .extract(&request_with_token(Some("token-b"), Some("10.0.0.7:1")))
            .unwrap();
        assert_ne!(
            a, b,
            "an office behind one NAT must not share a single bucket"
        );
    }

    #[test]
    fn one_session_from_two_addresses_shares_a_bucket() {
        let extractor = SessionOrIpKeyExtractor::new(false);
        let a = extractor
            .extract(&request_with_token(Some("same"), Some("10.0.0.7:1")))
            .unwrap();
        let b = extractor
            .extract(&request_with_token(Some("same"), Some("192.0.2.9:1")))
            .unwrap();
        assert_eq!(
            a, b,
            "rotating source addresses must not multiply a user's allowance"
        );
    }

    #[test]
    fn the_raw_token_never_appears_in_the_key() {
        let extractor = SessionOrIpKeyExtractor::new(false);
        let key = extractor
            .extract(&request_with_token(
                Some("secret-token"),
                Some("10.0.0.7:1"),
            ))
            .unwrap();
        assert!(!key.contains("secret-token"));
    }

    #[test]
    fn an_anonymous_request_falls_back_to_its_address() {
        let extractor = SessionOrIpKeyExtractor::new(false);
        let key = extractor
            .extract(&request_with_token(None, Some("10.0.0.7:1")))
            .unwrap();
        assert_eq!(key, "i:10.0.0.7");
    }

    #[test]
    fn session_and_address_keys_cannot_collide() {
        // Distinct prefixes, so a crafted address can never land in the bucket
        // of a session (or the reverse).
        let extractor = SessionOrIpKeyExtractor::new(false);
        let session = extractor
            .extract(&request_with_token(Some("t"), Some("10.0.0.7:1")))
            .unwrap();
        let anonymous = extractor
            .extract(&request_with_token(None, Some("10.0.0.7:1")))
            .unwrap();
        assert!(session.starts_with("s:"));
        assert!(anonymous.starts_with("i:"));
    }

    #[test]
    fn an_empty_bearer_value_is_treated_as_anonymous() {
        let extractor = SessionOrIpKeyExtractor::new(false);
        let key = extractor
            .extract(&request_with_token(Some("   "), Some("10.0.0.7:1")))
            .unwrap();
        assert_eq!(key, "i:10.0.0.7");
    }
}
