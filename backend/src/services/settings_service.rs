use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::setting::{CompanySetting, SettingUpdate};

pub async fn get_all_settings(
    pool: &PgPool,
    company_id: Uuid,
    category: Option<&str>,
) -> AppResult<Vec<CompanySetting>> {
    let settings = sqlx::query_as::<_, CompanySetting>(
        r#"SELECT * FROM company_settings
        WHERE company_id = $1
        AND ($2::text IS NULL OR category = $2)
        ORDER BY category, key"#,
    )
    .bind(company_id)
    .bind(category)
    .fetch_all(pool)
    .await?;
    Ok(settings)
}

pub async fn get_setting(
    pool: &PgPool,
    company_id: Uuid,
    category: &str,
    key: &str,
) -> AppResult<CompanySetting> {
    sqlx::query_as::<_, CompanySetting>(
        "SELECT * FROM company_settings WHERE company_id = $1 AND category = $2 AND key = $3",
    )
    .bind(company_id)
    .bind(category)
    .bind(key)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Setting not found".into()))
}

pub async fn update_setting(
    pool: &PgPool,
    company_id: Uuid,
    category: &str,
    key: &str,
    value: serde_json::Value,
    updated_by: Uuid,
) -> AppResult<CompanySetting> {
    let setting = sqlx::query_as::<_, CompanySetting>(
        r#"UPDATE company_settings
        SET value = $4, updated_by = $5, updated_at = NOW()
        WHERE company_id = $1 AND category = $2 AND key = $3
        RETURNING *"#,
    )
    .bind(company_id)
    .bind(category)
    .bind(key)
    .bind(&value)
    .bind(updated_by)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Setting not found".into()))?;

    Ok(setting)
}

pub async fn bulk_update_settings(
    pool: &PgPool,
    company_id: Uuid,
    updates: Vec<SettingUpdate>,
    updated_by: Uuid,
) -> AppResult<Vec<CompanySetting>> {
    let mut tx = pool.begin().await?;
    let mut results = Vec::with_capacity(updates.len());

    for update in updates {
        let setting = sqlx::query_as::<_, CompanySetting>(
            r#"UPDATE company_settings
            SET value = $4, updated_by = $5, updated_at = NOW()
            WHERE company_id = $1 AND category = $2 AND key = $3
            RETURNING *"#,
        )
        .bind(company_id)
        .bind(&update.category)
        .bind(&update.key)
        .bind(&update.value)
        .bind(updated_by)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("Setting not found: {}/{}", update.category, update.key))
        })?;

        results.push(setting);
    }

    tx.commit().await?;
    Ok(results)
}
