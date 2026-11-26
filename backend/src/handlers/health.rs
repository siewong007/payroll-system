use std::net::SocketAddr;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use axum::{
    Extension, Json,
    extract::{ConnectInfo, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use tokio::sync::RwLock;

use crate::core::app_state::AppState;
use crate::core::config::{BUILD_REVISION, BUILD_VERSION};
use crate::core::extract::ClientIp;
use crate::services::job_health_service;

const READINESS_CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Clone)]
struct CachedReadiness {
    database_url: String,
    fetched_at: Instant,
    status_code: StatusCode,
    detailed: Value,
    public: Value,
}

static READINESS_CACHE: OnceLock<RwLock<Option<CachedReadiness>>> = OnceLock::new();

fn readiness_cache() -> &'static RwLock<Option<CachedReadiness>> {
    READINESS_CACHE.get_or_init(|| RwLock::new(None))
}

fn is_internal_loopback(
    resolved_ip: Option<std::net::IpAddr>,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
) -> bool {
    let Some(Extension(ConnectInfo(peer))) = connect_info else {
        return false;
    };
    if !peer.ip().is_loopback() {
        return false;
    }
    resolved_ip.is_none_or(|ip| ip.is_loopback())
}

/// Deep health check for orchestration probes, host watchdog timers, and deploy
/// verification. Verifies Postgres connectivity + applied migrations, surfaces
/// the running `BUILD_REVISION`, and reports sanitized background job staleness
/// to local loopback callers while returning only `{ "status": ... }` to external requests.
pub async fn ready(
    State(state): State<AppState>,
    ClientIp(resolved_ip): ClientIp,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
) -> (StatusCode, Json<Value>) {
    let internal = is_internal_loopback(resolved_ip, connect_info);

    {
        let guard = readiness_cache().read().await;
        if let Some(cached) = guard.as_ref()
            && cached.database_url == state.config.database_url
            && cached.fetched_at.elapsed() < READINESS_CACHE_TTL
        {
            return (
                cached.status_code,
                Json(if internal {
                    cached.detailed.clone()
                } else {
                    cached.public.clone()
                }),
            );
        }
    }

    let row = crate::repositories::reads::system::get_migration_status(&state.pool).await;
    let (status_code, detailed, public) = match row {
        Ok((Some(_latest), _count)) => {
            let jobs = job_health_service::read_public_job_health(&state.pool)
                .await
                .ok();
            let status = if jobs.as_ref().is_some_and(|j| j.any_unhealthy()) {
                "degraded"
            } else {
                "ready"
            };
            (
                StatusCode::OK,
                json!({
                    "status": status,
                    "version": BUILD_VERSION,
                    "revision": BUILD_REVISION,
                    "database": "ok",
                    "checks": {
                        "database": "ok",
                        "config": "ok"
                    },
                    "jobs": jobs,
                }),
                json!({ "status": status }),
            )
        }
        Ok((None, _)) => {
            tracing::error!("Readiness check failed: no migrations applied");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                json!({
                    "status": "unready",
                    "version": BUILD_VERSION,
                    "revision": BUILD_REVISION,
                    "database": "unavailable",
                }),
                json!({ "status": "unready" }),
            )
        }
        Err(e) => {
            tracing::error!("Readiness check failed: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                json!({
                    "status": "unready",
                    "version": BUILD_VERSION,
                    "revision": BUILD_REVISION,
                    "database": "unavailable",
                }),
                json!({ "status": "unready" }),
            )
        }
    };

    if status_code == StatusCode::OK {
        let mut guard = readiness_cache().write().await;
        *guard = Some(CachedReadiness {
            database_url: state.config.database_url.clone(),
            fetched_at: Instant::now(),
            status_code,
            detailed: detailed.clone(),
            public: public.clone(),
        });
    }

    (status_code, Json(if internal { detailed } else { public }))
}
