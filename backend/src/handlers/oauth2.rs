use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
};

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::cookie;
use crate::core::error::{AppError, AppResult};
use crate::models::oauth2::{LinkedAccount, OAuth2CallbackQuery, OAuth2ProviderInfo};
use crate::models::session::LoginOutcome;
use crate::services::{auth_service, oauth2_service};

/// List available OAuth2 providers and their status.
pub async fn list_providers(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<OAuth2ProviderInfo>>> {
    let providers = vec![OAuth2ProviderInfo {
        provider: "google".to_string(),
        enabled: state.config.google_oauth_enabled(),
        authorize_url: None, // Clients must call /authorize to get a URL with proper PKCE + state
    }];

    Ok(Json(providers))
}

/// Send the browser back into the SPA with a message for its error card.
///
/// The message travels in the fragment, not the query string, so it never
/// reaches a server log or `Referer` header on the way. The state binder is
/// burned here too: a failed attempt must not leave a usable binder behind for
/// the remainder of its ten-minute TTL.
fn oauth2_error_redirect(frontend_url: &str, message: &str) -> Response {
    let mut headers = HeaderMap::new();
    let (name, value) = cookie::clear_oauth_state_cookie(frontend_url);
    headers.insert(name, value.parse().unwrap());

    (
        headers,
        Redirect::temporary(&format!(
            "{}/oauth2/callback#error={}",
            frontend_url,
            urlencoding::encode(message),
        )),
    )
        .into_response()
}

/// Google OAuth2 callback — validates state/PKCE, exchanges code, finds/links user, redirects.
///
/// This endpoint is loaded by the user's browser, not by our own client, so it
/// always answers with a redirect: returning an `AppError` here rendered a raw
/// JSON body in the address bar instead of the login screen.
pub async fn google_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<OAuth2CallbackQuery>,
) -> Response {
    let frontend_url = state.config.frontend_url.clone();

    match google_callback_inner(state, headers, query).await {
        Ok(response) => response,
        Err(err) => {
            // `client_response` logs the detail of Internal/Database errors and
            // hands back only what is safe to show.
            let (_, message) = err.client_response();
            oauth2_error_redirect(&frontend_url, &message)
        }
    }
}

