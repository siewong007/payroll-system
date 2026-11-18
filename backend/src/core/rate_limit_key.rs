//! Rate-limiting key extraction.
//!
//! The decision of *which* address identifies a client — peer vs. forwarded
//! header — lives in `core::client_ip`, shared with the audit trail so the two
//! cannot drift apart. This module only adapts it to `tower_governor`.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::ConnectInfo;
use axum::http::Request;
use tower_governor::{errors::GovernorError, key_extractor::KeyExtractor};

use super::auth::verify_token;
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

/// Keys on the verified session ID (`claims.sid`) when a valid Bearer JWT is
/// presented, falling back to the client IP otherwise.
///
/// IP keying alone is the wrong shape for authenticated endpoints. A whole
/// office behind one NAT shares a bucket, so a colleague's bulk import can
/// throttle everyone else; meanwhile a single authenticated user can spread
/// abuse across as many addresses as they can reach. Expensive authenticated
/// work — payroll runs, bulk imports, uploads, outbound mail — belongs to the
/// session that requested it.
///
/// Verifying the JWT signature against `jwt_secret` guarantees an unauthenticated
/// attacker cannot escape their IP bucket by sending arbitrary/forged `Bearer`
/// strings.
#[derive(Debug, Clone)]
pub struct SessionOrIpKeyExtractor {
    trust_forwarded: bool,
    jwt_secret: Arc<String>,
}

impl SessionOrIpKeyExtractor {
    pub fn new(trust_forwarded: bool, jwt_secret: impl Into<Arc<String>>) -> Self {
        Self {
            trust_forwarded,
            jwt_secret: jwt_secret.into(),
        }
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
        if let Some(token) = bearer_token(req)
            && let Ok(claims) = verify_token(token, &self.jwt_secret)
        {
            return Ok(format!("s:{}", claims.sid));
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

    fn mint_jwt(secret: &str, sid: uuid::Uuid) -> String {
        crate::core::auth::create_token(
            uuid::Uuid::new_v4(),
            "test@example.invalid",
            &["admin".to_string()],
            None,
            None,
            sid,
            secret,
            1,
        )
        .expect("mint test jwt")
    }

    #[test]
    fn two_sessions_from_one_address_get_separate_buckets() {
        let secret = Arc::new("test-jwt-secret-at-least-32-bytes-long".to_string());
        let extractor = SessionOrIpKeyExtractor::new(false, secret.clone());
        let tok_a = mint_jwt(&secret, uuid::Uuid::new_v4());
        let tok_b = mint_jwt(&secret, uuid::Uuid::new_v4());
        let a = extractor
            .extract(&request_with_token(Some(&tok_a), Some("10.0.0.7:1")))
            .unwrap();
        let b = extractor
            .extract(&request_with_token(Some(&tok_b), Some("10.0.0.7:1")))
            .unwrap();
        assert_ne!(
            a, b,
            "an office behind one NAT must not share a single bucket"
        );
    }

    #[test]
    fn one_session_from_two_addresses_shares_a_bucket() {
        let secret = Arc::new("test-jwt-secret-at-least-32-bytes-long".to_string());
        let extractor = SessionOrIpKeyExtractor::new(false, secret.clone());
        let tok = mint_jwt(&secret, uuid::Uuid::new_v4());
        let a = extractor
            .extract(&request_with_token(Some(&tok), Some("10.0.0.7:1")))
            .unwrap();
        let b = extractor
            .extract(&request_with_token(Some(&tok), Some("192.0.2.9:1")))
            .unwrap();
        assert_eq!(
            a, b,
            "rotating source addresses must not multiply a user's allowance"
        );
    }

    #[test]
    fn the_raw_token_never_appears_in_the_key() {
        let secret = Arc::new("test-jwt-secret-at-least-32-bytes-long".to_string());
        let extractor = SessionOrIpKeyExtractor::new(false, secret.clone());
        let tok = mint_jwt(&secret, uuid::Uuid::new_v4());
        let key = extractor
            .extract(&request_with_token(Some(&tok), Some("10.0.0.7:1")))
            .unwrap();
        assert!(!key.contains(&tok));
    }

    #[test]
    fn forged_bearer_token_falls_back_to_client_ip() {
        let secret = Arc::new("test-jwt-secret-at-least-32-bytes-long".to_string());
        let extractor = SessionOrIpKeyExtractor::new(false, secret);
        let key = extractor
            .extract(&request_with_token(
                Some("forged-token"),
                Some("10.0.0.7:1"),
            ))
            .unwrap();
        assert_eq!(key, "i:10.0.0.7");
    }

    #[test]
    fn an_anonymous_request_falls_back_to_its_address() {
        let extractor = SessionOrIpKeyExtractor::new(false, Arc::new("secret".to_string()));
        let key = extractor
            .extract(&request_with_token(None, Some("10.0.0.7:1")))
            .unwrap();
        assert_eq!(key, "i:10.0.0.7");
    }

    #[test]
    fn session_and_address_keys_cannot_collide() {
        // Distinct prefixes, so a crafted address can never land in the bucket
        // of a session (or the reverse).
        let secret = Arc::new("test-jwt-secret-at-least-32-bytes-long".to_string());
        let extractor = SessionOrIpKeyExtractor::new(false, secret.clone());
        let tok = mint_jwt(&secret, uuid::Uuid::new_v4());
        let session = extractor
            .extract(&request_with_token(Some(&tok), Some("10.0.0.7:1")))
            .unwrap();
        let anonymous = extractor
            .extract(&request_with_token(None, Some("10.0.0.7:1")))
            .unwrap();
        assert!(session.starts_with("s:"));
        assert!(anonymous.starts_with("i:"));
    }

    #[test]
    fn an_empty_bearer_value_is_treated_as_anonymous() {
        let extractor = SessionOrIpKeyExtractor::new(false, Arc::new("secret".to_string()));
        let key = extractor
            .extract(&request_with_token(Some("   "), Some("10.0.0.7:1")))
            .unwrap();
        assert_eq!(key, "i:10.0.0.7");
    }
}
