//! Read-models joining `users` with `passkey_credentials`.
//!
//! NOTE: query indentation matches the byte-exact SQL in the offline `.sqlx` cache.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;

/// The id of an active user who has at least one passkey, by email.
pub async fn user_id_by_email_with_passkeys(
    executor: impl Executor<'_, Database = Postgres>,
    email: &str,
) -> AppResult<Option<Uuid>> {
    let user_id = sqlx::query_scalar!(
        r#"SELECT u.id FROM users u
        INNER JOIN passkey_credentials pc ON pc.user_id = u.id
        WHERE lower(btrim(u.email)) = lower(btrim($1)) AND u.is_active = TRUE
        LIMIT 1"#,
        email,
    )
    .fetch_optional(executor)
    .await?;
    Ok(user_id)
}
