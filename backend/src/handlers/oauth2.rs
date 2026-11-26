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
use crate::core::extract::ValidatedJson;
use crate::models::audit::AuditRequestMeta;
use crate::models::oauth2::{
    LinkGoogleRequest, LinkedAccount, OAuth2AuthorizeRequest, OAuth2CallbackQuery,
    OAuth2ProviderInfo, UnlinkOAuth2Request,
};
use crate::models::session::LoginOutcome;
use crate::services::{audit_service, auth_service, oauth2_service, totp_service};

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
    audit_meta: AuditRequestMeta,
    Query(query): Query<OAuth2CallbackQuery>,
) -> Response {
    let frontend_url = state.config.frontend_url.clone();

    match google_callback_inner(state, headers, audit_meta, query).await {
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
    audit_meta: AuditRequestMeta,
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
    // The callback is an Axum route, so the URI is built from the API's own
    // public origin — on split-origin deploys the frontend host is a static
    // site that would swallow the redirect.
    let redirect_uri = state.config.google_redirect_uri();

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

    // Only pre-linked Google identities may sign in here. Auto-linking by
    // verified email on login would trust Google email claims before local
    // authentication; users must link Google explicitly while signed in.
    let user = match oauth2_service::find_user_by_oauth2(&state.pool, "google", &google_user.sub)
        .await?
    {
        Some(user) => user,
        None => {
            if oauth2_service::find_oauth2_account(&state.pool, "google", &google_user.sub)
                .await?
                .is_some()
            {
                return Err(AppError::Unauthorized(
                    "Account is disabled or unavailable.".into(),
                ));
            }
            return Err(AppError::Unauthorized(
                "No payroll account is linked to this Google identity. Sign in with your password and link Google in Account Security settings."
                    .into(),
            ));
        }
    };

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
        "google",
        Some(&audit_meta),
    )
    .await?;

    if matches!(outcome, LoginOutcome::Session(_))
        && let Err(err) = oauth2_service::update_oauth2_tokens(
            &state.pool,
            "google",
            &google_user.sub,
            &token_resp.access_token,
            token_resp.refresh_token.as_deref(),
            token_resp.expires_in,
        )
        .await
    {
        if let LoginOutcome::Session(ref session) = outcome
            && let Ok(claims) =
                crate::core::auth::verify_token(&session.token, &state.config.jwt_secret)
        {
            let _ =
                crate::services::session_service::revoke_session(&state.pool, user.id, claims.sid)
                    .await;
        }
        return Err(err);
    }

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
            // Access token and user payload never travel in the redirect URL;
            // the SPA exchanges the httpOnly refresh cookie via `/auth/refresh`.
            headers.append(name, value.parse().unwrap());

            format!("{}/oauth2/callback#status=ok", state.config.frontend_url)
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
pub async fn google_authorize(
    State(state): State<AppState>,
    audit_meta: AuditRequestMeta,
    Json(body): Json<OAuth2AuthorizeRequest>,
) -> AppResult<Response> {
    if body.flow.as_deref() == Some("link") {
        crate::core::turnstile::verify(
            &state.config,
            body.turnstile_token.as_deref(),
            audit_meta.ip_address.as_deref(),
            "oauth2_google",
        )
        .await?;
    } else {
        crate::core::turnstile::verify_any_action(
            &state.config,
            body.turnstile_token.as_deref(),
            audit_meta.ip_address.as_deref(),
            &["login", "oauth2_google"],
        )
        .await?;
    }

    let client_id = state
        .config
        .google_client_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Google OAuth2 is not configured".into()))?;

    // `flow=link` is the account-linking journey: the code comes back to a SPA
    // route that POSTs it with the user's JWT, not to the login callback.
    let redirect_uri = if body.flow.as_deref() == Some("link") {
        state.config.google_link_redirect_uri()
    } else {
        state.config.google_redirect_uri()
    };

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
    audit_meta: AuditRequestMeta,
    Path(provider): Path<String>,
    ValidatedJson(body): ValidatedJson<UnlinkOAuth2Request>,
) -> AppResult<Json<serde_json::Value>> {
    auth_service::verify_user_password(&state.pool, auth.0.sub, &body.password, Some(&audit_meta))
        .await?;
    totp_service::verify_step_up_if_enabled(
        &state.pool,
        auth.0.sub,
        body.totp_code.as_deref(),
        &state.config.totp_encryption_key,
        Some(&audit_meta),
    )
    .await?;
    oauth2_service::unlink_oauth2_account(&state.pool, auth.0.sub, &provider).await?;
    let _ = audit_service::log_action_with_metadata(
        &state.pool,
        auth.0.company_id,
        Some(auth.0.sub),
        "oauth2_unlinked",
        "auth",
        Some(auth.0.sub),
        None::<serde_json::Value>,
        Some(serde_json::json!({ "provider": provider })),
        Some("OAuth2 provider unlinked"),
        Some(&audit_meta),
    )
    .await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Link an OAuth2 provider to the current user via authorization code.
pub async fn link_google(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    audit_meta: AuditRequestMeta,
    ValidatedJson(body): ValidatedJson<LinkGoogleRequest>,
) -> AppResult<Response> {
    let mut clear_headers = HeaderMap::new();
    let (name, value) = cookie::clear_oauth_state_cookie(&state.config.frontend_url);
    clear_headers.insert(name, value.parse().unwrap());

    if let Err(err) = auth_service::verify_user_password(
        &state.pool,
        auth.0.sub,
        &body.password,
        Some(&audit_meta),
    )
    .await
    {
        return Ok((clear_headers, err.into_response()).into_response());
    }
    if let Err(err) = totp_service::verify_step_up_if_enabled(
        &state.pool,
        auth.0.sub,
        body.totp_code.as_deref(),
        &state.config.totp_encryption_key,
        Some(&audit_meta),
    )
    .await
    {
        return Ok((clear_headers, err.into_response()).into_response());
    }

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

    // The state goes through the same binder check as the login callback:
    // without it, anyone holding a Google code could attach their own account
    // to a victim's session. The cookie path covers /api/auth/oauth2, so this
    // POST carries the binder just like the callback does.
    let binder = cookie::extract_oauth_state_binder(&headers);
    let code_verifier =
        match oauth2_service::consume_oauth2_state(&state.pool, &body.state, binder.as_deref())
            .await
        {
            Ok(v) => v,
            Err(err) => return Ok((clear_headers, err.into_response()).into_response()),
        };

    let result = complete_link_google(
        &state,
        &auth,
        &audit_meta,
        &body.code,
        &code_verifier,
        client_id,
        client_secret,
    )
    .await;

    match result {
        Ok(linked) => Ok((clear_headers, Json(linked)).into_response()),
        Err(err) => Ok((clear_headers, err.into_response()).into_response()),
    }
}

async fn complete_link_google(
    state: &AppState,
    auth: &AuthUser,
    audit_meta: &AuditRequestMeta,
    code: &str,
    code_verifier: &str,
    client_id: &str,
    client_secret: &str,
) -> AppResult<crate::models::oauth2::LinkedAccount> {
    // Must equal the redirect_uri the authorization URL was built with — the
    // SPA route that handed us this code, not the login callback.
    let token_resp = oauth2_service::google_exchange_code(
        client_id,
        client_secret,
        &state.config.google_link_redirect_uri(),
        code,
        code_verifier,
    )
    .await?;

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

    let current_links = oauth2_service::list_linked_accounts(&state.pool, auth.0.sub).await?;
    if current_links.iter().any(|a| a.provider == "google")
        && oauth2_service::find_oauth2_account(&state.pool, "google", &google_user.sub)
            .await?
            .is_none()
    {
        return Err(AppError::Conflict(
            "A Google account is already linked to your profile".into(),
        ));
    }

    let account = oauth2_service::link_oauth2_account(
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

    let linked = crate::models::oauth2::LinkedAccount {
        id: account.id,
        provider: account.provider,
        provider_email: account.provider_email,
        provider_name: account.provider_name,
        avatar_url: account.avatar_url,
        linked_at: account.created_at,
    };

    let _ = audit_service::log_action_with_metadata(
        &state.pool,
        auth.0.company_id,
        Some(auth.0.sub),
        "oauth2_linked",
        "auth",
        Some(auth.0.sub),
        None::<serde_json::Value>,
        Some(serde_json::json!({ "provider": "google", "auto_linked": false })),
        Some("Google account linked by user"),
        Some(audit_meta),
    )
    .await;

    Ok(linked)
}
