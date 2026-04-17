use std::env;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_expiry_hours: i64,
    pub server_host: String,
    pub server_port: u16,
    pub frontend_url: String,
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
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            jwt_secret: env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
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
            google_client_id: env::var("GOOGLE_CLIENT_ID").ok(),
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET").ok(),
            smtp_host: env::var("SMTP_HOST").ok(),
            smtp_port: env::var("SMTP_PORT").ok().and_then(|p| p.parse().ok()),
            smtp_username: env::var("SMTP_USERNAME").ok(),
            smtp_password: env::var("SMTP_PASSWORD").ok(),
            smtp_from_email: env::var("SMTP_FROM_EMAIL").ok(),
            smtp_from_name: env::var("SMTP_FROM_NAME").ok(),
        }
    }

    pub fn google_oauth_enabled(&self) -> bool {
        self.google_client_id.is_some() && self.google_client_secret.is_some()
    }

    pub fn smtp_enabled(&self) -> bool {
        self.smtp_host.is_some() && self.smtp_from_email.is_some()
    }
}
