//! Data access for the `teams` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::team::Team;

pub async fn get(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    team_id: Uuid,
) -> AppResult<Option<Team>> {
    let team = sqlx::query_as!(
        Team,
        "SELECT * FROM teams WHERE id = $1 AND company_id = $2",
        team_id,
        company_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(team)
}

/// Insert a team. A unique-constraint violation is translated to `Conflict`.
pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    name: &str,
    description: Option<&str>,
    tag: &str,
    created_by: Uuid,
) -> AppResult<Team> {
    sqlx::query_as!(
        Team,
        r#"INSERT INTO teams (company_id, name, description, tag, created_by)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *"#,
        company_id,
        name,
        description,
        tag,
        created_by,
    )
    .fetch_one(executor)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err)
            if db_err.constraint() == Some("teams_company_id_name_key") =>
        {
            AppError::Conflict(format!("A team named '{}' already exists", name))
        }
        _ => AppError::Database(e),
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn update(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    team_id: Uuid,
    name: Option<&str>,
    description: Option<&str>,
    tag: Option<&str>,
    is_active: Option<bool>,
    updated_by: Uuid,
) -> AppResult<Option<Team>> {
    let team = sqlx::query_as!(
        Team,
        r#"UPDATE teams SET
            name = COALESCE($3, name),
            description = CASE WHEN $4::text IS NULL THEN description ELSE NULLIF(BTRIM($4), '') END,
            tag = COALESCE(NULLIF(BTRIM($5), ''), tag),
            is_active = COALESCE($6, is_active),
            updated_by = $7,
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *"#,
        team_id,
        company_id,
        name,
        description,
        tag,
        is_active,
        updated_by,
    )
    .fetch_optional(executor)
    .await?;
    Ok(team)
}

pub async fn delete(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    team_id: Uuid,
) -> AppResult<u64> {
    let rows = sqlx::query!(
        "DELETE FROM teams WHERE id = $1 AND company_id = $2",
        team_id,
        company_id,
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows)
}
