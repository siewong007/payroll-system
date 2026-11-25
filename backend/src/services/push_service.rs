//! RFC 8291 (`aes128gcm`) + RFC 8292 (VAPID ES256) Web Push delivery service.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use aws_lc_rs::aead::{AES_128_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use aws_lc_rs::agreement::{self, ECDH_P256, PrivateKey, UnparsedPublicKey};
use aws_lc_rs::hkdf::{self, HKDF_SHA256, KeyType};
use aws_lc_rs::rand::{SecureRandom, SystemRandom};
use aws_lc_rs::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration as ChronoDuration, Utc};
use serde::Serialize;
use sqlx::PgPool;
use url::Url;
use uuid::Uuid;

use crate::core::config::AppConfig;
use crate::core::error::{AppError, AppResult};
use crate::core::http_client::is_blocked_ip;
use crate::models::notification::{PushOutboxBatchRow, PushSubscription, TestPushResponse};
use crate::repositories::{push_outbox, push_subscriptions};

const MAX_ENDPOINT_LEN: usize = 2048;

/// Hosts that browsers actually hand out as Web Push endpoints. A subscription
/// endpoint is a URL the client supplies and the server later POSTs to, so
/// anything outside the real push services is refused outright: that closes the
/// request-forgery class without relying on the address checks alone. Matched
/// as the exact host or any subdomain of it.
///
/// - FCM: Chrome, Edge (Chromium), Opera, Samsung Internet, Brave.
/// - Mozilla autopush: Firefox (`updates.push.services.mozilla.com`).
/// - Apple: Safari / iOS home-screen apps (`web.push.apple.com`).
/// - WNS: legacy Edge on Windows (`*.notify.windows.com`).
const ALLOWED_PUSH_HOSTS: &[&str] = &[
    "fcm.googleapis.com",
    "android.googleapis.com",
    "push.services.mozilla.com",
    "push.apple.com",
    "notify.windows.com",
];

fn is_allowed_push_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    ALLOWED_PUSH_HOSTS.iter().any(|allowed| {
        host == *allowed
            || host
                .strip_suffix(allowed)
                .is_some_and(|prefix| prefix.ends_with('.'))
    })
}

/// DNS resolver for the push client that drops every non-public address before
/// the connector sees it.
///
/// Checking addresses in a separate `lookup_host` and then letting reqwest
/// resolve the name again leaves a rebinding window: the second answer can be
/// internal. Doing the vetting *inside* the resolver means the addresses that
/// were checked are the only ones that can be dialed.
struct PublicOnlyResolver;

impl reqwest::dns::Resolve for PublicOnlyResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            // Port 0: reqwest substitutes the URL's port on the returned addrs.
            let addrs: Vec<SocketAddr> =
                tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            if addrs.is_empty() {
                return Err("push endpoint resolved to no addresses".into());
            }
            if addrs.iter().any(|addr| is_blocked_ip(addr.ip())) {
                return Err("push endpoint resolved to a non-public address".into());
            }
            let addrs: reqwest::dns::Addrs = Box::new(addrs.into_iter());
            Ok(addrs)
        })
    }
}
const MAX_PLAINTEXT_BYTES: usize = 3900;
const RECORD_SIZE: u32 = 4096;
const RETRY_BACKOFF_SECS: [i64; 3] = [30, 120, 600];

#[derive(Clone, Copy)]
struct HkdfLen(usize);

fn internal_err(msg: &str) -> AppError {
    AppError::Internal(msg.to_string())
}

impl KeyType for HkdfLen {
    fn len(&self) -> usize {
        self.0
    }
}

#[derive(Clone)]
pub struct SharedPushClient {
    inner: Option<Arc<PushClientInner>>,
    #[cfg(test)]
    allow_test_http: bool,
}

struct PushClientInner {
    http: reqwest::Client,
    vapid_key: EcdsaKeyPair,
    vapid_public_key_b64: String,
    vapid_subject: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebPushPayload<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub url: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<&'a str>,
    pub icon: &'static str,
    pub badge: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unread_count: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushDeliveryOutcome {
    Sent,
    Gone(String),
    Retryable(String),
    Failed(String),
}

impl SharedPushClient {
    pub fn from_config(config: &AppConfig) -> Self {
        let Some(pub_raw) = config.vapid_public_key.as_deref() else {
            if config.vapid_private_key.is_some() {
                tracing::warn!(
                    "VAPID_PRIVATE_KEY is set without VAPID_PUBLIC_KEY; Web Push disabled"
                );
            }
            return Self::disabled();
        };
        let Some(priv_raw) = config.vapid_private_key.as_deref() else {
            tracing::warn!("VAPID_PUBLIC_KEY is set without VAPID_PRIVATE_KEY; Web Push disabled");
            return Self::disabled();
        };
        let Some(subject) = config.resolved_vapid_subject() else {
            tracing::warn!(
                "VAPID keys are set but neither VAPID_SUBJECT (mailto:/https://) nor an HTTPS FRONTEND_URL is configured; Web Push disabled"
            );
            return Self::disabled();
        };

        match Self::build_inner(pub_raw, priv_raw, subject) {
            Ok(inner) => {
                tracing::info!("Web Push (VAPID) client initialized");
                Self {
                    inner: Some(Arc::new(inner)),
                    #[cfg(test)]
                    allow_test_http: false,
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "Invalid VAPID configuration; Web Push disabled");
                Self::disabled()
            }
        }
    }

