//! The OAuth2 `state` is a commitment to a browser-held secret, not a bearer
//! value. These cover both halves: the cookie that carries the secret, and the
//! consume path that refuses a state it cannot match to one.

use axum::http::{HeaderMap, header};

use crate::core::cookie;
use crate::core::error::AppError;
use crate::services::oauth2_service;
use crate::tests::support::skip_if_no_db;

#[test]
fn the_state_is_a_stable_commitment_that_does_not_leak_the_binder() {
    let binder = oauth2_service::generate_state_binder();
    let state = oauth2_service::state_for_binder(&binder);

    assert_eq!(state.len(), 64, "sha256 hex fits oauth2_states.state");
    assert!(state.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(state, oauth2_service::state_for_binder(&binder));

    let other = oauth2_service::generate_state_binder();
    assert_ne!(binder, other, "binders must not repeat");
    assert_ne!(state, oauth2_service::state_for_binder(&other));

    // The value that travels in URLs, logs and Referer must not contain the
    // secret it commits to.
    assert!(!state.contains(&binder));
}

#[test]
fn the_binder_cookie_is_scoped_and_lax() {
    let (_, value) = cookie::set_oauth_state_cookie("b1nd3r", "https://payroll.example");

    assert!(value.contains("oauth2_binder=b1nd3r"));
    assert!(value.contains("HttpOnly"));
    // Lax rather than Strict: the Google callback is a top-level cross-site
    // navigation, and a Strict cookie is simply not sent with it.
    assert!(value.contains("SameSite=Lax"));
    assert!(value.contains("Path=/api/auth/oauth2"));
    // Matches the ten-minute server-side state TTL.
    assert!(value.contains("Max-Age=600"));
    assert!(value.contains("Secure"));

    let (_, insecure) = cookie::set_oauth_state_cookie("b1nd3r", "http://localhost:5173");
    assert!(
        !insecure.contains("Secure"),
        "a Secure cookie is never stored over plain http, which breaks local dev"
    );

    let (_, cleared) = cookie::clear_oauth_state_cookie("https://payroll.example");
    assert!(cleared.contains("Max-Age=0"));
    assert!(cleared.contains("Path=/api/auth/oauth2"));
}

#[test]
fn the_binder_is_read_back_alongside_the_refresh_cookie() {
    let mut headers = HeaderMap::new();
    let cookies = "refresh_token=rt_abc; oauth2_binder=b1nd3r";
    headers.insert(header::COOKIE, cookies.parse().unwrap());

    let binder = cookie::extract_oauth_state_binder(&headers);
    let refresh = cookie::extract_refresh_token(&headers);
    assert_eq!(binder.as_deref(), Some("b1nd3r"));
    assert_eq!(refresh.as_deref(), Some("rt_abc"));

    // A cleared cookie is still sent by some clients; an empty value is absent.
    let mut emptied = HeaderMap::new();
    emptied.insert(header::COOKIE, "oauth2_binder=".parse().unwrap());
    assert!(cookie::extract_oauth_state_binder(&emptied).is_none());

    assert!(cookie::extract_oauth_state_binder(&HeaderMap::new()).is_none());
}

/// The login-CSRF shape: a callback URL carrying a valid state, loaded in a
/// browser that never started the flow. Without the binder it completed and
/// replaced the victim's session with the attacker's account.
#[tokio::test]
async fn a_state_without_its_binder_is_refused_and_the_row_survives() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let binder = oauth2_service::generate_state_binder();
    let state = oauth2_service::state_for_binder(&binder);
    let stored = oauth2_service::store_oauth2_state(&pool, &state, "verifier-abc").await;
    stored.expect("store state");

    let no_cookie = oauth2_service::consume_oauth2_state(&pool, &state, None)
        .await
        .expect_err("a callback with no binder must be refused");
    assert!(matches!(no_cookie, AppError::BadRequest(_)));

    let wrong = oauth2_service::generate_state_binder();
    let mismatched = oauth2_service::consume_oauth2_state(&pool, &state, Some(&wrong))
        .await
        .expect_err("a mismatched binder must be refused");
    assert!(matches!(mismatched, AppError::BadRequest(_)));

    // Neither probe may burn the legitimate user's single-use verifier.
    let verifier = oauth2_service::consume_oauth2_state(&pool, &state, Some(&binder))
        .await
        .expect("the originating browser still completes the flow");
    assert_eq!(verifier, "verifier-abc");

    // Single-use is unchanged by the binding.
    let replayed = oauth2_service::consume_oauth2_state(&pool, &state, Some(&binder)).await;
    assert!(replayed.is_err());
}
