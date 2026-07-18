use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use chrono::Timelike;
use tokio::net::TcpListener;

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
async fn main() -> anyhow::Result<()> {
    // Load .env
    dotenvy::dotenv().ok();

    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Load config
    let config = AppConfig::from_env();

    // Claim the configured address before running migrations or spawning background work.
    // This makes startup fail fast with an actionable error when another process owns the port.
    let addr: SocketAddr = format!("{}:{}", config.server_host, config.server_port)
        .parse()
        .with_context(|| {
            format!(
                "invalid API server address {}:{}; check SERVER_HOST and SERVER_PORT",
                config.server_host, config.server_port
            )
        })?;
    let listener = bind_api_listener(addr).await?;

    // Create DB pool + run migrations
    let pool = db::create_pool(&config.database_url).await;
    db::run_migrations(&pool).await;

    tracing::info!("Database connected; schema and reference data applied");

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
    let rp_origin = Url::parse(&config.webauthn_rp_origin).expect("Invalid WEBAUTHN_RP_ORIGIN URL");
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
        .layer(axum::Extension(JwtSecret(config.jwt_secret.clone())))
        // Gzip-compress eligible responses (large JSON lists, CSV/report exports).
        // Outermost so it wraps the final response body.
        .layer(tower_http::compression::CompressionLayer::new());

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

    // Background task: auto-mark absent employees daily at 12:30 PM MYT (04:30 UTC)
    let absent_pool = pool.clone();
    tokio::spawn(async move {
        use payroll_system::services::attendance_service;

        // Wait until the next 04:30 UTC, then run daily
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60 * 60)); // check hourly
        loop {
            interval.tick().await;
            // Only run between 04:00-05:00 UTC (12:00-13:00 MYT)
            let now = chrono::Utc::now();
            if now.hour() != 4 {
                continue;
            }

            tracing::info!("Running auto-absent marking...");
            match attendance_service::mark_absent_for_date(&absent_pool, "Asia/Kuala_Lumpur").await
            {
                Ok(count) => {
                    if count > 0 {
                        tracing::info!("Auto-marked {} employees as absent", count);
                    }
                }
                Err(e) => tracing::error!("Auto-absent marking failed: {}", e),
            }
            // Sleep past this hour to avoid re-running
            tokio::time::sleep(std::time::Duration::from_secs(60 * 60)).await;
        }
    });

    tracing::info!("Starting server on {}", addr);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("API server stopped unexpectedly")?;

    tracing::info!("Shutting down — closing database pool...");
    pool.close().await;
    tracing::info!("Shutdown complete");

    Ok(())
}

async fn bind_api_listener(addr: SocketAddr) -> anyhow::Result<TcpListener> {
    match TcpListener::bind(addr).await {
        Ok(listener) => Ok(listener),
        Err(error) if error.kind() == ErrorKind::AddrInUse => {
            Err(anyhow::Error::new(error).context(format!(
                "cannot start API server on {addr}: port {} is already in use; stop the existing process or set SERVER_PORT to a free port",
                addr.port()
            )))
        }
        Err(error) => {
            Err(anyhow::Error::new(error).context(format!("failed to bind API server to {addr}")))
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received SIGINT"),
        _ = terminate => tracing::info!("Received SIGTERM"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bind_api_listener_explains_address_conflicts() {
        let occupied_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let occupied_addr = occupied_listener.local_addr().unwrap();

        let error = bind_api_listener(occupied_addr).await.unwrap_err();
        let message = format!("{error:#}");

        assert!(message.contains(&occupied_addr.to_string()));
        assert!(message.contains("already in use"));
        assert!(message.contains("SERVER_PORT"));
    }
}
