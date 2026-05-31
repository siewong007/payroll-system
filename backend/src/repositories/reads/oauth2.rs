//! Read-models joining `users` with `oauth2_accounts`.
//!
//! NOTE: query indentation matches the byte-exact SQL in the offline `.sqlx` cache.

use sqlx::{Executor, Postgres};

use crate::core::error::AppResult;
use crate::models::user::User;

/// The active user linked to a given OAuth2 identity, if any.
pub async fn find_user_by_oauth2(
    executor: impl Executor<'_, Database = Postgres>,
    provider: &str,
    provider_user_id: &str,
) -> AppResult<Option<User>> {
    let user = sqlx::query_as!(
        User,
        r#"SELECT u.id, u.email, u.password_hash, u.full_name, u.roles, u.company_id,
            u.employee_id, u.is_active, u.must_change_password, u.last_login, u.created_at, u.updated_at
        FROM users u
        JOIN oauth2_accounts oa ON u.id = oa.user_id
        WHERE oa.provider = $1 AND oa.provider_user_id = $2 AND u.is_active = TRUE"#,
        provider,
        provider_user_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(user)
}
