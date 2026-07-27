use rust_decimal::Decimal;
use sqlx::{Executor, PgPool, Postgres};
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

/// Executor-generic so a caller already holding a connection reads on it rather
/// than taking a second one from the pool. `&PgPool` still satisfies it, so the
/// handler path is unchanged.
pub async fn get_setting(
    executor: impl Executor<'_, Database = Postgres>,
    company_id: Uuid,
    category: &str,
    key: &str,
) -> AppResult<CompanySetting> {
    company_settings::get(executor, company_id, category, key)
        .await?
        .ok_or_else(|| AppError::NotFound("Setting not found".into()))
}

/// The `payroll` settings the overtime and unpaid-leave arithmetic divides or
/// multiplies by. Every one of them must be a number greater than zero: a
/// divisor of `"0"` is a panic and a multiplier of `"0"` silently zeroes an
/// employee's overtime.
///
/// `unpaid_leave_divisor` is the pre-1017 name of `working_days_per_month` and
/// is listed so a settings row restored from an old backup can still be
/// corrected through the API rather than only rejected by it.
const POSITIVE_NUMERIC_PAYROLL_KEYS: &[&str] = &[
    "effective_hours_per_day",
    "working_days_per_month",
    "unpaid_leave_divisor",
    "overtime_multiplier_normal",
    "overtime_multiplier_rest",
    "overtime_multiplier_public",
    "max_overtime_hours_per_day",
];

/// Reject a value that would make the payroll arithmetic wrong before it is
/// stored, rather than defaulting around it on every read.
///
/// The read side deliberately keeps its own fallback (see `overtime_settings`)
/// so a tenant whose row is *already* bad still gets a run; this stops any new
/// one being written. Rejecting names the field and the offending value, because
/// the settings UI saves the whole category at once and "invalid value" alone
/// leaves the operator hunting.
pub(crate) fn validate_numeric_payroll_setting(
    category: &str,
    key: &str,
    value: &serde_json::Value,
) -> AppResult<()> {
    if category != "payroll" || !POSITIVE_NUMERIC_PAYROLL_KEYS.contains(&key) {
        return Ok(());
    }

    let parsed = match value {
        serde_json::Value::String(text) => Decimal::from_str_exact(text.trim()).ok(),
        // The seed stores these as JSON strings and the UI posts strings, but a
        // bare number is the same intent and rejecting it would be pedantry.
        serde_json::Value::Number(number) => Decimal::from_str_exact(&number.to_string()).ok(),
        _ => None,
    };

    match parsed {
        Some(decimal) if decimal > Decimal::ZERO => Ok(()),
        _ => Err(AppError::BadRequest(format!(
            "Setting payroll/{key} must be a number greater than zero, but {value} was supplied. \
             It divides or multiplies overtime and unpaid-leave pay, so zero, a blank and a \
             non-numeric value are all unusable."
        ))),
    }
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
/// `validate_numeric_payroll_setting` stops any new such value being written;
/// this fallback is what keeps an already-bad row from failing a run.
///
/// Six sequential reads, so it takes a reborrowable connection: `overtime_settings`
/// below is the pool-taking wrapper for callers holding no transaction.
pub(crate) async fn overtime_settings_on(
    conn: &mut sqlx::PgConnection,
    company_id: Uuid,
) -> OvertimeSettings {
    async fn decimal_setting(
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        key: &str,
    ) -> Option<Decimal> {
        get_setting(&mut *conn, company_id, "payroll", key)
            .await
            .ok()
            .and_then(|s| {
                s.value
                    .as_str()
                    .and_then(|v| Decimal::from_str_exact(v).ok())
            })
            .filter(|v| !v.is_zero())
    }

    let defaults = OvertimeSettings::statutory_defaults();

    // One `let` per read, deliberately: each call reborrows `conn`, and the
    // temporaries of a single struct-literal expression all live to the end of
    // that statement, which would overlap the mutable borrows.
    let effective_hours = decimal_setting(conn, company_id, "effective_hours_per_day").await;

    // Migration 1017 renamed this key: it has always been the divisor that
    // derives the OVERTIME hourly rate, never anything to do with unpaid leave.
    // The old name is still read as a fallback for one release, for a tenant
    // whose migration ran but whose settings row was restored from an older
    // backup.
    let mut working_days = decimal_setting(conn, company_id, "working_days_per_month").await;
    if working_days.is_none() {
        working_days = decimal_setting(conn, company_id, "unpaid_leave_divisor").await;
    }

    let normal = decimal_setting(conn, company_id, "overtime_multiplier_normal").await;
    let rest_day = decimal_setting(conn, company_id, "overtime_multiplier_rest").await;
    let public_holiday = decimal_setting(conn, company_id, "overtime_multiplier_public").await;
    let max_overtime = decimal_setting(conn, company_id, "max_overtime_hours_per_day").await;

    OvertimeSettings {
        effective_hours_per_day: effective_hours.unwrap_or(defaults.effective_hours_per_day),
        working_days_per_month: working_days.unwrap_or(defaults.working_days_per_month),
        multiplier_normal: normal.unwrap_or(defaults.multiplier_normal),
        multiplier_rest_day: rest_day.unwrap_or(defaults.multiplier_rest_day),
        multiplier_public_holiday: public_holiday.unwrap_or(defaults.multiplier_public_holiday),
        max_overtime_hours_per_day: max_overtime.unwrap_or(defaults.max_overtime_hours_per_day),
    }
}

/// Pool-taking wrapper for callers that hold no transaction.
///
/// A pool that cannot hand out a connection yields the statutory defaults, which
/// is the same answer an unreadable settings row already gives — this function
/// has never had a way to report failure and adding one would push a `?` into
/// two unrelated call sites.
pub(crate) async fn overtime_settings(pool: &PgPool, company_id: Uuid) -> OvertimeSettings {
    match pool.acquire().await {
        Ok(mut conn) => overtime_settings_on(&mut conn, company_id).await,
        Err(_) => OvertimeSettings::statutory_defaults(),
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
    validate_numeric_payroll_setting(category, key, &value)?;

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
    // Validated before the transaction opens: the settings UI saves a whole
    // category at once, so one bad multiplier must reject the batch rather than
    // land half of it.
    for update in &updates {
        validate_numeric_payroll_setting(&update.category, &update.key, &update.value)?;
    }

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
