//! Cloudflare Turnstile server-side verification for unauthenticated,
//! bot-abusable endpoints (login, password reset, passkey begins, OAuth
//! authorize).
//!
//! Verification is a no-op when `TURNSTILE_SECRET_KEY` is unset so local
//! dev and CI need no keys. When it IS set the outcome is fail-closed in
//! both directions: a missing token is a 400, a rejected token is a 403,
//! and Cloudflare being unreachable is a 502 — a captcha that passes when
//! it cannot run protects nothing.

use std::sync::OnceLock;
use std::time::Duration;

use serde::Deserialize;

use crate::core::config::AppConfig;
use crate::core::error::{AppError, AppResult};

const VERIFY_TIMEOUT: Duration = Duration::from_secs(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

static TURNSTILE_CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();

fn turnstile_http_client() -> AppResult<&'static reqwest::Client> {
    match TURNSTILE_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(VERIFY_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .map_err(|e| format!("Turnstile: could not build HTTP client: {e}"))
    }) {
        Ok(client) => Ok(client),
        Err(msg) => {
            tracing::warn!("{msg}");
            Err(verify_unavailable())
        }
    }
}

#[derive(Deserialize)]
struct SiteverifyResponse {
    success: bool,
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default, rename = "error-codes")]
    error_codes: Vec<String>,
}

fn expected_turnstile_hostname(frontend_url: &str) -> Option<String> {
    url::Url::parse(frontend_url.trim())
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
}

/// Check `token` with Cloudflare, validating both `hostname` (against
/// `config.frontend_url`) and `action` (against `expected_action`).
/// `remote_ip` is the server-observed client address (same `core::client_ip`
/// resolution the audit trail uses); it is optional evidence for Cloudflare.
pub async fn verify(
    config: &AppConfig,
    token: Option<&str>,
    remote_ip: Option<&str>,
    expected_action: &str,
) -> AppResult<()> {
    verify_any_action(config, token, remote_ip, &[expected_action]).await
}

/// Like [`verify`], but accepts any action in `expected_actions` (for entry
/// endpoints triggered by a shared widget such as the login card).
pub async fn verify_any_action(
    config: &AppConfig,
    token: Option<&str>,
    remote_ip: Option<&str>,
    expected_actions: &[&str],
) -> AppResult<()> {
    let Some(secret) = config.turnstile_secret_key.as_deref() else {
        return Ok(());
    };
    let token = token
        .filter(|t| !t.is_empty())
        .ok_or_else(|| AppError::BadRequest("Human verification required".into()))?;

    let client = turnstile_http_client()?;

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

    if !body.success {
        tracing::info!("Turnstile rejected token: {:?}", body.error_codes);
        return Err(AppError::Forbidden("Human verification failed".into()));
    }

    if let Some(expected_host) = expected_turnstile_hostname(&config.frontend_url) {
        let actual_host = body.hostname.as_deref().unwrap_or("");
        if !actual_host.eq_ignore_ascii_case(&expected_host) {
            tracing::info!(
                expected_host = %expected_host,
                actual_host = %actual_host,
                "Turnstile rejected token due to hostname mismatch"
            );
            return Err(AppError::Forbidden("Human verification failed".into()));
        }
    } else if config
        .frontend_url
        .to_ascii_lowercase()
        .starts_with("https://")
    {
        return Err(AppError::Internal(
            "FRONTEND_URL must be a valid URL for Turnstile validation".into(),
        ));
    }

    if !body
        .action
        .as_deref()
        .is_some_and(|actual| expected_actions.contains(&actual))
    {
        tracing::info!(
            expected_actions = ?expected_actions,
            actual_action = ?body.action,
            "Turnstile rejected token due to action mismatch"
        );
        return Err(AppError::Forbidden("Human verification failed".into()));
    }

    Ok(())
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
            frontend_url: "http://localhost:5173".into(),
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
            client_ip_header: None,
            turnstile_secret_key: secret.map(str::to_string),
            allow_unconfigured_turnstile: false,
            turnstile_verify_url: url.to_string(),
            vapid_public_key: None,
            vapid_private_key: None,
            vapid_subject: None,
        }
    }

    #[tokio::test]
    async fn skips_when_secret_unset() {
        let c = config(None, "http://127.0.0.1:1/unused");
        assert!(verify(&c, None, None, "login").await.is_ok());
    }

    #[tokio::test]
    async fn rejects_missing_token_when_configured() {
        let c = config(Some("secret"), "http://127.0.0.1:1/unused");
        let err = verify(&c, None, None, "login").await.unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    // Tiny in-process stub: no new dev-deps.
    async fn stub(success: bool, hostname: &'static str, action: &'static str) -> String {
        let app = axum::Router::new().route(
            "/siteverify",
            axum::routing::post(move || async move {
                axum::Json(serde_json::json!({
                    "success": success,
                    "hostname": hostname,
                    "action": action,
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/siteverify")
    }

    #[tokio::test]
    async fn passes_on_cloudflare_success() {
        let url = stub(true, "localhost", "login").await;
        let c = config(Some("secret"), &url);
        assert!(
            verify(&c, Some("tok"), Some("1.2.3.4"), "login")
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn forbids_on_cloudflare_failure() {
        let url = stub(false, "localhost", "login").await;
        let c = config(Some("secret"), &url);
        let err = verify(&c, Some("tok"), None, "login").await.unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn forbids_on_action_or_hostname_mismatch() {
        let bad_action = stub(true, "localhost", "forgot_password").await;
        let c1 = config(Some("secret"), &bad_action);
        let err1 = verify(&c1, Some("tok"), None, "login").await.unwrap_err();
        assert!(matches!(err1, AppError::Forbidden(_)));

        let bad_host = stub(true, "evil.example", "login").await;
        let c2 = config(Some("secret"), &bad_host);
        let err2 = verify(&c2, Some("tok"), None, "login").await.unwrap_err();
        assert!(matches!(err2, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn bad_gateway_when_unreachable() {
        let c = config(Some("secret"), "http://127.0.0.1:1/siteverify");
        let err = verify(&c, Some("tok"), None, "login").await.unwrap_err();
        assert!(matches!(err, AppError::BadGateway(_)));
    }
}
