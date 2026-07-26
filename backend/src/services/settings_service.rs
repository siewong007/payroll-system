use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::payroll::OvertimeSettings;
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

/// The company's overtime configuration, read once by every caller that rates
/// overtime.
///
/// Lives here rather than in `payroll_engine` because two callers now need it:
/// the run (which prices overtime) and attendance check-out (which decides
/// whether a derived figure is plausible enough to rate at all). Two copies
/// would let a company's ceiling and its multipliers drift apart.
///
/// A missing or unparsable setting falls back to the statutory defaults
/// (Employment Act multipliers, 26-day month, 8-hour day). Zero is treated as
/// unset: it is never a meaningful divisor, multiplier or ceiling, and one
/// blanked field would otherwise silently zero every overtime payment.
pub(crate) async fn overtime_settings(pool: &PgPool, company_id: Uuid) -> OvertimeSettings {
    async fn decimal_setting(
        pool: &PgPool,
        company_id: Uuid,
        key: &str,
        default: Decimal,
    ) -> Decimal {
        get_setting(pool, company_id, "payroll", key)
            .await
            .ok()
            .and_then(|s| {
                s.value
                    .as_str()
                    .and_then(|v| Decimal::from_str_exact(v).ok())
            })
            .filter(|v| !v.is_zero())
            .unwrap_or(default)
    }

    OvertimeSettings {
        effective_hours_per_day: decimal_setting(
            pool,
            company_id,
            "effective_hours_per_day",
            Decimal::from(8),
        )
        .await,
        working_days_per_month: decimal_setting(
            pool,
            company_id,
            "unpaid_leave_divisor",
            Decimal::from(26),
        )
        .await,
        multiplier_normal: decimal_setting(
            pool,
            company_id,
            "overtime_multiplier_normal",
            Decimal::new(15, 1),
        )
        .await,
        multiplier_rest_day: decimal_setting(
            pool,
            company_id,
            "overtime_multiplier_rest",
            Decimal::from(2),
        )
        .await,
        multiplier_public_holiday: decimal_setting(
            pool,
            company_id,
            "overtime_multiplier_public",
            Decimal::from(3),
        )
        .await,
        max_overtime_hours_per_day: decimal_setting(
            pool,
            company_id,
            "max_overtime_hours_per_day",
            Decimal::from(4),
        )
        .await,
    }
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
