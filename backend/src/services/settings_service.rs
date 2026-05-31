use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::setting::{CompanySetting, SettingUpdate};
use crate::repositories::company_settings;

pub async fn get_all_settings(
    pool: &PgPool,
    company_id: Uuid,
    category: Option<&str>,
) -> AppResult<Vec<CompanySetting>> {
    company_settings::list(pool, company_id, category).await
}

pub async fn get_setting(
    pool: &PgPool,
    company_id: Uuid,
    category: &str,
    key: &str,
) -> AppResult<CompanySetting> {
    company_settings::get(pool, company_id, category, key)
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
    company_settings::update(pool, company_id, category, key, &value, updated_by)
        .await?
        .ok_or_else(|| AppError::NotFound("Setting not found".into()))
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
        let setting = company_settings::update(
            &mut *tx,
            company_id,
            &update.category,
            &update.key,
            &update.value,
            updated_by,
        )
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Setting not found: {}/{}",
                update.category, update.key
            ))
        })?;

        results.push(setting);
    }

    tx.commit().await?;
    Ok(results)
}
