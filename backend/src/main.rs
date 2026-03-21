use std::net::SocketAddr;
use std::sync::Arc;

use axum::http::{HeaderValue, Method};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;
use url::Url;
use webauthn_rs::prelude::*;

use payroll_system::core::app_state::AppState;
use payroll_system::core::auth::JwtSecret;
use payroll_system::core::config::AppConfig;
use payroll_system::core::db;
use payroll_system::routes;

#[tokio::main]
async fn main() {
    // Load .env
    dotenvy::dotenv().ok();

    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Load config
    let config = AppConfig::from_env();

    // Create DB pool + run migrations
    let pool = db::create_pool(&config.database_url).await;
    db::run_migrations(&pool).await;

    tracing::info!("Database connected and migrations applied");

    // CORS — restrict to configured frontend origin
    let frontend_origin: HeaderValue = config
        .frontend_url
        .parse()
        .expect("Invalid FRONTEND_URL for CORS origin");

    let cors = CorsLayer::new()
        .allow_origin(frontend_origin)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
        ])
        .allow_credentials(true);

    // WebAuthn
    let rp_origin = Url::parse(&config.webauthn_rp_origin)
        .expect("Invalid WEBAUTHN_RP_ORIGIN URL");
    let webauthn = WebauthnBuilder::new(&config.webauthn_rp_id, &rp_origin)
        .expect("Failed to build WebAuthn")
        .rp_name("PayrollMY")
        .build()
        .expect("Failed to build WebAuthn");

    // App state
    let state = AppState {
        pool: pool.clone(),
        config: config.clone(),
        webauthn: Arc::new(webauthn),
    };

    // Build router
    let app = routes::create_router(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(axum::Extension(JwtSecret(config.jwt_secret.clone())));

    // Start server
    let addr: SocketAddr = format!("{}:{}", config.server_host, config.server_port)
        .parse()
        .expect("Invalid server address");

    tracing::info!("Starting server on {}", addr);

    // Background task: clean up stale refresh tokens every 24 hours
    let cleanup_pool = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
        loop {
            interval.tick().await;
            match sqlx::query(
                "DELETE FROM refresh_tokens \
                 WHERE (revoked = TRUE OR expires_at < NOW()) \
                 AND created_at < NOW() - INTERVAL '30 days'",
            )
            .execute(&cleanup_pool)
            .await
            {
                Ok(result) => {
                    if result.rows_affected() > 0 {
                        tracing::info!(
                            "Cleaned up {} stale refresh tokens",
                            result.rows_affected()
                        );
                    }
                }
                Err(e) => tracing::error!("Failed to clean up refresh tokens: {}", e),
            }
        }
    });

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