    pub fn disabled() -> Self {
        Self {
            inner: None,
            #[cfg(test)]
            allow_test_http: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        pub_b64: &str,
        priv_b64: &str,
        subject: &str,
        allow_test_http: bool,
    ) -> Self {
        let inner = Self::build_inner(pub_b64, priv_b64, subject.to_string())
            .expect("valid test VAPID keys");
        Self {
            inner: Some(Arc::new(inner)),
            allow_test_http,
        }
    }

    fn build_inner(
        pub_raw: &str,
        priv_raw: &str,
        subject: String,
    ) -> Result<PushClientInner, String> {
        let pub_bytes = decode_b64url(pub_raw).map_err(|e| format!("VAPID_PUBLIC_KEY: {e}"))?;
        if pub_bytes.len() != 65 || pub_bytes[0] != 0x04 {
            return Err(
                "VAPID_PUBLIC_KEY must decode to a 65-byte uncompressed P-256 point".into(),
            );
        }
        let priv_bytes = decode_b64url(priv_raw).map_err(|e| format!("VAPID_PRIVATE_KEY: {e}"))?;
        if priv_bytes.len() != 32 {
            return Err("VAPID_PRIVATE_KEY must decode to 32 raw scalar bytes".into());
        }

        let vapid_key = EcdsaKeyPair::from_private_key_and_public_key(
            &ECDSA_P256_SHA256_FIXED_SIGNING,
            &priv_bytes,
            &pub_bytes,
        )
        .map_err(|_| "VAPID_PRIVATE_KEY does not match VAPID_PUBLIC_KEY".to_string())?;

        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .dns_resolver(Arc::new(PublicOnlyResolver))
            .build()
            .map_err(|e| format!("Failed to construct Web Push HTTP client: {e}"))?;

        Ok(PushClientInner {
            http,
            vapid_key,
            vapid_public_key_b64: URL_SAFE_NO_PAD.encode(&pub_bytes),
            vapid_subject: subject,
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    pub fn public_key(&self) -> Option<&str> {
        self.inner
            .as_deref()
            .map(|inner| inner.vapid_public_key_b64.as_str())
    }

    fn allow_test_http(&self) -> bool {
        #[cfg(test)]
        {
            self.allow_test_http
        }
        #[cfg(not(test))]
        {
            false
        }
    }
}

pub fn decode_b64url(raw: &str) -> Result<Vec<u8>, String> {
    let trimmed = raw.trim().trim_end_matches('=');
    URL_SAFE_NO_PAD
        .decode(trimmed)
        .map_err(|_| "invalid base64url encoding".to_string())
}

/// Validates a browser PushSubscription (`endpoint`, `p256dh`, `auth`) before
/// storing or sending. Rejects non-HTTPS endpoints, embedded credentials, raw IP
/// hosts, non-443 ports, hosts outside [`ALLOWED_PUSH_HOSTS`], and malformed
/// P-256 / auth keys.
pub fn validate_push_subscription(
    endpoint: &str,
    p256dh: &str,
    auth: &str,
) -> AppResult<(String, String, String)> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() || endpoint.len() > MAX_ENDPOINT_LEN {
        return Err(AppError::BadRequest(
            "Push subscription endpoint is invalid".into(),
        ));
    }

    let parsed = Url::parse(endpoint)
        .map_err(|_| AppError::BadRequest("Push subscription endpoint is invalid".into()))?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || !matches!(parsed.port(), None | Some(443))
    {
        return Err(AppError::BadRequest(
            "Push subscription endpoint must use standard HTTPS".into(),
        ));
    }

    let Some(host) = parsed.host_str() else {
        return Err(AppError::BadRequest(
            "Push subscription endpoint must include a host".into(),
        ));
    };
    let host_lower = host.to_ascii_lowercase();
    if host.parse::<IpAddr>().is_ok()
        || host_lower.starts_with('[')
        || host_lower == "localhost"
        || host_lower.ends_with(".localhost")
        || host_lower.ends_with(".local")
        || host_lower.ends_with(".internal")
    {
        return Err(AppError::BadRequest(
            "Push subscription endpoint host is not allowed".into(),
        ));
    }
    if !is_allowed_push_host(&host_lower) {
        return Err(AppError::BadRequest(
            "Push subscription endpoint is not a recognised push service".into(),
        ));
    }

    let ua_pub = decode_b64url(p256dh)
        .map_err(|_| AppError::BadRequest("Invalid push p256dh key".into()))?;
    if ua_pub.len() != 65 || ua_pub[0] != 0x04 {
        return Err(AppError::BadRequest(
            "Push p256dh key must be a 65-byte uncompressed P-256 point".into(),
        ));
    }

    let auth_bytes =
        decode_b64url(auth).map_err(|_| AppError::BadRequest("Invalid push auth secret".into()))?;
    if auth_bytes.len() != 16 {
        return Err(AppError::BadRequest(
            "Push auth secret must be 16 bytes".into(),
        ));
    }

    Ok((
        parsed.to_string(),
        URL_SAFE_NO_PAD.encode(&ua_pub),
        URL_SAFE_NO_PAD.encode(&auth_bytes),
    ))
}

/// Pre-flight policy for a stored endpoint. The address check itself happens in
/// [`PublicOnlyResolver`] at connect time; this re-applies the string-level rules
/// so rows stored before the host allow-list existed cannot be dialed either.
fn ensure_endpoint_safe_to_dial(parsed: &Url, allow_test_http: bool) -> Result<(), String> {
    if allow_test_http && parsed.scheme() == "http" {
        return Ok(());
    }
    if parsed.scheme() != "https" || !matches!(parsed.port(), None | Some(443)) {
        return Err("Endpoint must use standard HTTPS".into());
    }
    let host = parsed.host_str().ok_or("Endpoint host missing")?;
    if host.parse::<IpAddr>().is_ok() || host.starts_with('[') {
        return Err("Raw IP endpoints are blocked".into());
    }
    if !is_allowed_push_host(host) {
        return Err("Endpoint is not a recognised push service".into());
    }
    Ok(())
}

/// Encrypts `plaintext` for Web Push (`aes128gcm`, RFC 8291) using a fresh
/// random 16-byte salt and ephemeral P-256 keypair.
pub fn encrypt_aes128gcm(
    ua_public_65: &[u8],
    auth_16: &[u8],
    plaintext: &[u8],
) -> AppResult<Vec<u8>> {
    let rng = SystemRandom::new();
    let mut salt = [0u8; 16];
    rng.fill(&mut salt)
        .map_err(|_| internal_err("RNG failure"))?;

    let as_key = agreement::EphemeralPrivateKey::generate(&ECDH_P256, &rng)
        .map_err(|_| internal_err("ECDH keygen failed"))?;
    let as_public = as_key
        .compute_public_key()
        .map_err(|_| internal_err("ECDH public key failed"))?;

    let peer = UnparsedPublicKey::new(&ECDH_P256, ua_public_65);
    let ecdh_secret = agreement::agree_ephemeral(
        as_key,
        peer,
        AppError::BadRequest("Invalid push subscriber public key".into()),
        |secret| Ok(secret.to_vec()),
    )?;

    seal_aes128gcm(
        ua_public_65,
        auth_16,
        plaintext,
        &salt,
        as_public.as_ref(),
        &ecdh_secret,
    )
}

/// Deterministic variant of RFC 8291 encryption for known-answer unit tests.
pub fn encrypt_aes128gcm_deterministic(
    ua_public_65: &[u8],
    auth_16: &[u8],
    plaintext: &[u8],
    salt_16: &[u8; 16],
    as_private_32: &[u8; 32],
) -> AppResult<Vec<u8>> {
    let as_key = PrivateKey::from_private_key(&ECDH_P256, as_private_32)
        .map_err(|_| internal_err("Invalid deterministic P-256 scalar"))?;
    let as_public = as_key
        .compute_public_key()
        .map_err(|_| internal_err("ECDH public key failed"))?;
    let peer = UnparsedPublicKey::new(&ECDH_P256, ua_public_65);
    let ecdh_secret = agreement::agree(
        &as_key,
        peer,
        AppError::BadRequest("Invalid push subscriber public key".into()),
        |secret| Ok(secret.to_vec()),
    )?;

    seal_aes128gcm(
        ua_public_65,
        auth_16,
        plaintext,
        salt_16,
        as_public.as_ref(),
        &ecdh_secret,
    )
}

fn seal_aes128gcm(
    ua_public_65: &[u8],
    auth_16: &[u8],
    plaintext: &[u8],
    salt_16: &[u8; 16],
    as_public: &[u8],
    ecdh_secret: &[u8],
) -> AppResult<Vec<u8>> {
    if ua_public_65.len() != 65 || ua_public_65[0] != 0x04 {
        return Err(AppError::BadRequest("Invalid push p256dh key".into()));
    }
    if auth_16.len() != 16 {
        return Err(AppError::BadRequest("Invalid push auth secret".into()));
    }
    if as_public.len() != 65 || as_public[0] != 0x04 {
        return Err(internal_err("Unexpected AS public key length"));
    }
    if plaintext.len() > MAX_PLAINTEXT_BYTES {
        return Err(AppError::BadRequest(
            "Push notification payload exceeds maximum size".into(),
        ));
    }

    // IKM = HKDF-SHA256(salt=auth_secret, ikm=ecdh_secret, info="WebPush: info\0" || ua_public || as_public, L=32)
    let auth_salt = hkdf::Salt::new(HKDF_SHA256, auth_16);
    let auth_prk = auth_salt.extract(ecdh_secret);
    let mut ikm = [0u8; 32];
    auth_prk
        .expand(&[b"WebPush: info\0", ua_public_65, as_public], HkdfLen(32))
        .and_then(|okm| okm.fill(&mut ikm))
        .map_err(|_| internal_err("WebPush HKDF IKM failed"))?;

    // PRK = HKDF-Extract(salt=salt_16, ikm=ikm)
    let content_salt = hkdf::Salt::new(HKDF_SHA256, salt_16);
    let content_prk = content_salt.extract(&ikm);

    let mut cek = [0u8; 16];
    content_prk
        .expand(&[b"Content-Encoding: aes128gcm\0"], HkdfLen(16))
        .and_then(|okm| okm.fill(&mut cek))
        .map_err(|_| internal_err("WebPush HKDF CEK failed"))?;

    let mut nonce_bytes = [0u8; 12];
    content_prk
        .expand(&[b"Content-Encoding: nonce\0"], HkdfLen(12))
        .and_then(|okm| okm.fill(&mut nonce_bytes))
        .map_err(|_| internal_err("WebPush HKDF nonce failed"))?;

    let unbound =
        UnboundKey::new(&AES_128_GCM, &cek).map_err(|_| internal_err("AES-128-GCM key error"))?;
    let key = LessSafeKey::new(unbound);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    let mut buffer = Vec::with_capacity(plaintext.len() + 1 + AES_128_GCM.tag_len());
    buffer.extend_from_slice(plaintext);
    buffer.push(0x02); // Final record delimiter per RFC 8188 / RFC 8291
    key.seal_in_place_append_tag(nonce, Aad::empty(), &mut buffer)
        .map_err(|_| internal_err("AES-128-GCM seal failed"))?;

    let mut body = Vec::with_capacity(16 + 4 + 1 + 65 + buffer.len());
    body.extend_from_slice(salt_16);
    body.extend_from_slice(&RECORD_SIZE.to_be_bytes());
    body.push(65);
    body.extend_from_slice(as_public);
    body.extend_from_slice(&buffer);
    Ok(body)
}

fn build_vapid_authorization(inner: &PushClientInner, endpoint: &Url) -> Result<String, String> {
    let host = endpoint.host_str().ok_or("Endpoint missing host")?;
    let aud = match endpoint.port() {
        Some(port) => format!("{}://{host}:{port}", endpoint.scheme()),
        None => format!("{}://{host}", endpoint.scheme()),
    };

    let header_json = serde_json::json!({ "typ": "JWT", "alg": "ES256" }).to_string();
    let exp = (Utc::now() + ChronoDuration::hours(12)).timestamp();
    let claims_json = serde_json::json!({
        "aud": aud,
        "exp": exp,
        "sub": inner.vapid_subject,
    })
    .to_string();

    let signing_input = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(header_json.as_bytes()),
        URL_SAFE_NO_PAD.encode(claims_json.as_bytes())
    );
    let rng = SystemRandom::new();
    let sig = inner
        .vapid_key
        .sign(&rng, signing_input.as_bytes())
        .map_err(|_| "VAPID ECDSA signing failed".to_string())?;
    let jwt = format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(sig.as_ref()));
    Ok(format!("vapid t={jwt}, k={}", inner.vapid_public_key_b64))
}

fn truncate_body_preview(body: &str, max_chars: usize) -> String {
    body.replace(['\r', '\n'], " ")
        .chars()
        .take(max_chars)
        .collect()
}

pub async fn send_encrypted_push(
    client: &SharedPushClient,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    payload: &WebPushPayload<'_>,
    urgency: &str,
) -> PushDeliveryOutcome {
    let Some(inner) = client.inner.as_deref() else {
        return PushDeliveryOutcome::Failed("Web Push is not configured".into());
    };

    let parsed = match Url::parse(endpoint) {
        Ok(u) => u,
        Err(_) => return PushDeliveryOutcome::Failed("Malformed push endpoint URL".into()),
    };
    if let Err(reason) = ensure_endpoint_safe_to_dial(&parsed, client.allow_test_http()) {
        return PushDeliveryOutcome::Failed(reason);
    }

    let ua_pub = match decode_b64url(p256dh) {
        Ok(v) if v.len() == 65 && v[0] == 0x04 => v,
        _ => return PushDeliveryOutcome::Gone("Invalid subscriber p256dh key".into()),
    };
    let auth_bytes = match decode_b64url(auth) {
        Ok(v) if v.len() == 16 => v,
        _ => return PushDeliveryOutcome::Gone("Invalid subscriber auth secret".into()),
    };

    let json_bytes = match serde_json::to_vec(payload) {
        Ok(b) => b,
        Err(e) => return PushDeliveryOutcome::Failed(format!("Payload serialization error: {e}")),
    };
    let ciphertext = match encrypt_aes128gcm(&ua_pub, &auth_bytes, &json_bytes) {
        Ok(c) => c,
        Err(e) => return PushDeliveryOutcome::Failed(format!("Encryption error: {e}")),
    };

    let auth_header = match build_vapid_authorization(inner, &parsed) {
        Ok(h) => h,
        Err(e) => return PushDeliveryOutcome::Failed(e),
    };

    let response = match inner
        .http
        .post(endpoint)
        .header("Authorization", auth_header)
        .header("Content-Type", "application/octet-stream")
        .header("Content-Encoding", "aes128gcm")
        .header("TTL", "86400")
        .header("Urgency", urgency)
        .body(ciphertext)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(err) => return PushDeliveryOutcome::Retryable(format!("Push transport error: {err}")),
    };

