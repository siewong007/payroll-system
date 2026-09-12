//! Cloudflare Turnstile server-side verification for unauthenticated,
//! bot-abusable endpoints (login, password reset, passkey begins, OAuth
//! authorize).
//!
//! Verification is a no-op when `TURNSTILE_SECRET_KEY` is unset so local
//! dev and CI need no keys. When it IS set the outcome is fail-closed in
//! both directions: a missing token is a 400, a rejected token is a 403,
//! and Cloudflare being unreachable is a 502 — a captcha that passes when
//! it cannot run protects nothing.

use std::time::Duration;

use serde::Deserialize;

use crate::core::config::AppConfig;
use crate::core::error::{AppError, AppResult};

const VERIFY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Deserialize)]
struct SiteverifyResponse {
    success: bool,
    #[serde(default, rename = "error-codes")]
    error_codes: Vec<String>,
}

/// Check `token` with Cloudflare. `remote_ip` is the server-observed client
/// address (same `core::client_ip` resolution the audit trail uses); it is
/// optional evidence for Cloudflare, never load-bearing.
pub async fn verify(
    config: &AppConfig,
    token: Option<&str>,
    remote_ip: Option<&str>,
) -> AppResult<()> {
    let Some(secret) = config.turnstile_secret_key.as_deref() else {
        return Ok(());
    };
    let token = token
        .filter(|t| !t.is_empty())
        .ok_or_else(|| AppError::BadRequest("Human verification required".into()))?;

    let client = reqwest::Client::builder()
        .timeout(VERIFY_TIMEOUT)
        .build()
        .map_err(|e| {
            tracing::warn!("Turnstile: could not build HTTP client: {}", e);
            verify_unavailable()
        })?;

    let mut form = vec![("secret", secret), ("response", token)];
    if let Some(ip) = remote_ip {
        form.push(("remoteip", ip));
    }

    let outcome = client
        .post(&config.turnstile_verify_url)
        .form(&form)
        .send()
        .await
        .and_then(|r| r.error_for_status());

    let body = match outcome {
        Ok(resp) => resp.json::<SiteverifyResponse>().await.map_err(|e| {
            tracing::warn!("Turnstile: unreadable siteverify body: {}", e);
            verify_unavailable()
        })?,
        Err(e) => {
            tracing::warn!("Turnstile: siteverify request failed: {}", e);
            return Err(verify_unavailable());
        }
    };

    if body.success {
        Ok(())
    } else {
        tracing::info!("Turnstile rejected token: {:?}", body.error_codes);
        Err(AppError::Forbidden("Human verification failed".into()))
    }
}

/// Uniform 502 for every "could not reach/understand Cloudflare" outcome —
/// `BadGateway` exists so a third party's outage is not reported as our bug.
fn verify_unavailable() -> AppError {
    AppError::BadGateway("Human verification is temporarily unavailable".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Literal fixture — `AppConfig::from_env()` panics without env vars, so
    // tests must not call it.
    fn config(secret: Option<&str>, url: &str) -> AppConfig {
        AppConfig {
            database_url: String::new(),
            jwt_secret: String::new(),
            totp_encryption_key: String::new(),
            jwt_expiry_hours: 1,
            server_host: String::new(),
            server_port: 0,
            frontend_url: String::new(),
            api_public_url: String::new(),
            google_client_id: None,
            google_client_secret: None,
            webauthn_rp_id: String::new(),
            webauthn_rp_origin: String::new(),
            smtp_host: None,
            smtp_port: None,
            smtp_username: None,
            smtp_password: None,
            smtp_from_email: None,
            smtp_from_name: None,
            trust_proxy_headers: false,
            turnstile_secret_key: secret.map(str::to_string),
            turnstile_verify_url: url.to_string(),
        }
    }

    #[tokio::test]
    async fn skips_when_secret_unset() {
        let c = config(None, "http://127.0.0.1:1/unused");
        assert!(verify(&c, None, None).await.is_ok());
    }

    #[tokio::test]
    async fn rejects_missing_token_when_configured() {
        let c = config(Some("secret"), "http://127.0.0.1:1/unused");
        let err = verify(&c, None, None).await.unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    // Tiny in-process stub: no new dev-deps.
    async fn stub(success: bool) -> String {
        let app = axum::Router::new().route(
            "/siteverify",
            axum::routing::post(move || async move {
                axum::Json(serde_json::json!({"success": success}))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/siteverify")
    }

    #[tokio::test]
    async fn passes_on_cloudflare_success() {
        let url = stub(true).await;
        let c = config(Some("secret"), &url);
        assert!(verify(&c, Some("tok"), Some("1.2.3.4")).await.is_ok());
    }

    #[tokio::test]
    async fn forbids_on_cloudflare_failure() {
        let url = stub(false).await;
        let c = config(Some("secret"), &url);
        let err = verify(&c, Some("tok"), None).await.unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn bad_gateway_when_unreachable() {
        let c = config(Some("secret"), "http://127.0.0.1:1/siteverify");
        let err = verify(&c, Some("tok"), None).await.unwrap_err();
        assert!(matches!(err, AppError::BadGateway(_)));
    }
}