async fn google_callback_inner(
    state: AppState,
    headers: HeaderMap,
    query: OAuth2CallbackQuery,
) -> AppResult<Response> {
    // Google reports a declined consent screen by redirecting back with `error`
    // and no code; say so plainly rather than complaining about a missing code.
    if let Some(provider_error) = query.error.as_deref() {
        return Err(match provider_error {
            "access_denied" => {
                AppError::BadRequest("Google sign-in was cancelled. Please try again.".into())
            }
            other => {
                tracing::warn!("Google returned an authorization error: {}", other);
                AppError::BadRequest(
                    "Google could not complete the sign-in. Please try again.".into(),
                )
            }
        });
    }

    // Validate state parameter (CSRF protection). The state alone proves only
    // that *someone* started a flow; the binder cookie proves it was this
    // browser, which is what stops a callback URL from being replayed into a
    // victim's session.
    let oauth_state = query
        .state
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Missing OAuth2 state parameter".into()))?;
    let binder = cookie::extract_oauth_state_binder(&headers);

    let code_verifier =
        oauth2_service::consume_oauth2_state(&state.pool, oauth_state, binder.as_deref()).await?;

    let client_id = state
        .config
        .google_client_id
        .as_deref()
        .ok_or_else(|| AppError::Internal("Google OAuth2 not configured".into()))?;
    let client_secret = state
        .config
        .google_client_secret
        .as_deref()
        .ok_or_else(|| AppError::Internal("Google OAuth2 not configured".into()))?;
    let redirect_uri = format!(
        "{}/api/auth/oauth2/google/callback",
        state.config.frontend_url
    );

    // Exchange code for tokens with PKCE code_verifier
    let token_resp = oauth2_service::google_exchange_code(
        client_id,
        client_secret,
        &redirect_uri,
        query
            .code
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("Missing OAuth2 code parameter".into()))?,
        &code_verifier,
    )
    .await?;

    // Get user info from Google
    let google_user = oauth2_service::google_user_info(&token_resp.access_token).await?;

    // Verify email is present and verified
    let google_email = google_user.email.as_deref().unwrap_or_default();
    if google_email.is_empty() {
        return Err(AppError::BadRequest(
            "Google account does not have an email".into(),
        ));
    }

    if google_user.email_verified != Some(true) {
        return Err(AppError::BadRequest(
            "Google account email is not verified. Please verify your email with Google first."
                .into(),
        ));
    }

    // Try to find user by linked OAuth2 account first
    let user =
        match oauth2_service::find_user_by_oauth2(&state.pool, "google", &google_user.sub).await? {
            Some(user) => user,
            None => {
                // No linked account — auto-link by verified email match
                let user = oauth2_service::find_user_by_email(&state.pool, google_email)
                    .await?
                    .ok_or_else(|| {
                        AppError::Unauthorized(
                            "No account found for this email. Please contact your administrator."
                                .into(),
                        )
                    })?;

                // Auto-link the Google account to this user
                oauth2_service::link_oauth2_account(
                    &state.pool,
                    user.id,
                    "google",
                    &google_user.sub,
                    google_user.email.as_deref(),
                    google_user.name.as_deref(),
                    google_user.picture.as_deref(),
                    Some(&token_resp.access_token),
                    token_resp.refresh_token.as_deref(),
                    token_resp.expires_in,
                )
                .await?;

                user
            }
        };

    // Update stored Google tokens
    oauth2_service::update_oauth2_tokens(
        &state.pool,
        "google",
        &google_user.sub,
        &token_resp.access_token,
        token_resp.refresh_token.as_deref(),
        token_resp.expires_in,
    )
    .await?;

    // Issue tokens (gated on 2FA if enabled) — this also records last_login,
    // deferred until 2FA is actually completed if it's required.
    let outcome = auth_service::complete_login(
        &state.pool,
        user.id,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
    )
    .await?;

    let mut headers = HeaderMap::new();
    // The flow is over either way, so the binder is spent.
    let (name, value) = cookie::clear_oauth_state_cookie(&state.config.frontend_url);
    headers.insert(name, value.parse().unwrap());

    let redirect_url = match outcome {
        LoginOutcome::Session(session) => {
            let (name, value) =
                cookie::set_refresh_cookie(&session.refresh_token, &state.config.frontend_url);
            // `append`, not `insert`: the binder-clearing Set-Cookie above is
            // the same header name and `insert` would silently drop it.
            headers.append(name, value.parse().unwrap());

            format!(
                "{}/oauth2/callback#token={}&user={}",
                state.config.frontend_url,
                urlencoding::encode(&session.token),
                urlencoding::encode(&serde_json::to_string(&session.user).unwrap_or_default()),
            )
        }
        LoginOutcome::MfaRequired { mfa_token } => format!(
            "{}/oauth2/callback#mfa_token={}",
            state.config.frontend_url,
            urlencoding::encode(&mfa_token),
        ),
    };

    Ok((headers, Redirect::temporary(&redirect_url)).into_response())
}

/// Initiate Google OAuth2 flow — generates PKCE + state, returns the authorization URL.
///
/// The response also sets the state binder cookie. The SPA calls this endpoint
/// with `withCredentials`, so the cookie is stored and replayed on the callback
/// navigation without any frontend change.
pub async fn google_authorize(State(state): State<AppState>) -> AppResult<Response> {
    let client_id = state
        .config
        .google_client_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Google OAuth2 is not configured".into()))?;

    let redirect_uri = format!(
        "{}/api/auth/oauth2/google/callback",
        state.config.frontend_url
    );

    // The state published to Google is a hash of the binder; the binder itself
    // stays in the browser.
    let binder = oauth2_service::generate_state_binder();
    let oauth_state = oauth2_service::state_for_binder(&binder);

    // Generate PKCE code verifier and challenge
    let code_verifier = oauth2_service::generate_code_verifier();
    let code_challenge = oauth2_service::compute_code_challenge(&code_verifier);

    // Store state + code_verifier in DB (single-use, expires in 10 minutes)
    oauth2_service::store_oauth2_state(&state.pool, &oauth_state, &code_verifier).await?;

    let url = oauth2_service::google_authorize_url(
        client_id,
        &redirect_uri,
        &oauth_state,
        &code_challenge,
    );

    let mut headers = HeaderMap::new();
    let (name, value) = cookie::set_oauth_state_cookie(&binder, &state.config.frontend_url);
    headers.insert(name, value.parse().unwrap());

    Ok((
        headers,
        Json(serde_json::json!({
            "authorize_url": url,
        })),
    )
        .into_response())
}