    let status = response.status();
    if status.is_success() {
        return PushDeliveryOutcome::Sent;
    }

    let code = status.as_u16();
    let resp_text = response.text().await.unwrap_or_default();
    let detail = if resp_text.trim().is_empty() {
        format!("HTTP {code}")
    } else {
        format!("HTTP {code}: {}", truncate_body_preview(&resp_text, 200))
    };

    if code == 404 || code == 410 {
        PushDeliveryOutcome::Gone(detail)
    } else if code == 429 || status.is_server_error() {
        PushDeliveryOutcome::Retryable(detail)
    } else {
        PushDeliveryOutcome::Failed(detail)
    }
}

enum DeliveryResult {
    Sent,
    SkippedOrGone,
    Failed,
}

async fn apply_delivery_outcome(
    pool: &PgPool,
    row: &PushOutboxBatchRow,
    outcome: PushDeliveryOutcome,
) -> Result<DeliveryResult, String> {
    match outcome {
        PushDeliveryOutcome::Sent => {
            if let Err(e) = push_outbox::mark_sent(pool, row.id).await {
                return Err(format!("mark_sent failed: {e}"));
            }
            if let Err(e) =
                push_subscriptions::record_delivery_success(pool, row.subscription_id).await
            {
                tracing::warn!(subscription_id = %row.subscription_id, error = %e, "Failed to update push subscription success timestamp");
            }
            Ok(DeliveryResult::Sent)
        }
        PushDeliveryOutcome::Gone(reason) => {
            if let Err(e) = push_outbox::mark_gone(pool, row.id, &reason).await {
                return Err(format!("mark_gone failed: {e}"));
            }
            if let Err(e) =
                push_subscriptions::revoke_by_id(pool, row.subscription_id, &reason).await
            {
                tracing::warn!(subscription_id = %row.subscription_id, error = %e, "Failed to revoke gone push subscription");
            }
            Ok(DeliveryResult::SkippedOrGone)
        }
        PushDeliveryOutcome::Retryable(err) => {
            let next_attempts = row.attempts + 1;
            if next_attempts >= row.max_attempts {
                if let Err(e) = push_outbox::mark_failed(pool, row.id, next_attempts, &err).await {
                    return Err(format!("mark_failed failed: {e}"));
                }
                if let Err(e) =
                    push_subscriptions::record_delivery_failure(pool, row.subscription_id, &err)
                        .await
                {
                    tracing::warn!(subscription_id = %row.subscription_id, error = %e, "Failed to record push subscription failure");
                }
            } else {
                let backoff_idx = (next_attempts as usize)
                    .saturating_sub(1)
                    .min(RETRY_BACKOFF_SECS.len() - 1);
                let next_retry =
                    Utc::now() + ChronoDuration::seconds(RETRY_BACKOFF_SECS[backoff_idx]);
                if let Err(e) =
                    push_outbox::mark_retry(pool, row.id, next_attempts, next_retry, &err).await
                {
                    return Err(format!("mark_retry failed: {e}"));
                }
            }
            Ok(DeliveryResult::Failed)
        }
        PushDeliveryOutcome::Failed(err) => {
            let next_attempts = row.attempts + 1;
            if let Err(e) = push_outbox::mark_failed(pool, row.id, next_attempts, &err).await {
                return Err(format!("mark_failed failed: {e}"));
            }
            if let Err(e) =
                push_subscriptions::record_delivery_failure(pool, row.subscription_id, &err).await
            {
                tracing::warn!(subscription_id = %row.subscription_id, error = %e, "Failed to record push subscription failure");
            }
            Ok(DeliveryResult::Failed)
        }
    }
}

/// Processes up to `batch_size` due rows in `push_outbox`.
pub async fn process_outbox_batch(
    pool: &PgPool,
    client: &SharedPushClient,
    batch_size: i64,
) -> AppResult<(usize, usize)> {
    if !client.is_enabled() {
        return Ok((0, 0));
    }

    let rows = push_outbox::claim_pending_batch(pool, batch_size).await?;
    if rows.is_empty() {
        return Ok((0, 0));
    }

    let mut sent = 0usize;
    let mut failed = 0usize;

    // Bound concurrency to 8 concurrent HTTP deliveries per batch.
    for chunk in rows.chunks(8) {
        let mut set = tokio::task::JoinSet::new();
        for row in chunk.iter().cloned() {
            let push = client.clone();
            let db = pool.clone();
            set.spawn(async move {
                if row.is_revoked {
                    let _ = push_outbox::mark_gone(&db, row.id, "Subscription or session revoked")
                        .await;
                    return Ok(DeliveryResult::SkippedOrGone);
                }
                let payload = WebPushPayload {
                    title: &row.title,
                    body: &row.body,
                    url: &row.url,
                    tag: row.tag.as_deref(),
                    icon: "/icons/icon-192.png",
                    badge: "/icons/badge-72.png",
                    unread_count: Some(row.unread_count),
                };
                let outcome = send_encrypted_push(
                    &push,
                    &row.endpoint,
                    &row.p256dh,
                    &row.auth,
                    &payload,
                    "normal",
                )
                .await;
                apply_delivery_outcome(&db, &row, outcome).await
            });
        }

        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(Ok(DeliveryResult::Sent)) => sent += 1,
                Ok(Ok(DeliveryResult::SkippedOrGone)) => {}
                Ok(Ok(DeliveryResult::Failed)) => failed += 1,
                Ok(Err(err)) => {
                    tracing::error!(error = %err, "Failed to persist push delivery outcome");
                    failed += 1;
                }
                Err(err) => {
                    tracing::error!(error = %err, "Push worker task panicked");
                    failed += 1;
                }
            }
        }
    }

    Ok((sent, failed))
}

