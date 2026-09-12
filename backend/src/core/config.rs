use std::env;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub jwt_secret: String,
    /// Dedicated AES key material for encrypting TOTP secrets at rest.
    /// Deliberately separate from `jwt_secret` so rotating the JWT secret
    /// after a leak cannot lock every 2FA user out of their own accounts.
    pub totp_encryption_key: String,
    pub jwt_expiry_hours: i64,
    pub server_host: String,
    pub server_port: u16,
    pub frontend_url: String,
    /// The API's externally reachable origin, when it differs from the
    /// frontend's. Empty means same-origin (local dev, where Vite proxies
    /// /api). Split-origin deploys set this so Google's callback lands on
    /// the Axum host rather than the static site.
    pub api_public_url: String,
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    // WebAuthn
    pub webauthn_rp_id: String,
    pub webauthn_rp_origin: String,
    // SMTP
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_from_email: Option<String>,
    pub smtp_from_name: Option<String>,
    /// Whether `X-Forwarded-For` / `X-Real-IP` may be trusted for rate-limiting
    /// keys. Enable only when a trusted proxy (CloudFront/ALB) is the sole path
    /// to the API — otherwise clients can spoof the header and bypass limits.
    pub trust_proxy_headers: bool,
    /// Cloudflare Turnstile secret. Unset = verification skipped on every
    /// protected endpoint (local dev, CI). Set in production only.
    pub turnstile_secret_key: Option<String>,
    /// Operator-trusted verify endpoint; overridden only by tests pointing
    /// at an in-process stub. Never tenant-supplied, so no SSRF policy applies.
    pub turnstile_verify_url: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            jwt_secret: env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
            totp_encryption_key: env::var("TOTP_ENCRYPTION_KEY")
                .expect("TOTP_ENCRYPTION_KEY must be set"),
            jwt_expiry_hours: env::var("JWT_EXPIRY_HOURS")
                .unwrap_or_else(|_| "1".to_string())
                .parse()
                .expect("JWT_EXPIRY_HOURS must be a number"),
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("SERVER_PORT must be a number"),
            frontend_url: env::var("FRONTEND_URL")
                .unwrap_or_else(|_| "http://localhost:5173".to_string()),
            webauthn_rp_id: env::var("WEBAUTHN_RP_ID").unwrap_or_else(|_| "localhost".to_string()),
            webauthn_rp_origin: env::var("WEBAUTHN_RP_ORIGIN")
                .unwrap_or_else(|_| "http://localhost:5173".to_string()),
            api_public_url: env::var("API_PUBLIC_URL").unwrap_or_default(),
            google_client_id: env::var("GOOGLE_CLIENT_ID").ok().filter(|v| !v.is_empty()),
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET")
                .ok()
                .filter(|v| !v.is_empty()),
            smtp_host: env::var("SMTP_HOST").ok(),
            smtp_port: env::var("SMTP_PORT").ok().and_then(|p| p.parse().ok()),
            smtp_username: env::var("SMTP_USERNAME").ok(),
            smtp_password: env::var("SMTP_PASSWORD").ok(),
            smtp_from_email: env::var("SMTP_FROM_EMAIL").ok(),
            smtp_from_name: env::var("SMTP_FROM_NAME").ok(),
            // Defaults to false: believing a forwarded header on a directly
            // reachable API lets anyone bypass the login rate limiter.
            trust_proxy_headers: env::var("TRUST_PROXY_HEADERS")
                .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
                .unwrap_or(false),
            turnstile_secret_key: env::var("TURNSTILE_SECRET_KEY")
                .ok()
                .filter(|v| !v.is_empty()),
            turnstile_verify_url: env::var("TURNSTILE_VERIFY_URL").unwrap_or_else(|_| {
                "https://challenges.cloudflare.com/turnstile/v0/siteverify".to_string()
            }),
        }
    }

    pub fn google_oauth_enabled(&self) -> bool {
        // Empty strings count as absent: compose renders unset optional vars
        // as "", and a Some("") would advertise a login that cannot work.
        self.google_client_id
            .as_deref()
            .is_some_and(|v| !v.is_empty())
            && self
                .google_client_secret
                .as_deref()
                .is_some_and(|v| !v.is_empty())
    }

    /// The URI Google redirects the browser to after a *login* consent: an
    /// Axum route, so it must be reachable on the API's own origin.
    pub fn google_redirect_uri(&self) -> String {
        let base = if self.api_public_url.is_empty() {
            &self.frontend_url
        } else {
            &self.api_public_url
        };
        format!(
            "{}/api/auth/oauth2/google/callback",
            base.trim_end_matches('/')
        )
    }

    /// The URI Google redirects the browser to after a *linking* consent: a
    /// SPA route, which then POSTs the code with the user's JWT attached.
    pub fn google_link_redirect_uri(&self) -> String {
        format!("{}/oauth2/link", self.frontend_url.trim_end_matches('/'))
    }

    pub fn smtp_enabled(&self) -> bool {
        self.smtp_host.is_some() && self.smtp_from_email.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    fn config() -> AppConfig {
        AppConfig {
            database_url: "postgres://localhost/test".into(),
            jwt_secret: "test-secret".into(),
            totp_encryption_key: "test-totp-encryption-key".into(),
            jwt_expiry_hours: 1,
            server_host: "0.0.0.0".into(),
            server_port: 8080,
            frontend_url: "http://localhost:5173".into(),
            api_public_url: String::new(),
            google_client_id: None,
            google_client_secret: None,
            webauthn_rp_id: "localhost".into(),
            webauthn_rp_origin: "http://localhost:5173".into(),
            smtp_host: None,
            smtp_port: None,
            smtp_username: None,
            smtp_password: None,
            smtp_from_email: None,
            smtp_from_name: None,
            trust_proxy_headers: false,
            turnstile_secret_key: None,
            turnstile_verify_url: "https://challenges.cloudflare.com/turnstile/v0/siteverify"
                .into(),
        }
    }

    #[test]
    fn google_oauth_needs_both_halves_of_the_credential() {
        let mut cfg = config();
        assert!(!cfg.google_oauth_enabled());

        cfg.google_client_id = Some("client-id".into());
        // A client id with no secret cannot complete the code exchange, so the
        // feature must stay off rather than half-advertise itself.
        assert!(!cfg.google_oauth_enabled());

        cfg.google_client_secret = Some("client-secret".into());
        assert!(cfg.google_oauth_enabled());

        cfg.google_client_id = None;
        assert!(!cfg.google_oauth_enabled());
    }

    #[test]
    fn google_oauth_treats_empty_credentials_as_absent() {
        // Compose renders unset optional vars as "" — a Some("") must not
        // turn the feature on, or the button renders against a dead client id.
        let mut cfg = config();
        cfg.google_client_id = Some(String::new());
        cfg.google_client_secret = Some(String::new());
        assert!(!cfg.google_oauth_enabled());
    }

    #[test]
    fn google_login_callback_uri_targets_the_api_origin() {
        let mut cfg = config();
        // Split-origin deploy: the SPA is static on payrollmy.com, Axum lives
        // on api.payrollmy.com. The callback is an Axum route — aimed at the
        // frontend it never reaches the server.
        cfg.api_public_url = "https://api.payrollmy.com".into();
        cfg.frontend_url = "https://payrollmy.com".into();

        assert_eq!(
            cfg.google_redirect_uri(),
            "https://api.payrollmy.com/api/auth/oauth2/google/callback"
        );
    }

    #[test]
    fn google_login_callback_uri_falls_back_to_the_frontend_origin() {
        // Empty api_public_url = same-origin deploy (local dev, where Vite
        // proxies /api). One variable fewer to configure.
        let cfg = config();
        assert_eq!(
            cfg.google_redirect_uri(),
            "http://localhost:5173/api/auth/oauth2/google/callback"
        );
    }

    #[test]
    fn google_link_redirect_uri_targets_the_spa() {
        // Linking must land the browser back in the SPA so the page can POST
        // the code with the user's JWT — a different destination from the
        // login callback even when the origins are split.
        let mut cfg = config();
        cfg.api_public_url = "https://api.payrollmy.com".into();
        cfg.frontend_url = "https://payrollmy.com/".into();

        assert_eq!(
            cfg.google_link_redirect_uri(),
            "https://payrollmy.com/oauth2/link"
        );
    }

    #[test]
    fn smtp_needs_both_a_host_and_a_from_address() {
        let mut cfg = config();
        assert!(!cfg.smtp_enabled());

        cfg.smtp_host = Some("smtp.example.test".into());
        assert!(!cfg.smtp_enabled());

        cfg.smtp_from_email = Some("payroll@example.test".into());
        assert!(cfg.smtp_enabled());

        cfg.smtp_host = None;
        assert!(!cfg.smtp_enabled());
    }

    #[test]
    fn optional_credentials_do_not_gate_the_features() {
        // Username/password are optional (an open relay or IP-allowlisted MTA),
        // so they must not decide whether SMTP is considered configured.
        let mut cfg = config();
        cfg.smtp_host = Some("smtp.example.test".into());
        cfg.smtp_from_email = Some("payroll@example.test".into());

        assert!(cfg.smtp_enabled());
        assert!(cfg.smtp_username.is_none());
        assert!(cfg.smtp_password.is_none());
    }
}