/// List OAuth2 accounts linked to the current user.
pub async fn my_linked_accounts(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<LinkedAccount>>> {
    let accounts = oauth2_service::list_linked_accounts(&state.pool, auth.0.sub).await?;
    Ok(Json(accounts))
}

/// Unlink an OAuth2 provider from the current user.
pub async fn unlink_provider(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(provider): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    oauth2_service::unlink_oauth2_account(&state.pool, auth.0.sub, &provider).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Link an OAuth2 provider to the current user via authorization code.
pub async fn link_google(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<LinkedAccount>> {
    let code = body
        .get("code")
        .and_then(|c| c.as_str())
        .ok_or_else(|| AppError::BadRequest("Authorization code is required".into()))?;

    let client_id = state
        .config
        .google_client_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Google OAuth2 is not configured".into()))?;
    let client_secret = state
        .config
        .google_client_secret
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Google OAuth2 is not configured".into()))?;
    let redirect_uri = format!(
        "{}/api/auth/oauth2/google/callback",
        state.config.frontend_url
    );

    // For account linking, PKCE state is optional (the code comes from a popup/redirect
    // the frontend manages). Exchange without PKCE code_verifier for the linking flow.
    // When a state *is* supplied it goes through the same binder check as the
    // login callback — linking mints no session, but leaving a second unbound
    // consume path is how the bypass creeps back.
    let code_verifier = match body.get("state").and_then(|s| s.as_str()) {
        Some(st) => {
            let binder = cookie::extract_oauth_state_binder(&headers);
            oauth2_service::consume_oauth2_state(&state.pool, st, binder.as_deref()).await?
        }
        None => String::new(),
    };

    let token_resp = if code_verifier.is_empty() {
        // Linking flow without PKCE (backward compatible)
        oauth2_service::google_exchange_code(client_id, client_secret, &redirect_uri, code, "")
            .await?
    } else {
        oauth2_service::google_exchange_code(
            client_id,
            client_secret,
            &redirect_uri,
            code,
            &code_verifier,
        )
        .await?
    };

    let google_user = oauth2_service::google_user_info(&token_resp.access_token).await?;

    // Verify email is verified
    if google_user.email_verified != Some(true) {
        return Err(AppError::BadRequest(
            "Google account email is not verified".into(),
        ));
    }

    // Check if already linked to another user
    if let Some(existing) =
        oauth2_service::find_oauth2_account(&state.pool, "google", &google_user.sub).await?
        && existing.user_id != auth.0.sub
    {
        return Err(AppError::BadRequest(
            "This Google account is already linked to another user".into(),
        ));
    }

    oauth2_service::link_oauth2_account(
        &state.pool,
        auth.0.sub,
        "google",
        &google_user.sub,
        google_user.email.as_deref(),
        google_user.name.as_deref(),
        google_user.picture.as_deref(),
        Some(&token_resp.access_token),
        token_resp.refresh_token.as_deref(),
        token_resp.expires_in,
    )
    .await?;

    let accounts = oauth2_service::list_linked_accounts(&state.pool, auth.0.sub).await?;
    let linked = accounts
        .into_iter()
        .find(|a| a.provider == "google")
        .ok_or_else(|| AppError::Internal("Failed to fetch linked account".into()))?;

    Ok(Json(linked))
}