/// Immediately sends a test Web Push notification to all active subscriptions
/// belonging to `user_id`.
pub async fn send_test_push(
    pool: &PgPool,
    client: &SharedPushClient,
    user_id: Uuid,
    target_url: &str,
) -> AppResult<TestPushResponse> {
    if !client.is_enabled() {
        return Err(AppError::BadRequest(
            "Push notifications are not configured on the server".into(),
        ));
    }

    let subs: Vec<PushSubscription> =
        push_subscriptions::list_active_for_user(pool, user_id).await?;
    if subs.is_empty() {
        return Ok(TestPushResponse {
            queued: 0,
            sent: 0,
            failed: 0,
            gone: 0,
        });
    }

    let payload = WebPushPayload {
        title: "PayrollMY Test Notification",
        body: "Push notifications are active on this device.",
        url: target_url,
        tag: Some("payrollmy-test"),
        icon: "/icons/icon-192.png",
        badge: "/icons/badge-72.png",
        unread_count: None,
    };

    let mut sent = 0usize;
    let mut failed = 0usize;
    let mut gone = 0usize;

    for sub in &subs {
        match send_encrypted_push(
            client,
            &sub.endpoint,
            &sub.p256dh,
            &sub.auth,
            &payload,
            "high",
        )
        .await
        {
            PushDeliveryOutcome::Sent => {
                sent += 1;
                let _ = push_subscriptions::record_delivery_success(pool, sub.id).await;
            }
            PushDeliveryOutcome::Gone(reason) => {
                gone += 1;
                let _ = push_subscriptions::revoke_by_id(pool, sub.id, &reason).await;
            }
            PushDeliveryOutcome::Retryable(err) | PushDeliveryOutcome::Failed(err) => {
                failed += 1;
                let _ = push_subscriptions::record_delivery_failure(pool, sub.id, &err).await;
            }
        }
    }

    Ok(TestPushResponse {
        queued: subs.len(),
        sent,
        failed,
        gone,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_lc_rs::signature::{ECDSA_P256_SHA256_FIXED, UnparsedPublicKey as SigPublicKey};
    use axum::{Router, extract::State, http::HeaderMap, http::StatusCode, routing::post};
    use std::sync::Mutex;
    use tokio::net::TcpListener;

    fn decrypt_aes128gcm_for_test(
        ua_private_32: &[u8; 32],
        ua_public_65: &[u8],
        auth_16: &[u8],
        packet: &[u8],
    ) -> Vec<u8> {
        assert!(packet.len() > 86, "packet too short");
        let salt = &packet[0..16];
        let rs = u32::from_be_bytes(packet[16..20].try_into().unwrap());
        assert_eq!(rs, 4096);
        let idlen = packet[20] as usize;
        assert_eq!(idlen, 65);
        let as_public = &packet[21..86];
        let mut ciphertext = packet[86..].to_vec();

        let ua_priv = PrivateKey::from_private_key(&ECDH_P256, ua_private_32).unwrap();
        let peer = UnparsedPublicKey::new(&ECDH_P256, as_public);
        let ecdh_secret = agreement::agree(&ua_priv, peer, (), |s| Ok(s.to_vec())).unwrap();

        let auth_salt = hkdf::Salt::new(HKDF_SHA256, auth_16);
        let auth_prk = auth_salt.extract(&ecdh_secret);
        let mut ikm = [0u8; 32];
        auth_prk
            .expand(&[b"WebPush: info\0", ua_public_65, as_public], HkdfLen(32))
            .unwrap()
            .fill(&mut ikm)
            .unwrap();

        let content_salt = hkdf::Salt::new(HKDF_SHA256, salt);
        let content_prk = content_salt.extract(&ikm);
        let mut cek = [0u8; 16];
        content_prk
            .expand(&[b"Content-Encoding: aes128gcm\0"], HkdfLen(16))
            .unwrap()
            .fill(&mut cek)
            .unwrap();
        let mut nonce_bytes = [0u8; 12];
        content_prk
            .expand(&[b"Content-Encoding: nonce\0"], HkdfLen(12))
            .unwrap()
            .fill(&mut nonce_bytes)
            .unwrap();

        let key = LessSafeKey::new(UnboundKey::new(&AES_128_GCM, &cek).unwrap());
        let plain = key
            .open_in_place(
                Nonce::assume_unique_for_key(nonce_bytes),
                Aad::empty(),
                &mut ciphertext,
            )
            .unwrap();
        assert_eq!(plain.last().copied(), Some(0x02));
        plain[..plain.len() - 1].to_vec()
    }

    #[test]
    fn rfc8291_aes128gcm_round_trip_and_deterministic_vector() {
        let ua_priv_bytes: [u8; 32] = [
            0x71, 0x77, 0x5d, 0x45, 0xab, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99,
            0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            0x09, 0x0a, 0x0b, 0x0c,
        ];
        let ua_key = PrivateKey::from_private_key(&ECDH_P256, &ua_priv_bytes).unwrap();
        let ua_pub = ua_key.compute_public_key().unwrap();

        let as_priv_bytes: [u8; 32] = [
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08,
        ];
        let salt: [u8; 16] = [0xaa; 16];
        let auth: [u8; 16] = [0xbb; 16];
        let msg = b"When I grow up, I want to be a watermelon";

        let pkt1 =
            encrypt_aes128gcm_deterministic(ua_pub.as_ref(), &auth, msg, &salt, &as_priv_bytes)
                .unwrap();
        let pkt2 =
            encrypt_aes128gcm_deterministic(ua_pub.as_ref(), &auth, msg, &salt, &as_priv_bytes)
                .unwrap();
        assert_eq!(pkt1, pkt2);

        let decrypted = decrypt_aes128gcm_for_test(&ua_priv_bytes, ua_pub.as_ref(), &auth, &pkt1);
        assert_eq!(decrypted, msg);

        // Random ephemeral encryption also round-trips cleanly.
        let random_pkt = encrypt_aes128gcm(ua_pub.as_ref(), &auth, msg).unwrap();
        let decrypted_random =
            decrypt_aes128gcm_for_test(&ua_priv_bytes, ua_pub.as_ref(), &auth, &random_pkt);
        assert_eq!(decrypted_random, msg);
    }

    #[test]
    fn vapid_jwt_signature_verifies_with_es256_public_key() {
        let priv_bytes: [u8; 32] = [
            0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e,
            0x2f, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c,
            0x3d, 0x3e, 0x3f, 0x40,
        ];
        let ec_key = PrivateKey::from_private_key(&ECDH_P256, &priv_bytes).unwrap();
        let pub_bytes = ec_key.compute_public_key().unwrap();

        let pub_b64 = URL_SAFE_NO_PAD.encode(pub_bytes.as_ref());
        let priv_b64 = URL_SAFE_NO_PAD.encode(priv_bytes);

        let client =
            SharedPushClient::for_test(&pub_b64, &priv_b64, "https://payrollmy.com", false);
        let inner = client.inner.as_deref().unwrap();
        let endpoint = Url::parse("https://fcm.googleapis.com/fcm/send/abc123").unwrap();
        let header = build_vapid_authorization(inner, &endpoint).unwrap();

        let rest = header.strip_prefix("vapid t=").unwrap();
        let (jwt, k_part) = rest.split_once(", k=").unwrap();
        assert_eq!(k_part, pub_b64);

        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);
        let signing_input = format!("{}.{}", parts[0], parts[1]);
        let sig = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
        assert_eq!(sig.len(), 64);

        let verifier = SigPublicKey::new(&ECDSA_P256_SHA256_FIXED, pub_bytes.as_ref());
        verifier.verify(signing_input.as_bytes(), &sig).unwrap();

        let claims: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
        assert_eq!(claims["aud"], "https://fcm.googleapis.com");
        assert_eq!(claims["sub"], "https://payrollmy.com");
    }

    #[test]
    fn endpoint_validation_rejects_ssrf_and_invalid_keys() {
        let valid_pub = URL_SAFE_NO_PAD.encode({
            let k = PrivateKey::from_private_key(&ECDH_P256, &[7u8; 32]).unwrap();
            k.compute_public_key().unwrap().as_ref().to_vec()
        });
        let valid_auth = URL_SAFE_NO_PAD.encode([9u8; 16]);

        assert!(
            validate_push_subscription(
                "https://fcm.googleapis.com/fcm/send/token-1",
                &valid_pub,
                &valid_auth,
            )
            .is_ok()
        );

        for bad_url in [
            "http://fcm.googleapis.com/send/1",
            "https://127.0.0.1/send/1",
            "https://169.254.169.254/latest/meta-data",
            "https://localhost/send/1",
            "https://push.internal/send/1",
            "https://user:pass@fcm.googleapis.com/send/1",
            "https://fcm.googleapis.com:8443/send/1",
            "https://push.example.com/send/1",
            "https://evilfcm.googleapis.com.attacker.net/send/1",
            "https://notfcm.googleapis.com.evil/send/1",
            "https://xpush.apple.com/send/1",
        ] {
            assert!(
                validate_push_subscription(bad_url, &valid_pub, &valid_auth).is_err(),
                "expected {bad_url} to be rejected"
            );
        }

        assert!(
            validate_push_subscription(
                "https://fcm.googleapis.com/send/1",
                "not-65-bytes",
                &valid_auth,
            )
            .is_err()
        );
        assert!(
            validate_push_subscription("https://fcm.googleapis.com/send/1", &valid_pub, "short",)
                .is_err()
        );
    }

    #[test]
    fn push_host_allow_list_matches_real_services_only() {
        for ok in [
            "fcm.googleapis.com",
            "FCM.googleapis.com.",
            "updates.push.services.mozilla.com",
            "web.push.apple.com",
            "wns2-par02p.notify.windows.com",
        ] {
            assert!(is_allowed_push_host(ok), "expected {ok} to be allowed");
        }
        for bad in [
            "googleapis.com",
            "evilfcm.googleapis.com",
            "fcm.googleapis.com.evil.net",
            "xpush.apple.com",
            "apple.com",
            "localhost",
        ] {
            assert!(!is_allowed_push_host(bad), "expected {bad} to be refused");
        }
    }

    #[test]
    fn stored_endpoint_outside_allow_list_is_not_dialed() {
        let parsed = Url::parse("https://attacker.example/push").unwrap();
        assert!(ensure_endpoint_safe_to_dial(&parsed, false).is_err());
        let parsed = Url::parse("https://fcm.googleapis.com/fcm/send/x").unwrap();
        assert!(ensure_endpoint_safe_to_dial(&parsed, false).is_ok());
    }

    #[tokio::test]
    async fn resolver_refuses_names_that_resolve_internally() {
        use reqwest::dns::Resolve;
        let name: reqwest::dns::Name = "localhost".parse().unwrap();
        assert!(PublicOnlyResolver.resolve(name).await.is_err());
    }

    #[tokio::test]
    async fn mock_push_server_handles_201_410_and_503() {
        let vapid_priv = [5u8; 32];
        let vapid_pub = PrivateKey::from_private_key(&ECDH_P256, &vapid_priv)
            .unwrap()
            .compute_public_key()
            .unwrap();
        let client = SharedPushClient::for_test(
            &URL_SAFE_NO_PAD.encode(vapid_pub.as_ref()),
            &URL_SAFE_NO_PAD.encode(vapid_priv),
            "mailto:ops@payrollmy.com",
            true,
        );

        let ua_priv = [8u8; 32];
        let ua_pub = PrivateKey::from_private_key(&ECDH_P256, &ua_priv)
            .unwrap()
            .compute_public_key()
            .unwrap();
        let ua_pub_b64 = URL_SAFE_NO_PAD.encode(ua_pub.as_ref());
        let auth_bytes = [3u8; 16];
        let auth_b64 = URL_SAFE_NO_PAD.encode(auth_bytes);

        type CapturedRequests = Arc<Mutex<Vec<(String, Vec<u8>)>>>;
        let captured: CapturedRequests = Arc::new(Mutex::new(Vec::new()));
        let captured_clone = captured.clone();

        let app = Router::new()
            .route(
                "/ok",
                post(
                    |State(store): State<CapturedRequests>,
                     headers: HeaderMap,
                     body: axum::body::Bytes| async move {
                        let auth_hdr = headers
                            .get("authorization")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("")
                            .to_string();
                        store.lock().unwrap().push((auth_hdr, body.to_vec()));
                        StatusCode::CREATED
                    },
                ),
            )
            .route("/gone", post(|| async { StatusCode::GONE }))
            .route("/busy", post(|| async { StatusCode::SERVICE_UNAVAILABLE }))
            .with_state(captured_clone);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let payload = WebPushPayload {
            title: "Leave Approved",
            body: "Your annual leave was approved.",
            url: "/portal/leave",
            tag: Some("leave-1"),
            icon: "/icons/icon-192.png",
            badge: "/icons/badge-72.png",
            unread_count: Some(2),
        };

        let ok_res = send_encrypted_push(
            &client,
            &format!("http://{addr}/ok"),
            &ua_pub_b64,
            &auth_b64,
            &payload,
            "normal",
        )
        .await;
        assert_eq!(ok_res, PushDeliveryOutcome::Sent);

        let gone_res = send_encrypted_push(
            &client,
            &format!("http://{addr}/gone"),
            &ua_pub_b64,
            &auth_b64,
            &payload,
            "normal",
        )
        .await;
        assert!(matches!(gone_res, PushDeliveryOutcome::Gone(_)));

        let retry_res = send_encrypted_push(
            &client,
            &format!("http://{addr}/busy"),
            &ua_pub_b64,
            &auth_b64,
            &payload,
            "normal",
        )
        .await;
        assert!(matches!(retry_res, PushDeliveryOutcome::Retryable(_)));

        let items = captured.lock().unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].0.starts_with("vapid t="));
        let plain = decrypt_aes128gcm_for_test(&ua_priv, ua_pub.as_ref(), &auth_bytes, &items[0].1);
        let decoded: serde_json::Value = serde_json::from_slice(&plain).unwrap();
        assert_eq!(decoded["title"], "Leave Approved");
        assert_eq!(decoded["url"], "/portal/leave");
        assert_eq!(decoded["unread_count"], 2);

        server.abort();
    }
}
