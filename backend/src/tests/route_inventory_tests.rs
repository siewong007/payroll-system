//! Exhaustive route-inventory auth sweep (plan item 31).
//!
//! Authorization lives in handler bodies, so a route mounted without a gate is
//! invisible to any per-route test that was never written. This test parses
//! the route table itself (`routes/mod.rs`, via `include_str!`) and fires an
//! UNAUTHENTICATED request at every registered path with each common method.
//! Every response must be an authentication/authorisation rejection — or, for
//! the explicitly public allow-list, a legitimate outcome. A 200 here means a
//! route reached business logic without a gate: exactly how items 15 and 16
//! happened.

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::Request;
use tower::ServiceExt;

use crate::tests::route_auth_tests::app_for;
use crate::tests::support::skip_if_no_db;

/// (method, status) pairs that are acceptable WITHOUT credentials for these
/// exact paths. Anything not listed must answer 401/403/404/405 unauthenticated.
const PUBLIC_ROUTES: &[(&str, &str)] = &[
    ("POST", "/api/auth/login"),
    ("POST", "/api/auth/refresh"),
    ("POST", "/api/auth/forgot-password"),
    ("POST", "/api/auth/reset-password"),
    ("POST", "/api/auth/validate-reset-token"),
    ("POST", "/api/auth/2fa/verify"),
    // Logout without a valid refresh cookie is a no-op by design — it must
    // never leak whether the presented cookie was live, and it always clears
    // whatever cookie the client holds. 200 is its legitimate outcome.
    ("POST", "/api/auth/logout"),
    ("GET", "/api/health"),
    ("GET", "/api/health/live"),
    ("GET", "/api/health/ready"),
    ("GET", "/api/auth/oauth2/providers"),
    // Passkey ceremony starts: they mint WebAuthn challenges and reveal
    // nothing — the pre-auth half of passkey login, like /auth/login itself.
    ("POST", "/api/auth/passkey/discoverable/begin"),
    ("POST", "/api/auth/passkey/authenticate/begin"),
    ("GET", "/api/auth/oauth2/google/authorize"),
    // The callback redirects to the frontend error page unauthenticated —
    // it is the browser-facing end of an OAuth flow, never an API response.
    ("GET", "/api/auth/oauth2/google/callback"),
];

fn registered_paths() -> Vec<String> {
    let source = include_str!("../routes/mod.rs");
    let mut paths = Vec::new();
    let mut rest = source;
    while let Some(pos) = rest.find(".route(") {
        let after = &rest[pos + ".route(".len()..];
        let trimmed = after.trim_start();
        if let Some(open) = trimmed.find('"') {
            let close = trimmed[open + 1..].find('"').map(|i| i + open + 1);
            if let Some(close) = close {
                let path = &trimmed[open + 1..close];
                paths.push(path.to_string());
            }
        }
        rest = &rest[pos + 7..];
    }
    paths.sort();
    paths.dedup();
    paths
}

fn substitute_params(path: &str) -> String {
    // {id}, {run_id}, ... all become a syntactically valid UUID placeholder;
    // the request is rejected before handlers read them anyway.
    let mut out = String::with_capacity(path.len());
    let mut in_param = false;
    for ch in path.chars() {
        match ch {
            '{' => {
                in_param = true;
                out.push('0');
            }
            '}' => in_param = false,
            _ if in_param => out.push('0'),
            c => out.push(c),
        }
    }
    out
}

#[tokio::test]
async fn every_registered_route_rejects_unauthenticated_access() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let app = app_for(pool).await;
    let paths = registered_paths();
    assert!(
        paths.len() > 80,
        "the inventory parse found only {} routes; the extraction broke",
        paths.len()
    );

    let methods = ["GET", "POST", "PUT", "DELETE"];
    let mut checked = 0usize;

    for path in &paths {
        assert!(
            path.starts_with('/'),
            "route literal without leading slash: {path}"
        );
        let concrete = substitute_params(path);
        for method in methods {
            let mut request = Request::builder()
                .method(method)
                .uri(format!("/api{concrete}"))
                .header("x-forwarded-for", "203.0.113.10, 10.0.0.1")
                .header(axum::http::header::USER_AGENT, "RouteInventoryTest/1.0")
                .body(Body::empty())
                .expect("build request");
            request
                .extensions_mut()
                .insert(ConnectInfo(SocketAddr::from(([203, 0, 113, 10], 12345))));

            let response = app.clone().oneshot(request).await.expect("router response");
            let status = response.status();

            let is_public = PUBLIC_ROUTES
                .iter()
                .any(|(m, p)| *m == method && *p == format!("/api{path}"));
            if is_public {
                continue;
            }

            checked += 1;
            // Any 4xx means the request was refused before or inside
            // authorisation — including 400/415/422 from extractors running
            // ahead of the gate (no business logic ran) and 429 from the rate
            // limiter. What must NEVER happen unauthenticated: a 2xx (logic
            // executed), a redirect into the app, or a 5xx (it got far enough
            // to crash).
            assert!(
                !status.is_success() && !status.is_server_error() && !status.is_redirection(),
                "{method} /api{path} answered {status} to an unauthenticated request"
            );
        }
    }

    assert!(checked > 300, "only {checked} checks ran");
}
