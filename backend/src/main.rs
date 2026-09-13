use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use tokio::net::TcpListener;

use axum::http::{HeaderValue, Method};
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;
use url::Url;
use webauthn_rs::prelude::*;

use payroll_system::core::app_state::AppState;
use payroll_system::core::auth::JwtSecret;
use payroll_system::core::config::AppConfig;
use payroll_system::core::db;
use payroll_system::routes;
use payroll_system::services;

/// Response for a caught handler panic.
///
/// Mirrors `AppError`'s `{"error","status"}` shape so a client parsing our error
/// body needs no special case for a crash. The panic payload is logged and never
/// sent — it routinely contains internal state.
fn panic_response(err: Box<dyn std::any::Any + Send + 'static>) -> axum::response::Response {
    use axum::response::IntoResponse;

    let detail = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| err.downcast_ref::<&'static str>().copied())
        .unwrap_or("non-string panic payload");
    tracing::error!(panic = detail, "Handler panicked — returning 500");

    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        axum::Json(serde_json::json!({
            "error": "Internal server error",
            "status": 500,
        })),
    )
        .into_response()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env
    dotenvy::dotenv().ok();

    // Init tracing
    // Structured JSON when running as a container (LOG_FORMAT=json); humans
    // get the pretty formatter locally (plan item 24). Without per-request
    // telemetry the deploy log documented its own blindness.
    let log_format = std::env::var("LOG_FORMAT").unwrap_or_else(|_| "pretty".into());
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false);
    if log_format.eq_ignore_ascii_case("json") {
        subscriber.json().init();
    } else {
        subscriber.init();
    }

    // Load config
    let config = AppConfig::from_env();

    // Retention windows for the daily cleanup task (plan item 26). Audit rows
    // default to the seven-year statutory horizon; the others are operational.
    const AUDIT_LOG_RETENTION_DAYS: i64 = 2_555;
    const NOTIFICATION_READ_RETENTION_DAYS: i32 = 90;
    const NOTIFICATION_UNREAD_RETENTION_DAYS: i32 = 365;
    const EMAIL_LOG_RETENTION_DAYS: i32 = 180;
    const SESSION_REVOKED_RETENTION_DAYS: i32 = 30;
    const SESSION_INACTIVE_RETENTION_DAYS: i32 = 90;

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

    // Migrate any TOTP secrets still encrypted under the old JWT-derived key
    // to the dedicated TOTP_ENCRYPTION_KEY. Idempotent; no-op once every row
    // is under the dedicated key. Runs before serving so a login never has to
    // fall back to the legacy derivation.
    services::totp_service::reencrypt_all(&pool, &config.totp_encryption_key, &config.jwt_secret)
        .await?;

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
        // Innermost of the added layers, so it wraps the handlers themselves.
        // A panic in a handler otherwise kills the task and drops the
        // connection: the client sees a reset rather than a response, and
        // nothing distinguishes it from a network fault. Applied *inside* the
        // CORS layer so the 500 it synthesises still carries CORS headers and
        // the browser can read it.
        .layer(CatchPanicLayer::custom(panic_response))
        // Outermost request bound: a handler that cannot finish within this
        // budget returns 408/503-shaped failure instead of holding its worker,
        // its connection and possibly a DB slot forever. PDF/statutory export
        // routes stay comfortably inside it on the production host.
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            std::time::Duration::from_secs(30),
        ))
        .layer(cors)
        // Correlation IDs: honour an upstream x-request-id (CloudFront issues
        // one) or mint a UUID, and echo it back — so a user-reported timestamp
        // maps to exactly one log line across API and access logs.
        .layer(tower_http::request_id::PropagateRequestIdLayer::x_request_id())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::extract::Request| {
                    let request_id = request
                        .extensions()
                        .get::<tower_http::request_id::RequestId>()
                        .and_then(|id| id.header_value().to_str().ok())
                        .unwrap_or("-");
                    tracing::info_span!(
                        "http_request",
                        request_id = %request_id,
                        method = %request.method(),
                        uri = %request.uri(),
                    )
                })
                .on_response(
                    |response: &axum::response::Response,
                     latency: std::time::Duration,
                     _span: &tracing::Span| {
                        tracing::info!(
                            status = %response.status(),
                            latency_ms = latency.as_millis() as u64,
                            "request completed"
                        )
                    },
                ),
        )
        .layer(tower_http::request_id::SetRequestIdLayer::x_request_id(
            tower_http::request_id::MakeRequestUuid,
        ))
        .layer(axum::Extension(JwtSecret(config.jwt_secret.clone())))
        // Gzip-compress eligible responses (large JSON lists, CSV/report exports).
        // Outermost so it wraps the final response body.
        .layer(tower_http::compression::CompressionLayer::new());

    // Background task: clean up stale refresh tokens and expired attendance
    // QR tokens every 24 hours. Every tick logs, even when nothing is deleted,
    // so a silent log means the runtime's timers are wedged — not that the
    // task had nothing to do.
    let cleanup_pool = pool.clone();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let mut cleanup_shutdown = shutdown_rx.clone();
    let cleanup_handle = tokio::spawn(async move {
        use payroll_system::repositories::{attendance_network_observations, attendance_qr_tokens};
        use payroll_system::services::attendance_network_service;

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
        // After an OS sleep/wake gap, fire one delayed tick instead of a
        // catch-up burst of every tick missed while suspended.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = cleanup_shutdown.changed() => break,
                _ = interval.tick() => {}
            }
            tracing::info!("refresh-token cleanup: tick");
            match sqlx::query(
                "DELETE FROM refresh_tokens \
                 WHERE (revoked = TRUE OR expires_at < NOW()) \
                 AND created_at < NOW() - INTERVAL '30 days'",
            )
            .execute(&cleanup_pool)
            .await
            {
                Ok(result) => {
                    tracing::info!(
                        rows = result.rows_affected(),
                        "refresh-token cleanup: completed"
                    );
                }
                Err(e) => tracing::error!("Failed to clean up refresh tokens: {}", e),
            }

            // A kiosk mints ~288 QR tokens/day; unreferenced expired ones are
            // dead weight. Tokens referenced by a check-in are kept as history.
            match attendance_qr_tokens::purge_expired(&cleanup_pool, 7).await {
                Ok(rows) => tracing::info!(rows, "qr-token cleanup: completed"),
                Err(e) => tracing::error!("Failed to clean up attendance QR tokens: {}", e),
            }

            // Attendance network observations are employees' home and mobile
            // addresses. They exist to inform a proposal, and stop being able
            // to do that once they age out of the learning window — so this is
            // a PDPA retention obligation, not a housekeeping nicety.
            match attendance_network_observations::purge_older_than(
                &cleanup_pool,
                attendance_network_service::OBSERVATION_RETENTION_DAYS,
            )
            .await
            {
                Ok(rows) => tracing::info!(rows, "network-observation cleanup: completed"),
                Err(e) => {
                    tracing::error!("Failed to clean up network observations: {}", e)
                }
            }

            // A dismissal means "not right now". Letting them lapse is what
            // makes a block proposable again after an office moves.
            match attendance_network_observations::purge_expired_dismissals(&cleanup_pool).await {
                Ok(rows) => tracing::info!(rows, "network-dismissal cleanup: completed"),
                Err(e) => tracing::error!("Failed to clean up network dismissals: {}", e),
            }

            // Retention purges for the append-only tables (plan item 26).
            // Nothing deleted from these before, so each one grew forever:
            // audit_logs and notifications are tenant data, email_logs holds
            // recipient addresses, user_sessions accumulate one per login and
            // bulk_import_sessions embed whole validated CSVs in jsonb.
            match payroll_system::repositories::audit_logs::purge_older_than(
                &cleanup_pool,
                AUDIT_LOG_RETENTION_DAYS,
            )
            .await
            {
                Ok(rows) => tracing::info!(rows, "audit-log retention: completed"),
                Err(e) => tracing::error!("Failed to purge old audit logs: {}", e),
            }

            match payroll_system::repositories::notifications::purge_stale(
                &cleanup_pool,
                NOTIFICATION_READ_RETENTION_DAYS,
                NOTIFICATION_UNREAD_RETENTION_DAYS,
            )
            .await
            {
                Ok(rows) => tracing::info!(rows, "notification retention: completed"),
                Err(e) => tracing::error!("Failed to purge stale notifications: {}", e),
            }

            match payroll_system::repositories::email_logs::purge_older_than(
                &cleanup_pool,
                EMAIL_LOG_RETENTION_DAYS,
            )
            .await
            {
                Ok(rows) => tracing::info!(rows, "email-log retention: completed"),
                Err(e) => tracing::error!("Failed to purge old email logs: {}", e),
            }

            match payroll_system::repositories::user_sessions::purge_stale(
                &cleanup_pool,
                SESSION_REVOKED_RETENTION_DAYS,
                SESSION_INACTIVE_RETENTION_DAYS,
            )
            .await
            {
                Ok(rows) => tracing::info!(rows, "session retention: completed"),
                Err(e) => tracing::error!("Failed to purge stale sessions: {}", e),
            }

            match payroll_system::repositories::bulk_import_sessions::purge_expired(&cleanup_pool)
                .await
            {
                Ok(rows) => tracing::info!(rows, "import-session retention: completed"),
                Err(e) => tracing::error!("Failed to purge expired import sessions: {}", e),
            }
        }
    });

    // Background task: auto-mark absent employees once a day, at 04:30 UTC
    // (12:30 PM MYT). Sleeps directly until the next occurrence instead of
    // polling hourly and gating on the wall-clock hour; the next-run
    // computation is pure and unit-tested (core::schedule) with the invariant
    // that the delay is strictly positive, so this loop cannot arm a
    // zero-length sleep and spin.
    //
    // The tick only *paces* the job — it carries no timezone. Each tenant is
    // evaluated on its own calendar and its own 12:30 cutoff inside the
    // service, so a company west of MYT is never marked absent for a date that
    // has not started there yet.
    //
    // Each run is a *catch-up*: it processes every local date since that
    // company's last recorded successful run (bounded), so a deploy or outage
    // spanning the daily window no longer skips that day forever. The startup
    // pass covers the restart-after-downtime case without waiting for the next
    // window.
    let absent_pool = pool.clone();
    let mut absent_shutdown = shutdown_rx.clone();
    let absent_handle = tokio::spawn(async move {
        use payroll_system::core::schedule::next_daily_run_utc;
        use payroll_system::services::attendance_service;

        const RUN_HOUR_UTC: u32 = 4;
        const RUN_MINUTE_UTC: u32 = 30;

        match attendance_service::run_auto_absent_catchup(&absent_pool).await {
            Ok(count) => tracing::info!(marked = count, "auto-absent: startup catch-up completed"),
            Err(e) => tracing::error!("Auto-absent startup catch-up failed: {}", e),
        }

        loop {
            let now = chrono::Utc::now();
            let next = next_daily_run_utc(now, RUN_HOUR_UTC, RUN_MINUTE_UTC);
            // Floor at 60s: even if the schedule arithmetic ever regresses to
            // a non-positive delay, the worst case is one wakeup per minute —
            // visible in logs — never a hot loop.
            let delay = (next - now)
                .to_std()
                .unwrap_or(std::time::Duration::from_secs(60))
                .max(std::time::Duration::from_secs(60));
            tracing::info!(
                next_run_utc = %next,
                sleep_secs = delay.as_secs(),
                "auto-absent: scheduled next run"
            );
            tokio::select! {
                _ = absent_shutdown.changed() => break,
                _ = tokio::time::sleep(delay) => {}
            }

            tracing::info!("auto-absent: tick fired; marking absentees");
            match attendance_service::run_auto_absent_catchup(&absent_pool).await {
                Ok(count) => tracing::info!(marked = count, "auto-absent: completed"),
                Err(e) => tracing::error!("Auto-absent marking failed: {}", e),
            }
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

    // Release the background loops so they stop between ticks instead of
    // dying mid-query when the runtime drops.
    let _ = shutdown_tx.send(true);
    let _ = tokio::join!(cleanup_handle, absent_handle);

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
