use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::repositories::{refresh_tokens, user_sessions};

const REFRESH_TOKEN_DAYS: i64 = 30;

/// How long after a rotation a superseded token may still be presented without
/// being read as a replay.
///
/// Load-bearing, not a softening of the reuse check: two admin tabs share one
/// refresh cookie and `api/client.ts` coalesces refreshes only *within* a tab,
/// so whenever two tabs' access tokens expire together one of them presents the
/// token the other just rotated away. Killing the family there would sign the
/// user out everywhere on an ordinary Monday morning. Ten seconds is far below
/// any realistic exfiltrate-and-replay, and the losing tab still gets a 401 and
/// recovers on its next request with the new cookie.
const ROTATION_GRACE_SECONDS: f64 = 10.0;

/// The single message every refresh failure returns. Which of the branches
/// below produced it is a server-side detail; telling the caller apart would
/// tell an attacker whether a token was ever valid.
const INVALID_REFRESH_MSG: &str = "Invalid or expired refresh token";

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Creates a new refresh token for a user, returns the raw token string.
pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    user_agent: Option<&str>,
) -> AppResult<(Uuid, String)> {
    let session_id = Uuid::now_v7();
    let raw_token = create_refresh_token(pool, user_id, session_id, user_agent).await?;
    Ok((session_id, raw_token))
}

pub async fn create_refresh_token(
    pool: &PgPool,
    user_id: Uuid,
    session_id: Uuid,
    user_agent: Option<&str>,
) -> AppResult<String> {
    let raw_token = format!("rt_{}_{}", Uuid::new_v4(), Uuid::new_v4());
    let token_hash = hash_token(&raw_token);
    let expires_at = Utc::now() + Duration::days(REFRESH_TOKEN_DAYS);

    user_sessions::insert(pool, session_id, user_id, user_agent, expires_at).await?;
    refresh_tokens::insert(pool, user_id, session_id, &token_hash, expires_at).await?;

    Ok(raw_token)
}

/// Validates a refresh token and returns the user_id if valid.
///
/// Read-only, so it is safe for logout, which only needs to know which session
/// to tear down. Rotation must not use it — see [`consume_for_rotation`].
pub async fn verify_refresh_token(pool: &PgPool, raw_token: &str) -> AppResult<(Uuid, Uuid)> {
    let token_hash = hash_token(raw_token);

    refresh_tokens::find_active(pool, &token_hash)
        .await?
        .map(|token| (token.user_id, token.session_id))
        .ok_or_else(|| AppError::Unauthorized(INVALID_REFRESH_MSG.into()))
}

/// Retire `raw_token` inside the caller's transaction and return its owner.
///
/// `None` means the token was not live; the caller must roll back and hand the
/// token to [`classify_rotation_miss`] rather than reporting a bare 401, since
/// the same miss covers both a harmless racing tab and a replayed credential.
pub async fn consume_for_rotation(
    conn: &mut PgConnection,
    raw_token: &str,
) -> AppResult<Option<refresh_tokens::ActiveRefreshToken>> {
    refresh_tokens::consume_active(conn, &hash_token(raw_token)).await
}

/// Mint the successor token and extend the session, in the caller's transaction.
pub async fn issue_rotated(
    conn: &mut PgConnection,
    user_id: Uuid,
    session_id: Uuid,
) -> AppResult<String> {
    let raw_token = format!("rt_{}_{}", Uuid::new_v4(), Uuid::new_v4());
    let token_hash = hash_token(&raw_token);
    let expires_at = Utc::now() + Duration::days(REFRESH_TOKEN_DAYS);

    refresh_tokens::insert(&mut *conn, user_id, session_id, &token_hash, expires_at).await?;
    user_sessions::touch(conn, session_id, expires_at).await?;
    Ok(raw_token)
}

/// Decide what a failed rotation means, and act on it.
///
/// Three cases hide behind one 401:
/// - the hash was never issued here — nothing to protect;
/// - it was rotated away moments ago and a live successor exists — a second tab
///   lost the race, so the session must survive;
/// - it was rotated away long enough ago that no honest client could still hold
///   it — treat it as stolen and revoke the whole family, which is the only
///   response that helps the real user.
///
/// Returns the error to surface; it never fails, because a classification
/// problem must not turn a 401 into a 500.
pub async fn classify_rotation_miss(pool: &PgPool, raw_token: &str) -> AppError {
    let unauthorized = || AppError::Unauthorized(INVALID_REFRESH_MSG.into());
    let token_hash = hash_token(raw_token);

    let owner = match refresh_tokens::find_owner_of_hash(pool, &token_hash).await {
        Ok(Some(owner)) => owner,
        Ok(None) => return unauthorized(),
        Err(e) => return e,
    };

    match refresh_tokens::has_fresh_sibling(pool, owner.session_id, ROTATION_GRACE_SECONDS).await {
        Ok(true) => return unauthorized(),
        Ok(false) => {}
        Err(e) => return e,
    }

    tracing::warn!(
        user_id = %owner.user_id,
        session_id = %owner.session_id,
        "Refresh token reuse detected — revoking the whole session family"
    );
    if let Err(e) = revoke_session_family(pool, owner.user_id, owner.session_id).await {
        return e;
    }

    unauthorized()
}

/// Revoke a session and every refresh token that belongs to it, together.
async fn revoke_session_family(pool: &PgPool, user_id: Uuid, session_id: Uuid) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    user_sessions::revoke(&mut *tx, user_id, session_id).await?;
    refresh_tokens::revoke_for_session(&mut *tx, session_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn list_sessions(
    pool: &PgPool,
    user_id: Uuid,
) -> AppResult<Vec<crate::models::session::UserSession>> {
    user_sessions::list_active(pool, user_id).await
}

pub async fn revoke_session(pool: &PgPool, user_id: Uuid, session_id: Uuid) -> AppResult<bool> {
    if !user_sessions::revoke(pool, user_id, session_id).await? {
        return Ok(false);
    }
    refresh_tokens::revoke_for_session(pool, session_id).await?;
    Ok(true)
}

pub async fn revoke_other_sessions(
    pool: &PgPool,
    user_id: Uuid,
    current_session_id: Uuid,
) -> AppResult<u64> {
    let count = user_sessions::revoke_others(pool, user_id, current_session_id).await?;
    // The session state is the immediate JWT revocation source; stale refresh
    // rows are also revoked so they cannot be exchanged later.
    refresh_tokens::revoke_for_revoked_sessions(pool, user_id, current_session_id).await?;
    Ok(count)
}

/// Revokes a specific refresh token. The retired count is deliberately dropped:
/// the only caller is the logout fallback, where an already-dead token is the
/// desired end state either way.
pub async fn revoke_refresh_token(pool: &PgPool, raw_token: &str) -> AppResult<()> {
    let token_hash = hash_token(raw_token);
    refresh_tokens::revoke_by_hash(pool, &token_hash).await?;
    Ok(())
}
