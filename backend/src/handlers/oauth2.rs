use axum::{
    extract::{Path, Query, State},
    response::Redirect,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::{create_token, AuthUser};
use crate::core::error::{AppError, AppResult};
use crate::models::oauth2::{LinkedAccount, OAuth2ProviderInfo};
use crate::models::user::UserResponse;
use crate::services::{oauth2_service, session_service};

/// List available OAuth2 providers and their status.
pub async fn list_providers(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<OAuth2ProviderInfo>>> {
    let redirect_uri = format!("{}/api/auth/oauth2/google/callback", state.config.frontend_url);

    let providers = vec![OAuth2ProviderInfo {
        provider: "google".to_string(),
        enabled: state.config.google_oauth_enabled(),
        authorize_url: if state.config.google_oauth_enabled() {
            Some(oauth2_service::google_authorize_url(
                state.config.google_client_id.as_deref().unwrap_or_default(),
                &redirect_uri,
                &Uuid::new_v4().to_string(),
            ))
        } else {
            None
        },
    }];

    Ok(Json(providers))
}

#[derive(Deserialize)]
pub struct OAuth2CallbackQuery {
    pub code: String,
    pub state: Option<String>,
}

/// Google OAuth2 callback — exchanges code for tokens, finds/links user, redirects to frontend.
pub async fn google_callback(
    State(state): State<AppState>,
    Query(query): Query<OAuth2CallbackQuery>,
) -> Result<Redirect, AppError> {
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
    let redirect_uri = format!("{}/api/auth/oauth2/google/callback", state.config.frontend_url);

    // Exchange code for tokens
    let token_resp =
        oauth2_service::google_exchange_code(client_id, client_secret, &redirect_uri, &query.code)
            .await?;

    // Get user info from Google
    let google_user = oauth2_service::google_user_info(&token_resp.access_token).await?;

    let google_email = google_user.email.as_deref().unwrap_or_default();
    if google_email.is_empty() {
        return Err(AppError::BadRequest(
            "Google account does not have an email".into(),
        ));
    }

    // Check if this Google account is already linked
    let user = if let Some(user) =
        oauth2_service::find_user_by_oauth2(&state.pool, "google", &google_user.sub).await?
    {
        user
    } else {
        // Try to match by email
        let matched_user = oauth2_service::find_user_by_email(&state.pool, google_email)
            .await?
            .ok_or_else(|| {
                AppError::Unauthorized(
                    "No account found for this email. Please contact your administrator.".into(),
                )
            })?;

        // Auto-link the Google account to the matched user
        oauth2_service::link_oauth2_account(
            &state.pool,
            matched_user.id,
            "google",
            &google_user.sub,
            google_user.email.as_deref(),
            google_user.name.as_deref(),
            google_user.picture.as_deref(),
        )
        .await?;

        matched_user
    };

    // Update last login
    sqlx::query("UPDATE users SET last_login = NOW() WHERE id = $1")
        .bind(user.id)
        .execute(&state.pool)
        .await?;

    // Issue JWT + refresh token
    let jwt = create_token(
        user.id,
        &user.email,
        &user.role,
        user.company_id,
        user.employee_id,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
    )?;

    let refresh_token = session_service::create_refresh_token(&state.pool, user.id).await?;

    // Redirect to frontend with token in URL fragment (not query param for security)
    let redirect_url = format!(
        "{}/oauth2/callback#token={}&refresh_token={}&user={}",
        state.config.frontend_url,
        urlencoding::encode(&jwt),
        urlencoding::encode(&refresh_token),
        urlencoding::encode(&serde_json::to_string(&UserResponse::from(user)).unwrap_or_default()),
    );

    Ok(Redirect::temporary(&redirect_url))
}

/// Initiate Google OAuth2 flow — returns the authorization URL.
pub async fn google_authorize(
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let client_id = state
        .config
        .google_client_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Google OAuth2 is not configured".into()))?;

    let redirect_uri = format!("{}/api/auth/oauth2/google/callback", state.config.frontend_url);
    let oauth_state = Uuid::new_v4().to_string();

    let url = oauth2_service::google_authorize_url(client_id, &redirect_uri, &oauth_state);

    Ok(Json(serde_json::json!({
        "authorize_url": url,
        "state": oauth_state,
    })))
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
    let redirect_uri = format!("{}/api/auth/oauth2/google/callback", state.config.frontend_url);

    let token_resp =
        oauth2_service::google_exchange_code(client_id, client_secret, &redirect_uri, code).await?;
    let google_user = oauth2_service::google_user_info(&token_resp.access_token).await?;

    // Check if already linked to another user
    if let Some(existing) =
        oauth2_service::find_oauth2_account(&state.pool, "google", &google_user.sub).await?
    {
        if existing.user_id != auth.0.sub {
            return Err(AppError::BadRequest(
                "This Google account is already linked to another user".into(),
            ));
        }
    }

    oauth2_service::link_oauth2_account(
        &state.pool,
        auth.0.sub,
        "google",
        &google_user.sub,
        google_user.email.as_deref(),
        google_user.name.as_deref(),
        google_user.picture.as_deref(),
    )
    .await?;

    let accounts = oauth2_service::list_linked_accounts(&state.pool, auth.0.sub).await?;
    let linked = accounts
        .into_iter()
        .find(|a| a.provider == "google")
        .ok_or_else(|| AppError::Internal("Failed to fetch linked account".into()))?;

    Ok(Json(linked))
}
