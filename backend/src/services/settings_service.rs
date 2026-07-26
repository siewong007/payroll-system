use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::setting::{CompanySetting, SettingUpdate};
use crate::repositories::company_settings;
use crate::services::audit_service::{self, AuditRequestMeta};

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

/// One audit row per changed setting, carrying the old and new value.
///
/// Company settings drive the payroll engine — overtime multipliers, rounding,
/// statutory toggles — so "who changed this and what was it before" is exactly
/// the question an auditor asks after an unexpected run. None of it was
/// recorded before.
async fn log_setting_change(
    pool: &PgPool,
    company_id: Uuid,
    updated_by: Uuid,
    before: Option<&CompanySetting>,
    after: &CompanySetting,
    audit_meta: Option<&AuditRequestMeta>,
) {
    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(updated_by),
        "update",
        "company_setting",
        Some(after.id),
        before.map(|setting| {
            serde_json::json!({
                "category": setting.category,
                "key": setting.key,
                "value": setting.value,
            })
        }),
        Some(serde_json::json!({
            "category": after.category,
            "key": after.key,
            "value": after.value,
        })),
        Some(&format!("Setting {}/{} updated", after.category, after.key)),
        audit_meta,
    )
    .await;
}

pub async fn update_setting(
    pool: &PgPool,
    company_id: Uuid,
    category: &str,
    key: &str,
    value: serde_json::Value,
    updated_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<CompanySetting> {
    let before = company_settings::get(pool, company_id, category, key).await?;

    let setting = company_settings::update(pool, company_id, category, key, &value, updated_by)
        .await?
        .ok_or_else(|| AppError::NotFound("Setting not found".into()))?;

    log_setting_change(
        pool,
        company_id,
        updated_by,
        before.as_ref(),
        &setting,
        audit_meta,
    )
    .await;

    Ok(setting)
}

pub async fn bulk_update_settings(
    pool: &PgPool,
    company_id: Uuid,
    updates: Vec<SettingUpdate>,
    updated_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Vec<CompanySetting>> {
    let mut tx = pool.begin().await?;
    let mut results = Vec::with_capacity(updates.len());
    let mut before_values = Vec::with_capacity(results.capacity());

    for update in updates {
        let before =
            company_settings::get(&mut *tx, company_id, &update.category, &update.key).await?;

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

        before_values.push(before);
        results.push(setting);
    }

    tx.commit().await?;

    // Audited after the commit, deliberately: these rows describe changes that
    // have already landed, and a failed audit insert must not roll back a
    // successful settings update.
    for (before, after) in before_values.iter().zip(results.iter()) {
        log_setting_change(
            pool,
            company_id,
            updated_by,
            before.as_ref(),
            after,
            audit_meta,
        )
        .await;
    }

    Ok(results)
}
