//! Comprehensive, idempotent development & benchmark scenario seeder covering all
//! 62 public PostgreSQL tables, every RBAC role, multi-tenant timezones,
//! Malaysian statutory archetypes, payroll lifecycle transitions, attendance,
//! leave/overtime/claims approvals, teams, documents, notifications, Web Push,
//! background jobs, bulk imports, and audit history.
//!
//! Safety: refuses execution if any `APP_ENV`/`RUST_ENV`/`ENVIRONMENT` equals
//! `production` or `prod`, or if `DATABASE_URL` points to a non-local host
//! without `ALLOW_DEV_SEED=true`. Never touches `1001_data.sql` or `2024` test
//! fixtures (`test-fixture-*-2024`, `legacy-prototype-*-2024`).
//!
//! Usage:
//!   cargo run --features dev-seed --bin seed_scenarios
//!   cargo run --features dev-seed --bin seed_scenarios -- --reset
//!   cargo run --features dev-seed --bin seed_scenarios -- --reset --purge-test-junk
//!   cargo run --features dev-seed --bin seed_scenarios -- --reset --scale=perf

use std::env;
use std::process;

use aws_lc_rs::agreement::{ECDH_P256, EphemeralPrivateKey};
use aws_lc_rs::rand::SystemRandom;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration as ChronoDuration, NaiveDate, NaiveTime, TimeZone, Utc};
use payroll_system::core::auth::{AuthUser, Claims, JWT_AUDIENCE, JWT_ISSUER};
use payroll_system::core::crypto::encrypt_secret;
use payroll_system::core::db::{create_pool, run_migrations};
use payroll_system::repositories::companies;
use payroll_system::services::{payroll_engine, payroll_lifecycle_service};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use url::Url;
use uuid::Uuid;

/// Fixed UUIDv5 namespace for deterministic idempotent scenario keys.
const SEED_NAMESPACE: Uuid = Uuid::from_bytes([
    0x74, 0x19, 0x62, 0x41, 0x9b, 0x5c, 0x4d, 0x8e, 0xa1, 0x22, 0x90, 0x0f, 0x53, 0xc8, 0x20, 0x26,
]);

const SEED_PASSWORD_PLAIN: &str = "SeedPass!2026";

const SEEDED_COMPANY_NAMES: &[&str] = &[
    "Aurelia Tech Sdn Bhd",
    "Selatan Manufacturing Sdn Bhd",
    "Merlion Digital Pte Ltd",
];

const SEEDED_EMAIL_DOMAINS: &[&str] = &[
    "%@aureliatech.test",
    "%@selatanmfg.test",
    "%@merliondigital.test",
    "%@payrollmy.test",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scale {
    Standard,
    Perf,
}

struct CliOpts {
    reset: bool,
    purge_test_junk: bool,
    scale: Scale,
}

fn parse_args() -> Result<CliOpts, String> {
    let mut opts = CliOpts {
        reset: false,
        purge_test_junk: false,
        scale: Scale::Standard,
    };
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--reset" => opts.reset = true,
            "--purge-test-junk" => opts.purge_test_junk = true,
            "--scale=standard" => opts.scale = Scale::Standard,
            "--scale=perf" => opts.scale = Scale::Perf,
            "--scale" => match args.next().as_deref() {
                Some("standard") => opts.scale = Scale::Standard,
                Some("perf") => opts.scale = Scale::Perf,
                other => return Err(format!("invalid --scale value: {:?}", other)),
            },
            "-h" | "--help" => {
                println!(
                    "Usage: seed_scenarios [--reset] [--purge-test-junk] [--scale=standard|perf]"
                );
                process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(opts)
}

fn det_id(key: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(SEED_NAMESPACE.as_bytes());
    hasher.update(key.as_bytes());
    let hash = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn sha256_hex(input: &str) -> String {
    let hasher = Sha256::digest(input.as_bytes());
    let mut out = String::with_capacity(64);
    for b in hasher {
        use std::fmt::Write;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn backup_code_lookup_hash(encryption_key: &str, user_id: Uuid, normalized_code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"totp-backup-lookup-v1:");
    hasher.update(encryption_key.as_bytes());
    hasher.update(b":");
    hasher.update(user_id.to_string().as_bytes());
    hasher.update(b":");
    hasher.update(normalized_code.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{b:02x}");
    }
    hex
}

fn enforce_non_production_guards(database_url: &str) -> Result<(), String> {
    for var in ["APP_ENV", "RUST_ENV", "ENVIRONMENT"] {
        if let Ok(val) = env::var(var) {
            let norm = val.trim().to_ascii_lowercase();
            if norm == "production" || norm == "prod" {
                return Err(format!(
                    "Refusing to seed: {var}={val:?} indicates a production environment."
                ));
            }
        }
    }

    let parsed = Url::parse(database_url)
        .map_err(|e| format!("Invalid DATABASE_URL (details redacted): {e}"))?;
    let host = parsed
        .host_str()
        .unwrap_or("")
        .trim_matches(|c| c == '[' || c == ']')
        .to_ascii_lowercase();
    let is_local = matches!(host.as_str(), "127.0.0.1" | "localhost" | "::1");
    let allow_override = env::var("ALLOW_DEV_SEED")
        .map(|v| v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if let Ok(frontend_url) = env::var("FRONTEND_URL") {
        let trimmed_frontend = frontend_url.trim();
        if let Ok(parsed_frontend) = Url::parse(trimmed_frontend) {
            let fhost = parsed_frontend
                .host_str()
                .unwrap_or("")
                .trim_matches(|c| c == '[' || c == ']')
                .to_ascii_lowercase();
            let frontend_local = matches!(fhost.as_str(), "127.0.0.1" | "localhost" | "::1");
            if parsed_frontend.scheme().eq_ignore_ascii_case("https")
                && !frontend_local
                && !allow_override
            {
                return Err(format!(
                    "Refusing to seed: FRONTEND_URL points to non-local HTTPS origin '{fhost}'."
                ));
            }
        }
    }

    if !is_local && !allow_override {
        return Err(format!(
            "Refusing to seed non-local database host '{host}'. Set ALLOW_DEV_SEED=true only on isolated non-production environments."
        ));
    }
    Ok(())
}

fn auth_for(user_id: Uuid, company_id: Option<Uuid>, email: &str, roles: &[&str]) -> AuthUser {
    AuthUser(
        Claims {
            sub: user_id,
            email: email.to_string(),
            roles: roles.iter().map(|r| (*r).to_string()).collect(),
            company_id,
            employee_id: None,
            sid: det_id(&format!("claim-sid:{user_id}")),
            exp: (Utc::now() + ChronoDuration::hours(8)).timestamp(),
            iat: Utc::now().timestamp(),
            iss: JWT_ISSUER.into(),
            aud: JWT_AUDIENCE.into(),
        },
        Vec::new(),
    )
}

fn synthetic_p256dh_and_auth(seed_idx: u8) -> (String, String) {
    let rng = SystemRandom::new();
    let priv_key = EphemeralPrivateKey::generate(&ECDH_P256, &rng).expect("generate P-256 keypair");
    let pub_bytes = priv_key.compute_public_key().expect("compute public key");
    let mut auth = [0u8; 16];
    for (i, byte) in auth.iter_mut().enumerate() {
        *byte = seed_idx
            .wrapping_mul(31)
            .wrapping_add(i as u8)
            .wrapping_add(7);
    }
    (
        URL_SAFE_NO_PAD.encode(pub_bytes.as_ref()),
        URL_SAFE_NO_PAD.encode(auth),
    )
}

async fn delete_users_safely(
    tx: &mut Transaction<'_, Postgres>,
    user_ids: &[Uuid],
) -> Result<(), sqlx::Error> {
    if user_ids.is_empty() {
        return Ok(());
    }
    // Null optional NO ACTION / user references before deleting orphan global users.
    sqlx::query("UPDATE platform_settings SET updated_by = NULL WHERE updated_by = ANY($1)")
        .bind(user_ids)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "UPDATE password_reset_requests SET reviewed_by = NULL WHERE reviewed_by = ANY($1)",
    )
    .bind(user_ids)
    .execute(&mut **tx)
    .await?;
    sqlx::query("DELETE FROM users WHERE id = ANY($1)")
        .bind(user_ids)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn purge_company_children_batch(
    tx: &mut Transaction<'_, Postgres>,
    cids: &[Uuid],
) -> Result<(), sqlx::Error> {
    if cids.is_empty() {
        return Ok(());
    }
    sqlx::query(
        r#"
        DELETE FROM payroll_item_details
        WHERE payroll_item_id IN (
            SELECT id FROM payroll_items
            WHERE payroll_run_id IN (SELECT id FROM payroll_runs WHERE company_id = ANY($1))
               OR employee_id IN (SELECT id FROM employees WHERE company_id = ANY($1))
        )
        "#,
    )
    .bind(cids)
    .execute(&mut **tx)
    .await?;

    for sql in [
        "DELETE FROM payroll_entries WHERE company_id = ANY($1)",
        "DELETE FROM claims WHERE company_id = ANY($1)",
        "DELETE FROM payroll_items WHERE payroll_run_id IN (SELECT id FROM payroll_runs WHERE company_id = ANY($1)) OR employee_id IN (SELECT id FROM employees WHERE company_id = ANY($1))",
        "DELETE FROM payroll_runs WHERE company_id = ANY($1)",
        "DELETE FROM leave_requests WHERE company_id = ANY($1)",
        "DELETE FROM overtime_applications WHERE company_id = ANY($1)",
        "DELETE FROM leave_balances WHERE employee_id IN (SELECT id FROM employees WHERE company_id = ANY($1)) OR leave_type_id IN (SELECT id FROM leave_types WHERE company_id = ANY($1))",
        "DELETE FROM leave_types WHERE company_id = ANY($1)",
        "DELETE FROM employee_allowances WHERE company_id = ANY($1)",
        "DELETE FROM salary_history WHERE company_id = ANY($1)",
        "DELETE FROM tp3_records WHERE company_id = ANY($1)",
        "DELETE FROM documents WHERE company_id = ANY($1)",
        "DELETE FROM document_categories WHERE company_id = ANY($1)",
        "DELETE FROM email_logs WHERE company_id = ANY($1)",
        "DELETE FROM email_templates WHERE company_id = ANY($1)",
        "DELETE FROM notifications WHERE company_id = ANY($1)",
        "DELETE FROM team_members WHERE team_id IN (SELECT id FROM teams WHERE company_id = ANY($1))",
        "DELETE FROM teams WHERE company_id = ANY($1)",
        "DELETE FROM holidays WHERE company_id = ANY($1)",
        "DELETE FROM working_day_config WHERE company_id = ANY($1)",
        "DELETE FROM company_settings WHERE company_id = ANY($1)",
        "DELETE FROM bulk_import_sessions WHERE company_id = ANY($1)",
        "DELETE FROM background_jobs WHERE company_id = ANY($1)",
        "DELETE FROM attendance_records WHERE company_id = ANY($1)",
        "DELETE FROM attendance_qr_tokens WHERE company_id = ANY($1)",
        "DELETE FROM attendance_kiosk_credentials WHERE company_id = ANY($1)",
        "DELETE FROM attendance_network_observations WHERE company_id = ANY($1)",
        "DELETE FROM attendance_network_dismissals WHERE company_id = ANY($1)",
        "DELETE FROM company_networks WHERE company_id = ANY($1)",
        "DELETE FROM company_locations WHERE company_id = ANY($1)",
        "DELETE FROM employee_work_schedules WHERE company_id = ANY($1)",
        "DELETE FROM company_work_schedules WHERE company_id = ANY($1)",
        "DELETE FROM audit_logs WHERE company_id = ANY($1)",
        "DELETE FROM push_outbox WHERE company_id = ANY($1)",
        "DELETE FROM uploaded_files WHERE company_id = ANY($1)",
        "DELETE FROM employees WHERE company_id = ANY($1)",
        "DELETE FROM payroll_groups WHERE company_id = ANY($1)",
        "DELETE FROM user_companies WHERE company_id = ANY($1)",
        "DELETE FROM user_groups WHERE company_id = ANY($1)",
    ] {
        sqlx::query(sql).bind(cids).execute(&mut **tx).await?;
    }
    Ok(())
}

async fn purge_test_junk(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let test_company_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id FROM companies
        WHERE name LIKE 'TestCo-%'
           OR name LIKE 'Restored-%'
           OR name = 'Golden Sdn Bhd'
        ORDER BY created_at
        "#,
    )
    .fetch_all(pool)
    .await?;

    // Pass 1: Delete all child tables across all test-company chunks first to avoid cross-chunk FKs.
    for chunk in test_company_ids.chunks(500) {
        let mut tx = pool.begin().await?;
        purge_company_children_batch(&mut tx, chunk).await?;
        tx.commit().await?;
    }

    // Pass 2: Delete test users (belonging to test companies or synthetic test email domains).
    let test_users: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id FROM users
        WHERE company_id = ANY($1)
           OR email LIKE '%@example.invalid'
           OR email LIKE '%@example.test'
        "#,
    )
    .bind(&test_company_ids)
    .fetch_all(pool)
    .await?;
    for chunk in test_users.chunks(1000) {
        let mut tx = pool.begin().await?;
        delete_users_safely(&mut tx, chunk).await?;
        tx.commit().await?;
    }

    // Pass 3: Delete the companies themselves.
    for chunk in test_company_ids.chunks(1000) {
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM companies WHERE id = ANY($1)")
            .bind(chunk)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
    }

    println!(
        "Purged {} TestCo-* companies and {} test users.",
        test_company_ids.len(),
        test_users.len()
    );
    Ok(())
}

async fn reset_seeded_data(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let mut tx = pool.begin().await?;

    let seeded_company_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id FROM companies
        WHERE lower(name) = ANY($1)
        ORDER BY CASE lower(name)
            WHEN 'merlion digital pte ltd' THEN 1
            WHEN 'selatan manufacturing sdn bhd' THEN 2
            ELSE 3
        END
        "#,
    )
    .bind(
        SEEDED_COMPANY_NAMES
            .iter()
            .map(|n| n.to_ascii_lowercase())
            .collect::<Vec<_>>(),
    )
    .fetch_all(&mut *tx)
    .await?;

    let seeded_user_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM users WHERE lower(email) LIKE ANY($1)")
            .bind(
                SEEDED_EMAIL_DOMAINS
                    .iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<_>>(),
            )
            .fetch_all(&mut *tx)
            .await?;

    for cid in &seeded_company_ids {
        companies::delete_cascade(&mut tx, *cid).await?;
    }

    sqlx::query("DELETE FROM oauth2_states WHERE state LIKE 'dev-seed-%'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM passkey_challenges WHERE lower(COALESCE(email, '')) LIKE ANY($1)")
        .bind(
            SEEDED_EMAIL_DOMAINS
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>(),
        )
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM audit_logs WHERE description LIKE '[dev-seed]%'")
        .execute(&mut *tx)
        .await?;

    delete_users_safely(&mut tx, &seeded_user_ids).await?;

    // Remove dev-seed 2026 statutory rows respecting ON DELETE RESTRICT on statutory_rule_sets.
    let seed_rule_set_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM statutory_rule_sets WHERE dataset_key LIKE 'dev-seed-%'",
    )
    .fetch_all(&mut *tx)
    .await?;
    if !seed_rule_set_ids.is_empty() {
        for sql in [
            "DELETE FROM epf_rates WHERE rule_set_id = ANY($1)",
            "DELETE FROM socso_rates WHERE rule_set_id = ANY($1)",
            "DELETE FROM eis_rates WHERE rule_set_id = ANY($1)",
            "DELETE FROM pcb_brackets WHERE rule_set_id = ANY($1)",
            "DELETE FROM pcb_reliefs WHERE rule_set_id = ANY($1)",
        ] {
            sqlx::query(sql)
                .bind(&seed_rule_set_ids)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("DELETE FROM statutory_rule_sets WHERE id = ANY($1)")
            .bind(&seed_rule_set_ids)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    println!(
        "Reset cleared {} seeded companies, {} seeded users, and {} dev-seed statutory rule sets.",
        seeded_company_ids.len(),
        seeded_user_ids.len(),
        seed_rule_set_ids.len()
    );
    Ok(())
}

async fn ensure_rule_set(
    pool: &PgPool,
    dataset_key: &str,
    rule_code: &str,
) -> Result<Uuid, Box<dyn std::error::Error>> {
    if let Some(existing) =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM statutory_rule_sets WHERE dataset_key = $1")
            .bind(dataset_key)
            .fetch_optional(pool)
            .await?
    {
        return Ok(existing);
    }

    let id = det_id(&format!("rule-set:{dataset_key}"));
    let sha = sha256_hex(dataset_key);
    sqlx::query(
        r#"
        INSERT INTO statutory_rule_sets (
            id, dataset_key, rule_code, effective_from, effective_to, status,
            source_url, source_version, source_sha256, verification_notes, verified_at
        )
        VALUES ($1, $2, $3, DATE '2026-01-01', DATE '2026-12-31', 'verified',
                'https://example.invalid/dev-seed-2026', '2026.1-dev-seed', $4,
                'Non-production development & benchmark fixture (2026)', NOW())
        ON CONFLICT (dataset_key) DO NOTHING
        "#,
    )
    .bind(id)
    .bind(dataset_key)
    .bind(rule_code)
    .bind(&sha)
    .execute(pool)
    .await?;

    Ok(id)
}

fn build_epf_bands_for_category(category: &str) -> Vec<(i64, i64, i64, i64)> {
    // Contiguous non-overlapping bands covering RM0.01 .. RM50,000 (1..=5_000_000 sen)
    // across KWSP Parts A-F. Steps:
    //   1..=200_000 in RM20 increments (2_000 sen)
    //   200_001..=2_000_000 in RM100 increments (10_000 sen)
    //   2_000_001..=5_000_000 in RM500 increments (50_000 sen)
    let mut ranges: Vec<(i64, i64)> = Vec::new();
    let mut cur = 1_i64;
    while cur <= 200_000 {
        let to = (cur + 1_999).min(200_000);
        ranges.push((cur, to));
        cur = to + 1;
    }
    while cur <= 2_000_000 {
        let to = (cur + 9_999).min(2_000_000);
        ranges.push((cur, to));
        cur = to + 1;
    }
    while cur <= 5_000_000 {
        let to = (cur + 49_999).min(5_000_000);
        ranges.push((cur, to));
        cur = to + 1;
    }

    let ceil_rm = |wage_sen: i64, bps: i64| -> i64 {
        if bps == 0 {
            return 0;
        }
        // Round up to nearest RM (100 sen) over bps/10_000.
        let numerator = wage_sen * bps;
        let divisor = 10_000 * 100;
        ((numerator + divisor - 1) / divisor) * 100
    };

    ranges
        .into_iter()
        .map(|(from, to)| {
            let (ee, er) = match category {
                // Part A: Citizen/PR <60 -> EE 11%, ER 13% up to RM5,000 else 12%
                "A" => (
                    ceil_rm(to, 1100),
                    ceil_rm(to, if to <= 500_000 { 1300 } else { 1200 }),
                ),
                // Part B: Foreign worker <60 -> EE 11%, ER RM5 fixed
                "B" => (ceil_rm(to, 1100), 500),
                // Part C: PR >=60 -> EE 5.5%, ER 6% up to RM5,000 / 6%
                "C" => (ceil_rm(to, 550), ceil_rm(to, 600)),
                // Part D: Foreign worker >=60 -> EE 5.5%, ER RM5 fixed
                "D" => (ceil_rm(to, 550), 500),
                // Part E: Citizen >=60 (until age 75 cessation in code) -> EE 0%, ER 4%
                "E" => (0, ceil_rm(to, 400)),
                // Part F: Citizen/PR voluntary lower EE (9%), ER 4%
                _ => (ceil_rm(to, 900), ceil_rm(to, 400)),
            };
            (from, to, ee, er)
        })
        .collect()
}

async fn seed_statutory_2026(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    // Platform settings (non-destructive if already populated)
    for (k, v) in [
        ("allow_company_override", "true"),
        ("attendance_method", "face_id"),
        ("enforce_2fa", "false"),
        ("pcb_calculator_status", "enabled"),
    ] {
        sqlx::query(
            "INSERT INTO platform_settings (key, value) VALUES ($1, $2) ON CONFLICT (key) DO NOTHING",
        )
        .bind(k)
        .bind(v)
        .execute(pool)
        .await?;
    }

    let epf_rs = ensure_rule_set(pool, "dev-seed-epf-2026", "epf").await?;
    let socso_rs = ensure_rule_set(pool, "dev-seed-socso-2026", "socso").await?;
    let eis_rs = ensure_rule_set(pool, "dev-seed-eis-2026", "eis").await?;
    let pcb_rs = ensure_rule_set(pool, "dev-seed-pcb-2026", "pcb").await?;

    // EPF rates (Parts A..F up to RM50,000)
    let epf_exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM epf_rates WHERE rule_set_id = $1")
            .bind(epf_rs)
            .fetch_one(pool)
            .await?;
    if epf_exists == 0 {
        let mut ids = Vec::new();
        let mut w_from = Vec::new();
        let mut w_to = Vec::new();
        let mut ee = Vec::new();
        let mut er = Vec::new();
        let mut cats = Vec::new();
        for cat in ["A", "B", "C", "D", "E", "F"] {
            for (idx, (from, to, emp, emr)) in
                build_epf_bands_for_category(cat).into_iter().enumerate()
            {
                ids.push(det_id(&format!("epf-2026:{cat}:{idx}")));
                w_from.push(from);
                w_to.push(to);
                ee.push(emp);
                er.push(emr);
                cats.push(cat.to_string());
            }
        }
        sqlx::query(
            r#"
            INSERT INTO epf_rates (
                id, wage_from, wage_to, employee_contribution, employer_contribution,
                category, effective_from, effective_to, rule_set_id
            )
            SELECT u.id, u.wf, u.wt, u.ee, u.er, u.cat::bpchar,
                   DATE '2026-01-01', DATE '2026-12-31', $7
            FROM UNNEST($1::uuid[], $2::bigint[], $3::bigint[], $4::bigint[], $5::bigint[], $6::text[])
                 AS u(id, wf, wt, ee, er, cat)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(&ids)
        .bind(&w_from)
        .bind(&w_to)
        .bind(&ee)
        .bind(&er)
        .bind(&cats)
        .bind(epf_rs)
        .execute(pool)
        .await?;
    }

    // SOCSO rates (copy 2024 1..=600_000 bands or generate fallback)
    let socso_exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM socso_rates WHERE rule_set_id = $1")
            .bind(socso_rs)
            .fetch_one(pool)
            .await?;
    if socso_exists == 0 {
        let inserted = sqlx::query(
            r#"
            INSERT INTO socso_rates (
                wage_from, wage_to, first_cat_employee, first_cat_employer,
                second_cat_employer, effective_from, effective_to, rule_set_id
            )
            SELECT wage_from, wage_to, first_cat_employee, first_cat_employer,
                   second_cat_employer, DATE '2026-01-01', DATE '2026-12-31', $1
            FROM socso_rates
            WHERE effective_from = DATE '2024-01-01'
            ORDER BY wage_from
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(socso_rs)
        .execute(pool)
        .await?
        .rows_affected();

        if inserted == 0 {
            let mut from = 1_i64;
            while from <= 600_000 {
                let to = (from + 9_999).min(600_000);
                let mid = (from + to) / 2;
                let ee = (mid * 5 / 1000).min(2_975);
                let er1 = (mid * 175 / 10_000).min(10_415);
                let er2 = (mid * 125 / 10_000).min(7_440);
                sqlx::query(
                    r#"
                    INSERT INTO socso_rates (
                        id, wage_from, wage_to, first_cat_employee, first_cat_employer,
                        second_cat_employer, effective_from, effective_to, rule_set_id
                    ) VALUES ($1, $2, $3, $4, $5, $6, DATE '2026-01-01', DATE '2026-12-31', $7)
                    ON CONFLICT DO NOTHING
                    "#,
                )
                .bind(det_id(&format!("socso-2026-fb:{from}")))
                .bind(from)
                .bind(to)
                .bind(ee)
                .bind(er1)
                .bind(er2)
                .bind(socso_rs)
                .execute(pool)
                .await?;
                from = to + 1;
            }
        }
    }

    // EIS rates (copy 2024 1..=500_000 bands AND extend up to RM6,000 ceiling)
    let eis_exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM eis_rates WHERE rule_set_id = $1")
            .bind(eis_rs)
            .fetch_one(pool)
            .await?;
    if eis_exists == 0 {
        let base_inserted = sqlx::query(
            r#"
            INSERT INTO eis_rates (
                wage_from, wage_to, employee_contribution, employer_contribution,
                effective_from, effective_to, rule_set_id
            )
            SELECT wage_from, wage_to, employee_contribution, employer_contribution,
                   DATE '2026-01-01', DATE '2026-12-31', $1
            FROM eis_rates
            WHERE effective_from = DATE '2024-01-01'
            ORDER BY wage_from
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(eis_rs)
        .execute(pool)
        .await?
        .rows_affected();

        let ext_start = if base_inserted > 0 {
            500_001_i64
        } else {
            1_i64
        };
        let mut cur = ext_start;
        while cur <= 600_000 {
            let to = (cur + 9_999).min(600_000);
            let contrib = (((cur + to) / 2) * 2 / 1000 / 5 * 5).clamp(10, 1_190);
            sqlx::query(
                r#"
                INSERT INTO eis_rates (
                    id, wage_from, wage_to, employee_contribution, employer_contribution,
                    effective_from, effective_to, rule_set_id
                ) VALUES ($1, $2, $3, $4, $4, DATE '2026-01-01', DATE '2026-12-31', $5)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(det_id(&format!("eis-2026:{cur}")))
            .bind(cur)
            .bind(to)
            .bind(contrib)
            .bind(eis_rs)
            .execute(pool)
            .await?;
            cur = to + 1;
        }
    }

    // PCB brackets (2026 contiguous from 0 .. 9_999_999_999)
    let brackets: [(i64, i64, Decimal, i64); 10] = [
        (0, 500_000, dec!(0.00), 0),
        (500_001, 2_000_000, dec!(1.00), 0),
        (2_000_001, 3_500_000, dec!(3.00), 15_000),
        (3_500_001, 5_000_000, dec!(6.00), 60_000),
        (5_000_001, 7_000_000, dec!(11.00), 150_000),
        (7_000_001, 10_000_000, dec!(19.00), 370_000),
        (10_000_001, 40_000_000, dec!(25.00), 940_000),
        (40_000_001, 60_000_000, dec!(26.00), 8_440_000),
        (60_000_001, 200_000_000, dec!(28.00), 13_640_000),
        (200_000_001, 9_999_999_999, dec!(30.00), 52_840_000),
    ];
    for (idx, (from, to, rate, cum)) in brackets.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO pcb_brackets (
                id, chargeable_income_from, chargeable_income_to, tax_rate_percent,
                cumulative_tax, effective_year, rule_set_id
            ) VALUES ($1, $2, $3, $4, $5, 2026, $6)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(det_id(&format!("pcb-bracket-2026:{idx}")))
        .bind(*from)
        .bind(*to)
        .bind(*rate)
        .bind(*cum)
        .bind(pcb_rs)
        .execute(pool)
        .await?;
    }

    // All 14 statutory PCB relief keys for 2026
    let reliefs: [(&str, i64, &str); 14] = [
        (
            "individual",
            900_000,
            "Individual and dependent relatives relief",
        ),
        ("spouse", 400_000, "Non-working spouse / alimony relief"),
        ("child_under_18", 200_000, "Unmarried child under 18 relief"),
        (
            "child_18_plus_education",
            800_000,
            "Unmarried child 18+ in higher education",
        ),
        (
            "disabled_individual",
            600_000,
            "Disabled individual additional relief",
        ),
        (
            "disabled_spouse",
            500_000,
            "Disabled spouse additional relief",
        ),
        ("disabled_child", 600_000, "Disabled child relief"),
        ("socso_relief", 35_000, "SOCSO contribution relief cap"),
        ("eis_relief", 35_000, "EIS contribution relief cap"),
        (
            "epf_additional",
            400_000,
            "EPF statutory contribution relief cap",
        ),
        (
            "life_insurance",
            300_000,
            "Life insurance premium relief cap",
        ),
        (
            "medical_insurance",
            300_000,
            "Medical & education insurance relief cap",
        ),
        ("lifestyle_relief", 250_000, "Lifestyle relief cap"),
        (
            "tax_rebate_individual",
            40_000,
            "Individual tax rebate (chargeable <= RM35k)",
        ),
    ];
    for (rtype, amt, desc) in reliefs {
        sqlx::query(
            r#"
            INSERT INTO pcb_reliefs (id, relief_type, amount, effective_year, description, rule_set_id)
            VALUES ($1, $2, $3, 2026, $4, $5)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(det_id(&format!("pcb-relief-2026:{rtype}")))
        .bind(rtype)
        .bind(amt)
        .bind(desc)
        .bind(pcb_rs)
        .execute(pool)
        .await?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_company_with_defaults(
    pool: &PgPool,
    key: &str,
    name: &str,
    reg_no: &str,
    city: &str,
    state: &str,
    country: &str,
    tz: &str,
    geofence_mode: &str,
    network_mode: &str,
    attendance_method: &str,
    hrdf_enabled: bool,
    unpaid_divisor: i32,
    auto_absent_last: Option<NaiveDate>,
    sched_start: NaiveTime,
    sched_end: NaiveTime,
    working_days: [bool; 7],
) -> Result<Uuid, Box<dyn std::error::Error>> {
    if let Some(existing) =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM companies WHERE lower(name) = lower($1)")
            .bind(name)
            .fetch_optional(pool)
            .await?
    {
        return Ok(existing);
    }

    let id = det_id(&format!("company:{key}"));
    sqlx::query(
        r#"
        INSERT INTO companies (
            id, name, registration_number, tax_number, epf_number, socso_code, eis_code,
            hrdf_number, address_line1, city, state, postcode, country, phone, email,
            hrdf_enabled, unpaid_leave_divisor, is_active, attendance_method, timezone,
            geofence_mode, attendance_network_mode, auto_absent_last_run_date
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8,
            $9, $10, $11, '50088', $12, '+603-2188-9000', $13,
            $14, $15, TRUE, $16, $17, $18, $19, $20
        )
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(id)
    .bind(name)
    .bind(reg_no)
    .bind(format!("E-{}", &reg_no[..6.min(reg_no.len())]))
    .bind(format!("KWSP-{key}-01"))
    .bind(format!("PERKESO-{key}"))
    .bind(format!("SIP-{key}"))
    .bind(format!("HRDF-{key}"))
    .bind(format!("Level 18, Menara {key}"))
    .bind(city)
    .bind(state)
    .bind(country)
    .bind(format!("hr@{key}.test"))
    .bind(hrdf_enabled)
    .bind(unpaid_divisor)
    .bind(attendance_method)
    .bind(tz)
    .bind(geofence_mode)
    .bind(network_mode)
    .bind(auto_absent_last)
    .execute(pool)
    .await?;

    // Default company_work_schedules
    sqlx::query(
        r#"
        INSERT INTO company_work_schedules (
            id, company_id, name, start_time, end_time, grace_minutes,
            half_day_hours, timezone, is_default, unpaid_break_minutes
        ) VALUES ($1, $2, 'Default Shift', $3, $4, 15, 4.0, $5, TRUE, 60)
        ON CONFLICT (company_id) WHERE is_default = TRUE DO NOTHING
        "#,
    )
    .bind(det_id(&format!("sched:{key}")))
    .bind(id)
    .bind(sched_start)
    .bind(sched_end)
    .bind(tz)
    .execute(pool)
    .await?;

    // Working day config (0=Sun..6=Sat)
    for (dow, is_work) in working_days.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO working_day_config (id, company_id, day_of_week, is_working_day)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (company_id, day_of_week) DO NOTHING
            "#,
        )
        .bind(det_id(&format!("wd:{key}:{dow}")))
        .bind(id)
        .bind(dow as i16)
        .bind(*is_work)
        .execute(pool)
        .await?;
    }

    // Company settings across all standard categories
    let settings = [
        (
            "email",
            "auto_welcome_email",
            json!(true),
            "Auto Welcome Email",
        ),
        (
            "notifications",
            "email_payslip",
            json!(true),
            "Email Payslip",
        ),
        (
            "notifications",
            "expiry_alert_days",
            json!("30"),
            "Expiry Alert Days",
        ),
        (
            "notifications",
            "probation_alert_days",
            json!("14"),
            "Probation Alert Days",
        ),
        ("payroll", "default_cutoff_day", json!("25"), "Cutoff Day"),
        ("payroll", "default_pay_day", json!("28"), "Pay Day"),
        (
            "payroll",
            "effective_hours_per_day",
            json!("8"),
            "Effective Hours",
        ),
        (
            "payroll",
            "overtime_multiplier_normal",
            json!("1.5"),
            "Normal OT",
        ),
        (
            "payroll",
            "overtime_multiplier_rest",
            json!("2.0"),
            "Rest OT",
        ),
        (
            "payroll",
            "overtime_multiplier_public",
            json!("3.0"),
            "Public Holiday OT",
        ),
        (
            "payroll",
            "working_days_per_month",
            json!("26"),
            "Working Days/Month",
        ),
        ("statutory", "eis_enabled", json!(true), "EIS Enabled"),
        ("statutory", "socso_enabled", json!(true), "SOCSO Enabled"),
        (
            "statutory",
            "hrdf_enabled",
            json!(hrdf_enabled),
            "HRDF Enabled",
        ),
        (
            "statutory",
            "minimum_wage_monthly",
            json!("1700"),
            "Minimum Wage",
        ),
        (
            "system",
            "currency",
            json!(if country == "Singapore" { "SGD" } else { "MYR" }),
            "Currency",
        ),
    ];
    for (cat, k, val, label) in settings {
        sqlx::query(
            r#"
            INSERT INTO company_settings (id, company_id, category, key, value, label)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (company_id, category, key) DO NOTHING
            "#,
        )
        .bind(det_id(&format!("cset:{key}:{cat}:{k}")))
        .bind(id)
        .bind(cat)
        .bind(k)
        .bind(val)
        .bind(label)
        .execute(pool)
        .await?;
    }

    Ok(id)
}

#[allow(clippy::too_many_arguments)]
async fn insert_user(
    pool: &PgPool,
    key: &str,
    email: &str,
    password_hash: &str,
    full_name: &str,
    primary_company_id: Option<Uuid>,
    employee_id: Option<Uuid>,
    roles: &[&str],
    is_active: bool,
    must_change_password: bool,
    deleted: bool,
    linked_companies: &[Uuid],
) -> Result<Uuid, Box<dyn std::error::Error>> {
    let id = if let Some(existing) = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1))",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?
    {
        existing
    } else {
        let uid = det_id(&format!("user:{key}"));
        let roles_vec: Vec<String> = roles.iter().map(|r| (*r).to_string()).collect();
        let deleted_at = if deleted {
            Some(Utc.with_ymd_and_hms(2026, 9, 1, 10, 0, 0).unwrap())
        } else {
            None
        };
        sqlx::query(
            r#"
            INSERT INTO users (
                id, email, password_hash, full_name, company_id, employee_id,
                is_active, must_change_password, roles, deleted_at, last_login
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::varchar(50)[], $10, NOW() - INTERVAL '2 days')
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(uid)
        .bind(email)
        .bind(password_hash)
        .bind(full_name)
        .bind(primary_company_id)
        .bind(employee_id)
        .bind(is_active)
        .bind(must_change_password)
        .bind(&roles_vec)
        .bind(deleted_at)
        .execute(pool)
        .await?;
        uid
    };

    for cid in linked_companies {
        sqlx::query(
            "INSERT INTO user_companies (user_id, company_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(id)
        .bind(*cid)
        .execute(pool)
        .await?;
    }

    Ok(id)
}

#[allow(clippy::too_many_arguments)]
async fn insert_employee(
    pool: &PgPool,
    key: &str,
    company_id: Uuid,
    group_id: Option<Uuid>,
    emp_no: &str,
    full_name: &str,
    dob: Option<NaiveDate>,
    gender: &str,
    race: &str,
    nationality: &str,
    residency: &str,
    marital: &str,
    working_spouse: bool,
    num_children: i32,
    epf_cat: Option<&str>,
    is_muslim: bool,
    zakat_sen: i64,
    ptptn_sen: i64,
    th_sen: i64,
    dept: &str,
    designation: &str,
    emp_type: &str,
    date_joined: NaiveDate,
    probation_start: Option<NaiveDate>,
    probation_end: Option<NaiveDate>,
    date_resigned: Option<NaiveDate>,
    resignation_reason: Option<&str>,
    basic_sen: i64,
    hourly_rate: Option<i64>,
    daily_rate: Option<i64>,
    bank_acct: &str,
    is_active: bool,
    email: &str,
) -> Result<Uuid, Box<dyn std::error::Error>> {
    if let Some(existing) = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM employees WHERE company_id = $1 AND employee_number = $2",
    )
    .bind(company_id)
    .bind(emp_no)
    .fetch_optional(pool)
    .await?
    {
        return Ok(existing);
    }

    let id = det_id(&format!("emp:{key}"));
    let confirmation_date = if probation_end.is_none() && date_resigned.is_none() {
        Some(date_joined + ChronoDuration::days(90))
    } else {
        None
    };

    sqlx::query(
        r#"
        INSERT INTO employees (
            id, company_id, employee_number, full_name, ic_number, passport_number,
            date_of_birth, gender, nationality, race, residency_status, marital_status,
            email, phone, address_line1, city, state, postcode, department, designation,
            cost_centre, branch, employment_type, date_joined, probation_start, probation_end,
            confirmation_date, date_resigned, resignation_reason,
            basic_salary, hourly_rate, daily_rate,
            bank_name, bank_account_number, bank_account_type,
            tax_identification_number, epf_number, socso_number, eis_number,
            working_spouse, num_children, epf_category, is_muslim,
            zakat_eligible, zakat_monthly_amount, ptptn_monthly_amount, tabung_haji_amount,
            hrdf_contribution, payroll_group_id, is_active
        ) VALUES (
            $1, $2, $3, $4, $5, $6,
            $7, $8::gender_type, $9, $10::race_type, $11::residency_status, $12::marital_status,
            $13, '+6012-3456789', 'No. 18 Jalan Ampang', 'Kuala Lumpur', 'WP Kuala Lumpur', '50450',
            $14, $15, 'CC-100', 'HQ', $16::employment_type, $17, $18, $19,
            $20, $21, $22,
            $23, $24, $25,
            'Maybank', $26, 'savings',
            $27, $28, $29, $30,
            $31, $32, $33::bpchar, $34,
            $35, $36, $37, $38,
            TRUE, $39, $40
        )
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(id)
    .bind(company_id)
    .bind(emp_no)
    .bind(full_name)
    .bind(if residency == "foreigner" {
        None
    } else {
        Some(format!(
            "900101-14-{}",
            &emp_no[emp_no.len().saturating_sub(4)..]
        ))
    })
    .bind(if residency == "foreigner" {
        Some(format!("P-{}", emp_no))
    } else {
        None
    })
    .bind(dob)
    .bind(gender)
    .bind(nationality)
    .bind(race)
    .bind(residency)
    .bind(marital)
    .bind(email)
    .bind(dept)
    .bind(designation)
    .bind(emp_type)
    .bind(date_joined)
    .bind(probation_start)
    .bind(probation_end)
    .bind(confirmation_date)
    .bind(date_resigned)
    .bind(resignation_reason)
    .bind(basic_sen)
    .bind(hourly_rate)
    .bind(daily_rate)
    .bind(bank_acct)
    .bind(format!("IG-{emp_no}"))
    .bind(format!("EPF-{emp_no}"))
    .bind(format!("SOC-{emp_no}"))
    .bind(format!("EIS-{emp_no}"))
    .bind(working_spouse)
    .bind(num_children)
    .bind(epf_cat)
    .bind(is_muslim)
    .bind(zakat_sen > 0)
    .bind(zakat_sen)
    .bind(ptptn_sen)
    .bind(th_sen)
    .bind(group_id)
    .bind(is_active)
    .execute(pool)
    .await?;

    Ok(id)
}

async fn ensure_group(
    pool: &PgPool,
    key: &str,
    company_id: Uuid,
    name: &str,
    desc: &str,
) -> Result<Uuid, Box<dyn std::error::Error>> {
    if let Some(existing) = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM payroll_groups WHERE company_id = $1 AND lower(btrim(name)) = lower(btrim($2))",
    )
    .bind(company_id)
    .bind(name)
    .fetch_optional(pool)
    .await?
    {
        return Ok(existing);
    }
    let id = det_id(&format!("pgroup:{key}"));
    sqlx::query(
        r#"
        INSERT INTO payroll_groups (id, company_id, name, description, cutoff_day, payment_day)
        VALUES ($1, $2, $3, $4, 25, 28)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(id)
    .bind(company_id)
    .bind(name)
    .bind(desc)
    .execute(pool)
    .await?;
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
async fn ensure_leave_type(
    pool: &PgPool,
    key: &str,
    company_id: Uuid,
    name: &str,
    default_days: Decimal,
    is_paid: bool,
    max_cf: Decimal,
    cf_expiry_months: i32,
    is_system: bool,
) -> Result<Uuid, Box<dyn std::error::Error>> {
    if let Some(existing) = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM leave_types WHERE company_id = $1 AND name = $2",
    )
    .bind(company_id)
    .bind(name)
    .fetch_optional(pool)
    .await?
    {
        return Ok(existing);
    }
    let id = det_id(&format!("lt:{key}:{name}"));
    sqlx::query(
        r#"
        INSERT INTO leave_types (
            id, company_id, name, default_days, is_paid, is_active,
            max_carry_forward, carry_forward_expiry_months, is_system
        ) VALUES ($1, $2, $3, $4, $5, TRUE, $6, $7, $8)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(id)
    .bind(company_id)
    .bind(name)
    .bind(default_days)
    .bind(is_paid)
    .bind(max_cf)
    .bind(cf_expiry_months)
    .bind(is_system)
    .execute(pool)
    .await?;
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
async fn ensure_active_payroll_run(
    pool: &PgPool,
    company_id: Uuid,
    group_id: Uuid,
    year: i32,
    month: i32,
    pay_date: NaiveDate,
    payroll_admin_id: Uuid,
    payroll_admin_email: &str,
    finance_id: Uuid,
    finance_email: &str,
    target_status: &str,
    notes: &str,
) -> Result<Uuid, Box<dyn std::error::Error>> {
    if let Some(existing) = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id FROM payroll_runs
        WHERE company_id = $1 AND payroll_group_id = $2
          AND period_year = $3 AND period_month = $4 AND status <> 'cancelled'
        ORDER BY version DESC LIMIT 1
        "#,
    )
    .bind(company_id)
    .bind(group_id)
    .bind(year)
    .bind(month)
    .fetch_optional(pool)
    .await?
    {
        return Ok(existing);
    }

    let finance_auth = auth_for(finance_id, Some(company_id), finance_email, &["finance"]);
    let admin_auth = auth_for(
        payroll_admin_id,
        Some(company_id),
        payroll_admin_email,
        &["payroll_admin"],
    );

    if target_status == "cancel_then_paid" {
        let v1 = payroll_engine::process_payroll(
            pool,
            company_id,
            group_id,
            year,
            month,
            pay_date,
            payroll_admin_id,
            Some(format!("{notes} (v1 initial batch)")),
            None,
        )
        .await?;
        payroll_lifecycle_service::cancel(
            pool,
            company_id,
            v1.id,
            &admin_auth,
            Some("Wrong allowance classification in initial batch".into()),
            None,
        )
        .await?;
    }

    let run = payroll_engine::process_payroll(
        pool,
        company_id,
        group_id,
        year,
        month,
        pay_date,
        payroll_admin_id,
        Some(notes.to_string()),
        None,
    )
    .await?;

    match target_status {
        "processed" => Ok(run.id),
        "pending_approval" => {
            payroll_lifecycle_service::submit_for_approval(
                pool,
                company_id,
                run.id,
                payroll_admin_id,
                None,
            )
            .await?;
            Ok(run.id)
        }
        "approved" => {
            payroll_lifecycle_service::submit_for_approval(
                pool,
                company_id,
                run.id,
                payroll_admin_id,
                None,
            )
            .await?;
            payroll_lifecycle_service::approve(pool, company_id, run.id, &finance_auth, None)
                .await?;
            Ok(run.id)
        }
        "paid" | "cancel_then_paid" => {
            payroll_lifecycle_service::submit_for_approval(
                pool,
                company_id,
                run.id,
                payroll_admin_id,
                None,
            )
            .await?;
            payroll_lifecycle_service::approve(pool, company_id, run.id, &finance_auth, None)
                .await?;
            payroll_lifecycle_service::lock_as_paid(pool, company_id, run.id, finance_id, None)
                .await?;
            Ok(run.id)
        }
        other => Err(format!("Unsupported target payroll status: {other}").into()),
    }
}

async fn seed_perf_scale(
    pool: &PgPool,
    company_id: Uuid,
    payroll_group_id: Uuid,
    annual_lt_id: Uuid,
    user_id: Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Generating --scale=perf benchmark data for Aurelia Tech...");
    let already: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM employees WHERE company_id = $1 AND employee_number LIKE 'PERF-%'",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await?;
    if already >= 500 {
        println!("Perf scale rows already present ({already} PERF-* employees); skipping.");
        return Ok(());
    }

    // 1. 500 employees
    let mut emp_ids = Vec::with_capacity(500);
    let mut emp_nos = Vec::with_capacity(500);
    let mut full_names = Vec::with_capacity(500);
    let mut depts = Vec::with_capacity(500);
    let mut emails = Vec::with_capacity(500);
    let mut salaries = Vec::with_capacity(500);

    let dept_pool = [
        "Engineering",
        "Operations",
        "Sales",
        "Customer Success",
        "Finance",
    ];
    for i in 1..=500_usize {
        let eid = det_id(&format!("perf-emp:{i:04}"));
        emp_ids.push(eid);
        emp_nos.push(format!("PERF-{i:04}"));
        full_names.push(format!("Perf Benchmark Employee {i:04}"));
        depts.push(dept_pool[i % dept_pool.len()].to_string());
        emails.push(format!("perf-{i:04}@aureliatech.test"));
        salaries.push(250_000_i64 + ((i as i64 % 30) * 25_000));
    }

    sqlx::query(
        r#"
        INSERT INTO employees (
            id, company_id, employee_number, full_name, date_of_birth,
            gender, nationality, race, residency_status, marital_status,
            email, department, designation, employment_type, date_joined,
            basic_salary, epf_category, payroll_group_id, is_active
        )
        SELECT u.id, $1, u.eno, u.fname, DATE '1991-05-15',
               'male'::gender_type, 'Malaysian', 'malay'::race_type, 'citizen'::residency_status, 'single'::marital_status,
               u.email, u.dept, 'Associate', 'permanent'::employment_type, DATE '2024-01-01',
               u.sal, 'A'::bpchar, $2, TRUE
        FROM UNNEST($3::uuid[], $4::text[], $5::text[], $6::text[], $7::text[], $8::bigint[])
             AS u(id, eno, fname, dept, email, sal)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(company_id)
    .bind(payroll_group_id)
    .bind(&emp_ids)
    .bind(&emp_nos)
    .bind(&full_names)
    .bind(&depts)
    .bind(&emails)
    .bind(&salaries)
    .execute(pool)
    .await?;

    // 2. 1,500 leave balances (3 years * 500)
    let mut lb_ids = Vec::with_capacity(1500);
    let mut lb_eids = Vec::with_capacity(1500);
    let mut lb_years = Vec::with_capacity(1500);
    for year in [2024, 2025, 2026] {
        for (i, eid) in emp_ids.iter().enumerate() {
            lb_ids.push(det_id(&format!("perf-lb:{year}:{i:04}")));
            lb_eids.push(*eid);
            lb_years.push(year);
        }
    }
    sqlx::query(
        r#"
        INSERT INTO leave_balances (id, employee_id, leave_type_id, year, entitled_days, taken_days, pending_days, carried_forward)
        SELECT u.id, u.eid, $1, u.yr, 14.0, 2.0, 1.0, 2.0
        FROM UNNEST($2::uuid[], $3::uuid[], $4::int[]) AS u(id, eid, yr)
        ON CONFLICT (employee_id, leave_type_id, year) DO NOTHING
        "#,
    )
    .bind(annual_lt_id)
    .bind(&lb_ids)
    .bind(&lb_eids)
    .bind(&lb_years)
    .execute(pool)
    .await?;

    // 3. 15,000 attendance_records (30 days * 500 employees)
    let base_day = Utc.with_ymd_and_hms(2026, 10, 1, 1, 0, 0).unwrap();
    for day in 0..30_i64 {
        let mut ar_ids = Vec::with_capacity(500);
        let mut ar_eids = Vec::with_capacity(500);
        let mut ar_cin = Vec::with_capacity(500);
        let mut ar_cout = Vec::with_capacity(500);
        let mut ar_stat = Vec::with_capacity(500);
        let mut ar_hw = Vec::with_capacity(500);
        let mut ar_ot = Vec::with_capacity(500);
        for (i, eid) in emp_ids.iter().enumerate() {
            ar_ids.push(det_id(&format!("perf-att:{day}:{i:04}")));
            ar_eids.push(*eid);
            let cin =
                base_day + ChronoDuration::days(day) + ChronoDuration::minutes((i % 25) as i64);
            let cout = cin
                + ChronoDuration::hours(9)
                + ChronoDuration::minutes(if i % 5 == 0 { 60 } else { 0 });
            ar_cin.push(cin);
            ar_cout.push(cout);
            ar_stat.push(if i % 7 == 0 { "late" } else { "present" }.to_string());
            ar_hw.push(if i % 5 == 0 { dec!(9.0) } else { dec!(8.0) });
            ar_ot.push(if i % 5 == 0 { dec!(1.0) } else { dec!(0.0) });
        }
        sqlx::query(
            r#"
            INSERT INTO attendance_records (
                id, company_id, employee_id, check_in_at, check_out_at, method,
                status, hours_worked, overtime_hours, is_outside_geofence
            )
            SELECT u.id, $1, u.eid, u.cin, u.cout, 'qr_code', u.st, u.hw, u.ot, FALSE
            FROM UNNEST($2::uuid[], $3::uuid[], $4::timestamptz[], $5::timestamptz[], $6::text[], $7::numeric[], $8::numeric[])
                 AS u(id, eid, cin, cout, st, hw, ot)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(company_id)
        .bind(&ar_ids)
        .bind(&ar_eids)
        .bind(&ar_cin)
        .bind(&ar_cout)
        .bind(&ar_stat)
        .bind(&ar_hw)
        .bind(&ar_ot)
        .execute(pool)
        .await?;
    }

    // 4. 2,000 leave_requests, 2,000 claims, 2,000 overtime_applications, 2,000 payroll_entries
    let lr_statuses = ["pending", "approved", "rejected", "cancelled"];
    let cl_statuses = [
        "draft",
        "pending",
        "approved",
        "rejected",
        "processed",
        "cancelled",
    ];
    let mut lr_ids = Vec::with_capacity(2000);
    let mut lr_eids = Vec::with_capacity(2000);
    let mut lr_sdate = Vec::with_capacity(2000);
    let mut lr_st = Vec::with_capacity(2000);

    let mut cl_ids = Vec::with_capacity(2000);
    let mut cl_amt = Vec::with_capacity(2000);
    let mut cl_date = Vec::with_capacity(2000);
    let mut cl_st = Vec::with_capacity(2000);

    let mut ot_ids = Vec::with_capacity(2000);
    let mut ot_date = Vec::with_capacity(2000);
    let mut ot_st = Vec::with_capacity(2000);

    let mut pe_ids = Vec::with_capacity(2000);
    let mut pe_amt = Vec::with_capacity(2000);

    let jan1 = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
    for i in 0..2000_usize {
        let eid = emp_ids[i % 500];
        let offset = (i % 280) as i64;
        lr_ids.push(det_id(&format!("perf-lr:{i}")));
        lr_eids.push(eid);
        lr_sdate.push(jan1 + ChronoDuration::days(offset));
        lr_st.push(lr_statuses[i % lr_statuses.len()].to_string());

        cl_ids.push(det_id(&format!("perf-cl:{i}")));
        cl_amt.push(5_000 + ((i as i64 % 25) * 1_500));
        cl_date.push(jan1 + ChronoDuration::days(offset));
        cl_st.push(cl_statuses[i % cl_statuses.len()].to_string());

        ot_ids.push(det_id(&format!("perf-ot:{i}")));
        ot_date.push(jan1 + ChronoDuration::days(offset));
        ot_st.push(lr_statuses[i % lr_statuses.len()].to_string());

        pe_ids.push(det_id(&format!("perf-pe:{i}")));
        pe_amt.push(15_000 + ((i as i64 % 20) * 2_500));
    }

    sqlx::query(
        r#"
        INSERT INTO leave_requests (id, employee_id, company_id, leave_type_id, start_date, end_date, days, reason, status, created_by)
        SELECT u.id, u.eid, $1, $2, u.d, u.d, 1.0, 'Perf test leave', u.st, $7
        FROM UNNEST($3::uuid[], $4::uuid[], $5::date[], $6::text[]) AS u(id, eid, d, st)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(company_id)
    .bind(annual_lt_id)
    .bind(&lr_ids)
    .bind(&lr_eids)
    .bind(&lr_sdate)
    .bind(&lr_st)
    .bind(user_id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO claims (id, employee_id, company_id, title, amount, category, expense_date, status, created_by)
        SELECT u.id, u.eid, $1, 'Travel claim', u.amt, 'travel', u.d, u.st, $7
        FROM UNNEST($2::uuid[], $3::uuid[], $4::bigint[], $5::date[], $6::text[]) AS u(id, eid, amt, d, st)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(company_id)
    .bind(&cl_ids)
    .bind(&lr_eids)
    .bind(&cl_amt)
    .bind(&cl_date)
    .bind(&cl_st)
    .bind(user_id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO overtime_applications (id, employee_id, company_id, ot_date, start_time, end_time, hours, ot_type, status, created_by)
        SELECT u.id, u.eid, $1, u.d, TIME '18:30:00', TIME '20:30:00', 2.0, 'normal', u.st, $6
        FROM UNNEST($2::uuid[], $3::uuid[], $4::date[], $5::text[]) AS u(id, eid, d, st)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(company_id)
    .bind(&ot_ids)
    .bind(&lr_eids)
    .bind(&ot_date)
    .bind(&ot_st)
    .bind(user_id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO payroll_entries (
            id, employee_id, company_id, period_year, period_month,
            category, item_type, description, amount, is_taxable, is_processed
        )
        SELECT u.id, u.eid, $1, 2026, 12, 'earning', 'bonus', 'Perf Staged Bonus', u.amt, TRUE, FALSE
        FROM UNNEST($2::uuid[], $3::uuid[], $4::bigint[]) AS u(id, eid, amt)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(company_id)
    .bind(&pe_ids)
    .bind(&lr_eids)
    .bind(&pe_amt)
    .execute(pool)
    .await?;

    // 5. 5,000 notifications + 5,000 audit_logs
    let mut n_ids = Vec::with_capacity(5000);
    let mut n_read = Vec::with_capacity(5000);
    let mut a_ids = Vec::with_capacity(5000);
    for i in 0..5000_usize {
        n_ids.push(det_id(&format!("perf-notif:{i}")));
        n_read.push(i % 4 != 0);
        a_ids.push(det_id(&format!("perf-audit:{i}")));
    }

    sqlx::query(
        r#"
        INSERT INTO notifications (id, user_id, company_id, notification_type, title, message, is_read)
        SELECT u.id, $1, $2, 'system', 'Benchmark alert', 'Perf benchmark notification row', u.rd
        FROM UNNEST($3::uuid[], $4::bool[]) AS u(id, rd)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(company_id)
    .bind(&n_ids)
    .bind(&n_read)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO audit_logs (id, user_id, company_id, action, entity_type, description)
        SELECT u.id, $1, $2, 'benchmark_probe', 'employee', '[dev-seed] Perf benchmark audit row'
        FROM UNNEST($3::uuid[]) AS u(id)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(company_id)
    .bind(&a_ids)
    .execute(pool)
    .await?;

    println!("Perf scale insertion complete.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let opts = parse_args().unwrap_or_else(|e| {
        eprintln!("Argument error: {e}");
        process::exit(1);
    });

    let database_url =
        env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set in environment or .env")?;
    if let Err(msg) = enforce_non_production_guards(&database_url) {
        eprintln!("{msg}");
        process::exit(1);
    }

    let totp_key = env::var("TOTP_ENCRYPTION_KEY")
        .unwrap_or_else(|_| "dev-seed-totp-key-only-for-local-seeding".to_string());

    let pool = create_pool(&database_url).await;
    run_migrations(&pool).await;

    let allow_override = env::var("ALLOW_DEV_SEED")
        .map(|v| v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !allow_override {
        let non_seed_companies: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM companies WHERE NOT (name = ANY($1)) AND name NOT LIKE 'test-fixture-%' AND name NOT LIKE 'legacy-prototype-%'",
        )
        .bind(SEEDED_COMPANY_NAMES)
        .fetch_one(&pool)
        .await?;
        if non_seed_companies > 0 {
            return Err(format!(
                "Refusing to seed: database contains {non_seed_companies} non-seed company record(s). Set ALLOW_DEV_SEED=true if intentional on a dev database."
            )
            .into());
        }
    }

    if opts.purge_test_junk {
        purge_test_junk(&pool).await?;
    }
    if opts.reset {
        reset_seeded_data(&pool).await?;
    }

    // 1. Platform + 2026 Verified Statutory Tables
    seed_statutory_2026(&pool).await?;

    // Compute ONE bcrypt hash (cost 10) for fast local dev login across seeded accounts.
    let pw_hash = bcrypt::hash(SEED_PASSWORD_PLAIN, 10)?;

    // 2. Three Multi-Tenant Companies
    let c_kl = insert_company_with_defaults(
        &pool,
        "kl",
        "Aurelia Tech Sdn Bhd",
        "202101008812",
        "Kuala Lumpur",
        "WP Kuala Lumpur",
        "Malaysia",
        "Asia/Kuala_Lumpur",
        "enforce",
        "enforce",
        "qr_code",
        true,
        26,
        Some(NaiveDate::from_ymd_opt(2026, 11, 25).unwrap()),
        NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
        NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
        [false, true, true, true, true, true, false], // Mon-Fri
    )
    .await?;

    let c_jb = insert_company_with_defaults(
        &pool,
        "jb",
        "Selatan Manufacturing Sdn Bhd",
        "201901004421",
        "Johor Bahru",
        "Johor",
        "Malaysia",
        "Asia/Kuala_Lumpur",
        "warn",
        "learn",
        "face_id",
        true,
        22,
        Some(NaiveDate::from_ymd_opt(2026, 11, 25).unwrap()),
        NaiveTime::from_hms_opt(22, 0, 0).unwrap(),
        NaiveTime::from_hms_opt(7, 0, 0).unwrap(), // overnight factory shift
        [true, true, true, true, true, false, false], // Sun-Thu Johor workweek
    )
    .await?;

    let c_sg = insert_company_with_defaults(
        &pool,
        "sg",
        "Merlion Digital Pte Ltd",
        "20220091882Z",
        "Singapore",
        "Singapore",
        "Singapore",
        "Asia/Singapore",
        "none",
        "warn",
        "qr_code",
        false,
        22,
        Some(NaiveDate::from_ymd_opt(2026, 11, 25).unwrap()),
        NaiveTime::from_hms_opt(8, 30, 0).unwrap(),
        NaiveTime::from_hms_opt(17, 30, 0).unwrap(),
        [false, true, true, true, true, true, false],
    )
    .await?;

    // 3. Global & Admin Role Users (all 8 canonical roles)
    let all_companies = [c_kl, c_jb, c_sg];
    let super_admin_id = insert_user(
        &pool,
        "superadmin",
        "superadmin@payrollmy.test",
        &pw_hash,
        "Platform Super Admin",
        None,
        None,
        &["super_admin"],
        true,
        false,
        false,
        &all_companies,
    )
    .await?;

    let kl_admin_id = insert_user(
        &pool,
        "kl-admin",
        "admin@aureliatech.test",
        &pw_hash,
        "Aurelia Admin",
        Some(c_kl),
        None,
        &["admin"],
        true,
        false,
        false,
        &[c_kl],
    )
    .await?;

    let kl_payroll_admin_id = insert_user(
        &pool,
        "kl-payroll-admin",
        "payroll@aureliatech.test",
        &pw_hash,
        "Siti Payroll Admin",
        Some(c_kl),
        None,
        &["payroll_admin"],
        true,
        false,
        false,
        &[c_kl, c_jb, c_sg],
    )
    .await?;

    let kl_hr_manager_id = insert_user(
        &pool,
        "kl-hr",
        "hr@aureliatech.test",
        &pw_hash,
        "Nurul HR Manager",
        Some(c_kl),
        None,
        &["hr_manager"],
        true,
        false,
        false,
        &[c_kl],
    )
    .await?;

    let kl_finance_id = insert_user(
        &pool,
        "kl-finance",
        "finance@aureliatech.test",
        &pw_hash,
        "Daniel Finance Controller",
        Some(c_kl),
        None,
        &["finance"],
        true,
        false,
        false,
        &all_companies,
    )
    .await?;

    let _kl_exec_id = insert_user(
        &pool,
        "kl-exec",
        "exec@aureliatech.test",
        &pw_hash,
        "Datuk Amir Executive",
        Some(c_kl),
        None,
        &["exec"],
        true,
        false,
        false,
        &all_companies,
    )
    .await?;

    let _must_change_user_id = insert_user(
        &pool,
        "kl-mustchange",
        "newbie@aureliatech.test",
        &pw_hash,
        "New Hire Force Password Change",
        Some(c_kl),
        None,
        &["manager"],
        true,
        true,
        false,
        &[c_kl],
    )
    .await?;

    let inactive_sa_id = insert_user(
        &pool,
        "inactive-sa",
        "inactive.sa@payrollmy.test",
        &pw_hash,
        "Inactive Super Admin",
        Some(c_kl),
        None,
        &["super_admin"],
        false,
        false,
        false,
        &[c_kl],
    )
    .await?;

    let _soft_deleted_user_id = insert_user(
        &pool,
        "deleted-user",
        "departed.ops@aureliatech.test",
        &pw_hash,
        "Departed User Account",
        Some(c_kl),
        None,
        &["manager"],
        false,
        false,
        true,
        &[c_kl],
    )
    .await?;

    let jb_admin_id = insert_user(
        &pool,
        "jb-admin",
        "admin@selatanmfg.test",
        &pw_hash,
        "Selatan Plant Admin",
        Some(c_jb),
        None,
        &["admin", "payroll_admin"],
        true,
        false,
        false,
        &[c_jb],
    )
    .await?;

    let sg_admin_id = insert_user(
        &pool,
        "sg-admin",
        "admin@merliondigital.test",
        &pw_hash,
        "Merlion Director",
        Some(c_sg),
        None,
        &["admin", "payroll_admin"],
        true,
        false,
        false,
        &[c_sg],
    )
    .await?;

    // 4. Payroll Groups (clean payable vs isolated fail-closed group)
    let pg_kl_exec = ensure_group(
        &pool,
        "kl-exec",
        c_kl,
        "Monthly Executive",
        "Primary Malaysian payroll group for monthly salaried staff",
    )
    .await?;
    let pg_kl_ops = ensure_group(
        &pool,
        "kl-ops",
        c_kl,
        "Operations",
        "Contract, intern, daily and hourly operations staff",
    )
    .await?;
    let pg_kl_edge = ensure_group(
        &pool,
        "kl-edge",
        c_kl,
        "Edge Cases / Manual Review",
        "Fail-closed statutory archetypes excluded from automatic batch runs",
    )
    .await?;
    let pg_jb_shift = ensure_group(
        &pool,
        "jb-shift",
        c_jb,
        "Plant Shift",
        "24x7 overnight manufacturing group",
    )
    .await?;
    let pg_jb_mgmt = ensure_group(
        &pool,
        "jb-mgmt",
        c_jb,
        "Management",
        "Selatan management payroll",
    )
    .await?;
    let pg_sg_core =
        ensure_group(&pool, "sg-core", c_sg, "SG Core", "Merlion Singapore staff").await?;

    // 5. Company Locations & Networks (Geofencing + IP binding)
    for (loc_key, cid, name, lat, lng, radius, active) in [
        ("kl-hq", c_kl, "KLCC Menara HQ", 3.1578, 101.7119, 250, true),
        ("kl-pj", c_kl, "PJ Jaya33 Hub", 3.1095, 101.6373, 200, true),
        (
            "kl-old",
            c_kl,
            "Old Damansara Office",
            3.1360,
            101.6290,
            150,
            false,
        ),
        (
            "jb-plant",
            c_jb,
            "Pasir Gudang Plant",
            1.4726,
            103.9029,
            300,
            true,
        ),
        (
            "sg-cbd",
            c_sg,
            "Raffles Place Hub",
            1.2842,
            103.8515,
            150,
            true,
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO company_locations (id, company_id, name, latitude, longitude, radius_meters, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(det_id(&format!("loc:{loc_key}")))
        .bind(cid)
        .bind(name)
        .bind(lat)
        .bind(lng)
        .bind(radius)
        .bind(active)
        .execute(&pool)
        .await?;
    }

    for (nkey, cid, label, net, plen, actor, learned) in [
        (
            "kl-fiber",
            c_kl,
            "KL HQ Fiber Gateway",
            "203.0.113.0",
            24_i16,
            kl_admin_id,
            false,
        ),
        (
            "kl-pj",
            c_kl,
            "PJ Branch Corp VPN",
            "198.51.100.0",
            24_i16,
            kl_admin_id,
            true,
        ),
        (
            "jb-gw",
            c_jb,
            "Pasir Gudang ISP",
            "203.0.114.0",
            24_i16,
            jb_admin_id,
            false,
        ),
        (
            "sg-gw",
            c_sg,
            "Raffles Fibre",
            "198.51.101.0",
            24_i16,
            sg_admin_id,
            false,
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO company_networks (id, company_id, label, network, prefix_len, approved_by, learned_from_observation)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (company_id, network, prefix_len) DO NOTHING
            "#,
        )
        .bind(det_id(&format!("net:{nkey}")))
        .bind(cid)
        .bind(label)
        .bind(net)
        .bind(plen)
        .bind(actor)
        .bind(learned)
        .execute(&pool)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO attendance_network_dismissals (id, company_id, network, prefix_len, dismissed_by)
        VALUES ($1, $2, '192.0.2.0', 24, $3)
        ON CONFLICT (company_id, network, prefix_len) DO NOTHING
        "#,
    )
    .bind(det_id("net-dismiss:kl"))
    .bind(c_kl)
    .bind(kl_admin_id)
    .execute(&pool)
    .await?;

    // 6. Leave Types (for all 3 companies)
    let kl_lt_annual = ensure_leave_type(
        &pool,
        "kl",
        c_kl,
        "Annual Leave",
        dec!(16),
        true,
        dec!(5),
        3,
        true,
    )
    .await?;
    let kl_lt_medical = ensure_leave_type(
        &pool,
        "kl",
        c_kl,
        "Medical Leave",
        dec!(14),
        true,
        dec!(0),
        0,
        true,
    )
    .await?;
    let kl_lt_unpaid = ensure_leave_type(
        &pool,
        "kl",
        c_kl,
        "Unpaid Leave",
        dec!(30),
        false,
        dec!(0),
        0,
        false,
    )
    .await?;
    let kl_lt_repl = ensure_leave_type(
        &pool,
        "kl",
        c_kl,
        "Replacement Leave",
        dec!(0),
        true,
        dec!(5),
        6,
        true,
    )
    .await?;
    let _kl_lt_mat = ensure_leave_type(
        &pool,
        "kl",
        c_kl,
        "Maternity Leave",
        dec!(98),
        true,
        dec!(0),
        0,
        false,
    )
    .await?;
    let _kl_lt_pat = ensure_leave_type(
        &pool,
        "kl",
        c_kl,
        "Paternity Leave",
        dec!(7),
        true,
        dec!(0),
        0,
        false,
    )
    .await?;

    let jb_lt_annual = ensure_leave_type(
        &pool,
        "jb",
        c_jb,
        "Annual Leave",
        dec!(14),
        true,
        dec!(5),
        3,
        true,
    )
    .await?;
    let _jb_lt_repl = ensure_leave_type(
        &pool,
        "jb",
        c_jb,
        "Replacement Leave",
        dec!(0),
        true,
        dec!(3),
        3,
        true,
    )
    .await?;
    let sg_lt_annual = ensure_leave_type(
        &pool,
        "sg",
        c_sg,
        "Annual Leave",
        dec!(18),
        true,
        dec!(5),
        3,
        true,
    )
    .await?;

    // 7. Holidays (including earlier-year recurring + 2026 federal & state holidays)
    for (hkey, cid, name, date, htype, recurring, state) in [
        (
            "kl-ny",
            c_kl,
            "New Year's Day",
            NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            "public_holiday",
            true,
            None,
        ),
        (
            "kl-merdeka",
            c_kl,
            "Hari Kebangsaan (Merdeka)",
            NaiveDate::from_ymd_opt(2020, 8, 31).unwrap(),
            "public_holiday",
            true,
            None,
        ),
        (
            "kl-fed",
            c_kl,
            "Federal Territory Day",
            NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            "state_holiday",
            false,
            Some("WP Kuala Lumpur"),
        ),
        (
            "kl-raya",
            c_kl,
            "Hari Raya Aidilfitri",
            NaiveDate::from_ymd_opt(2026, 3, 20).unwrap(),
            "public_holiday",
            false,
            None,
        ),
        (
            "kl-co",
            c_kl,
            "Aurelia Founders Day",
            NaiveDate::from_ymd_opt(2026, 12, 24).unwrap(),
            "company_holiday",
            false,
            None,
        ),
        (
            "jb-sultan",
            c_jb,
            "Sultan of Johor Birthday",
            NaiveDate::from_ymd_opt(2026, 3, 23).unwrap(),
            "state_holiday",
            false,
            Some("Johor"),
        ),
        (
            "jb-merdeka",
            c_jb,
            "Hari Kebangsaan",
            NaiveDate::from_ymd_opt(2020, 8, 31).unwrap(),
            "public_holiday",
            true,
            None,
        ),
        (
            "sg-nd",
            c_sg,
            "Singapore National Day",
            NaiveDate::from_ymd_opt(2020, 8, 9).unwrap(),
            "public_holiday",
            true,
            None,
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO holidays (id, company_id, name, date, holiday_type, is_recurring, state, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(det_id(&format!("hol:{hkey}")))
        .bind(cid)
        .bind(name)
        .bind(date)
        .bind(htype)
        .bind(recurring)
        .bind(state)
        .bind(if cid == c_kl { kl_admin_id } else if cid == c_jb { jb_admin_id } else { sg_admin_id })
        .execute(&pool)
        .await?;
    }

    // 8. Employees covering all Malaysian statutory & employment archetypes
    // --- Company 1: Monthly Executive (clean payable across 2026) ---
    let emp_std = insert_employee(
        &pool,
        "kl-std",
        c_kl,
        Some(pg_kl_exec),
        "AT-0001",
        "Aisyah Binti Ahmad",
        NaiveDate::from_ymd_opt(1992, 3, 15),
        "female",
        "malay",
        "Malaysian",
        "citizen",
        "single",
        false,
        0,
        Some("A"),
        true,
        0,
        15_000,
        10_000,
        "Engineering",
        "Senior Backend Engineer",
        "permanent",
        NaiveDate::from_ymd_opt(2022, 1, 10).unwrap(),
        None,
        None,
        None,
        None,
        450_000,
        None,
        None,
        "1140-9001-0001",
        true,
        "aisyah@aureliatech.test",
    )
    .await?;

    let emp_min_wage = insert_employee(
        &pool,
        "kl-min",
        c_kl,
        Some(pg_kl_exec),
        "AT-0002",
        "Kumar A/L Subramaniam",
        NaiveDate::from_ymd_opt(1999, 7, 10),
        "male",
        "indian",
        "Malaysian",
        "citizen",
        "single",
        false,
        0,
        Some("A"),
        false,
        0,
        0,
        0,
        "Support",
        "Junior Desk Coordinator",
        "permanent",
        NaiveDate::from_ymd_opt(2024, 4, 1).unwrap(),
        None,
        None,
        None,
        None,
        150_000,
        None,
        None,
        "1140-9001-0002",
        true,
        "kumar@aureliatech.test",
    )
    .await?;

    let emp_married_zakat = insert_employee(
        &pool,
        "kl-zakat",
        c_kl,
        Some(pg_kl_exec),
        "AT-0003",
        "Farid Bin Razak",
        NaiveDate::from_ymd_opt(1987, 11, 2),
        "male",
        "malay",
        "Malaysian",
        "citizen",
        "married",
        true,
        3,
        Some("A"),
        true,
        12_000,
        0,
        20_000,
        "Product",
        "Engineering Manager",
        "permanent",
        NaiveDate::from_ymd_opt(2020, 6, 1).unwrap(),
        None,
        None,
        None,
        None,
        850_000,
        None,
        None,
        "1140-9001-0003",
        true,
        "farid@aureliatech.test",
    )
    .await?;

    let emp_tp3_inc = insert_employee(
        &pool,
        "kl-tp3",
        c_kl,
        Some(pg_kl_exec),
        "AT-0004",
        "Mei Ling Tan",
        NaiveDate::from_ymd_opt(1989, 4, 20),
        "female",
        "chinese",
        "Malaysian",
        "citizen",
        "married",
        false,
        1,
        Some("A"),
        false,
        0,
        0,
        0,
        "Finance",
        "Lead Financial Analyst",
        "permanent",
        NaiveDate::from_ymd_opt(2026, 1, 2).unwrap(),
        None,
        None,
        None,
        None,
        1_200_000,
        None,
        None,
        "1140-9001-0004",
        true,
        "meiling@aureliatech.test",
    )
    .await?;

    let emp_high = insert_employee(
        &pool,
        "kl-high",
        c_kl,
        Some(pg_kl_exec),
        "AT-0005",
        "Jason Lim Wei Sheng",
        NaiveDate::from_ymd_opt(1984, 1, 12),
        "male",
        "chinese",
        "Malaysian",
        "citizen",
        "married",
        true,
        2,
        Some("A"),
        false,
        0,
        0,
        0,
        "Executive",
        "VP Engineering",
        "permanent",
        NaiveDate::from_ymd_opt(2019, 2, 1).unwrap(),
        None,
        None,
        None,
        None,
        2_800_000,
        None,
        None,
        "1140-9001-0005-OLD",
        true,
        "jason.lim@aureliatech.test",
    )
    .await?;

    let _emp_age62_part_e = insert_employee(
        &pool,
        "kl-age62",
        c_kl,
        Some(pg_kl_exec),
        "AT-0006",
        "Haji Ismail Bin Yusof",
        NaiveDate::from_ymd_opt(1964, 2, 1),
        "male",
        "malay",
        "Malaysian",
        "citizen",
        "married",
        false,
        0,
        Some("E"),
        true,
        5_000,
        0,
        0,
        "Advisory",
        "Senior Principal Advisor",
        "permanent",
        NaiveDate::from_ymd_opt(2016, 1, 1).unwrap(),
        None,
        None,
        None,
        None,
        650_000,
        None,
        None,
        "1140-9001-0006",
        true,
        "ismail@aureliatech.test",
    )
    .await?;

    let _emp_age65_part_c_pr = insert_employee(
        &pool,
        "kl-pr65",
        c_kl,
        Some(pg_kl_exec),
        "AT-0007",
        "Anita Sharma",
        NaiveDate::from_ymd_opt(1961, 1, 5),
        "female",
        "indian",
        "Indian",
        "permanent_resident",
        "widowed",
        false,
        0,
        Some("C"),
        false,
        0,
        0,
        0,
        "Legal",
        "Compliance Counsel",
        "permanent",
        NaiveDate::from_ymd_opt(2018, 3, 1).unwrap(),
        None,
        None,
        None,
        None,
        750_000,
        None,
        None,
        "1140-9001-0007",
        true,
        "anita@aureliatech.test",
    )
    .await?;

    let _emp_age76_cessation = insert_employee(
        &pool,
        "kl-age76",
        c_kl,
        Some(pg_kl_exec),
        "AT-0008",
        "Dr. Wong Kok Wing",
        NaiveDate::from_ymd_opt(1950, 1, 10),
        "male",
        "chinese",
        "Malaysian",
        "citizen",
        "married",
        false,
        0,
        Some("E"),
        false,
        0,
        0,
        0,
        "Advisory",
        "Board Fellow",
        "permanent",
        NaiveDate::from_ymd_opt(2015, 1, 1).unwrap(),
        None,
        None,
        None,
        None,
        500_000,
        None,
        None,
        "1140-9001-0008",
        true,
        "dr.wong@aureliatech.test",
    )
    .await?;

    let _emp_mid_join = insert_employee(
        &pool,
        "kl-midjoin",
        c_kl,
        Some(pg_kl_exec),
        "AT-0009",
        "Siti Sarah Zainal",
        NaiveDate::from_ymd_opt(1995, 8, 18),
        "female",
        "malay",
        "Malaysian",
        "citizen",
        "single",
        false,
        0,
        Some("A"),
        true,
        0,
        0,
        0,
        "Engineering",
        "QA Automation Engineer",
        "permanent",
        NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
        None,
        None,
        None,
        None,
        420_000,
        None,
        None,
        "1140-9001-0009",
        true,
        "sarah@aureliatech.test",
    )
    .await?;

    let _emp_mid_resign = insert_employee(
        &pool,
        "kl-resign",
        c_kl,
        Some(pg_kl_exec),
        "AT-0010",
        "Hafizuddin Ali",
        NaiveDate::from_ymd_opt(1993, 9, 25),
        "male",
        "malay",
        "Malaysian",
        "citizen",
        "single",
        false,
        0,
        Some("A"),
        true,
        0,
        0,
        0,
        "Engineering",
        "DevOps Engineer",
        "permanent",
        NaiveDate::from_ymd_opt(2023, 2, 1).unwrap(),
        None,
        None,
        Some(NaiveDate::from_ymd_opt(2026, 8, 20).unwrap()),
        Some("Relocated overseas"),
        520_000,
        None,
        None,
        "1140-9001-0010",
        true,
        "hafiz@aureliatech.test",
    )
    .await?;

    let emp_q1_deleted = insert_employee(
        &pool,
        "kl-q1del",
        c_kl,
        Some(pg_kl_exec),
        "AT-0011",
        "Priya Nair",
        NaiveDate::from_ymd_opt(1994, 5, 12),
        "female",
        "indian",
        "Malaysian",
        "citizen",
        "single",
        false,
        0,
        Some("A"),
        false,
        0,
        0,
        0,
        "Design",
        "UX Researcher",
        "permanent",
        NaiveDate::from_ymd_opt(2022, 5, 1).unwrap(),
        None,
        None,
        Some(NaiveDate::from_ymd_opt(2026, 3, 31).unwrap()),
        Some("End of term"),
        480_000,
        None,
        None,
        "1140-9001-0011",
        true,
        "priya.nair@aureliatech.test",
    )
    .await?;

    // --- Company 1: Operations Group (contract, intern, daily, hourly, probationer) ---
    let emp_contract = insert_employee(
        &pool,
        "kl-contract",
        c_kl,
        Some(pg_kl_ops),
        "AT-0012",
        "Nadia Chong",
        NaiveDate::from_ymd_opt(1996, 10, 4),
        "female",
        "chinese",
        "Malaysian",
        "citizen",
        "single",
        false,
        0,
        Some("A"),
        false,
        0,
        0,
        0,
        "Operations",
        "Implementation Consultant",
        "contract",
        NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
        Some(NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()),
        Some(NaiveDate::from_ymd_opt(2026, 12, 15).unwrap()),
        None,
        None,
        380_000,
        None,
        None,
        "1140-9001-0012",
        true,
        "nadia@aureliatech.test",
    )
    .await?;

    let _emp_intern = insert_employee(
        &pool,
        "kl-intern",
        c_kl,
        Some(pg_kl_ops),
        "AT-0013",
        "Danial Haikal",
        NaiveDate::from_ymd_opt(2003, 12, 19),
        "male",
        "malay",
        "Malaysian",
        "citizen",
        "single",
        false,
        0,
        Some("A"),
        true,
        0,
        0,
        0,
        "Engineering",
        "Software Engineering Intern",
        "intern",
        NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
        None,
        None,
        None,
        None,
        120_000,
        None,
        None,
        "1140-9001-0013",
        true,
        "danial@aureliatech.test",
    )
    .await?;

    let _emp_daily = insert_employee(
        &pool,
        "kl-daily",
        c_kl,
        Some(pg_kl_ops),
        "AT-0014",
        "Muthu Veloo",
        NaiveDate::from_ymd_opt(1991, 6, 8),
        "male",
        "indian",
        "Malaysian",
        "citizen",
        "married",
        false,
        2,
        Some("A"),
        false,
        0,
        0,
        0,
        "Operations",
        "Field Technician",
        "daily_rated",
        NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
        None,
        None,
        None,
        None,
        240_000,
        None,
        Some(11_000),
        "1140-9001-0014",
        true,
        "muthu@aureliatech.test",
    )
    .await?;

    let _emp_hourly = insert_employee(
        &pool,
        "kl-hourly",
        c_kl,
        Some(pg_kl_ops),
        "AT-0015",
        "Joyce Yap",
        NaiveDate::from_ymd_opt(1998, 2, 22),
        "female",
        "chinese",
        "Malaysian",
        "citizen",
        "single",
        false,
        0,
        Some("F"),
        false,
        0,
        0,
        0,
        "Support",
        "Weekend Concierge",
        "hourly_rated",
        NaiveDate::from_ymd_opt(2025, 5, 15).unwrap(),
        None,
        None,
        None,
        None,
        220_000,
        Some(2_500),
        None,
        "1140-9001-0015",
        true,
        "joyce@aureliatech.test",
    )
    .await?;

    // --- Company 1: Isolated Fail-Closed Edge Cases / Manual Review Group ---
    let _emp_foreigner = insert_employee(
        &pool,
        "kl-foreigner",
        c_kl,
        Some(pg_kl_edge),
        "AT-9001",
        "Arjun Pratama",
        NaiveDate::from_ymd_opt(1993, 4, 9),
        "male",
        "other",
        "Indonesian",
        "foreigner",
        "single",
        false,
        0,
        Some("B"),
        true,
        0,
        0,
        0,
        "Engineering",
        "Visiting Solutions Architect",
        "contract",
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
        None,
        None,
        None,
        None,
        620_000,
        None,
        None,
        "1140-9900-0001",
        true,
        "arjun@aureliatech.test",
    )
    .await?;

    let _emp_age57 = insert_employee(
        &pool,
        "kl-age57",
        c_kl,
        Some(pg_kl_edge),
        "AT-9002",
        "Shahrul Nizam",
        NaiveDate::from_ymd_opt(1969, 1, 15),
        "male",
        "malay",
        "Malaysian",
        "citizen",
        "married",
        false,
        0,
        Some("A"),
        true,
        0,
        0,
        0,
        "Operations",
        "Fleet Supervisor",
        "permanent",
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        None,
        None,
        None,
        None,
        480_000,
        None,
        None,
        "1140-9900-0002",
        true,
        "shahrul@aureliatech.test",
    )
    .await?;

    let _emp_null_dob = insert_employee(
        &pool,
        "kl-nulldob",
        c_kl,
        Some(pg_kl_edge),
        "AT-9003",
        "Missing DOB Profile",
        None,
        "female",
        "other",
        "Malaysian",
        "citizen",
        "single",
        false,
        0,
        Some("A"),
        false,
        0,
        0,
        0,
        "Operations",
        "Onboarding Draft",
        "permanent",
        NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
        None,
        None,
        None,
        None,
        300_000,
        None,
        None,
        "1140-9900-0003",
        true,
        "nulldob@aureliatech.test",
    )
    .await?;

    let _emp_inactive_no_resign = insert_employee(
        &pool,
        "kl-inact",
        c_kl,
        Some(pg_kl_edge),
        "AT-9004",
        "Suspended Without Resign Date",
        NaiveDate::from_ymd_opt(1990, 6, 1),
        "male",
        "malay",
        "Malaysian",
        "citizen",
        "single",
        false,
        0,
        Some("A"),
        false,
        0,
        0,
        0,
        "Operations",
        "Suspended Staff",
        "permanent",
        NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        None,
        None,
        None,
        None,
        320_000,
        None,
        None,
        "1140-9900-0004",
        false,
        "suspended@aureliatech.test",
    )
    .await?;

    let emp_neg_net = insert_employee(
        &pool,
        "kl-negnet",
        c_kl,
        Some(pg_kl_edge),
        "AT-9005",
        "Over-Deducted Staff",
        NaiveDate::from_ymd_opt(1995, 3, 3),
        "female",
        "chinese",
        "Malaysian",
        "citizen",
        "single",
        false,
        0,
        Some("A"),
        false,
        0,
        0,
        0,
        "Operations",
        "Manual Review Staff",
        "permanent",
        NaiveDate::from_ymd_opt(2025, 6, 1).unwrap(),
        None,
        None,
        None,
        None,
        100_000,
        None,
        None,
        "1140-9900-0005",
        true,
        "overdeduct@aureliatech.test",
    )
    .await?;

    // --- Company 2 (Selatan JB) & Company 3 (Merlion SG) Employees ---
    let emp_jb_shift1 = insert_employee(
        &pool,
        "jb-s1",
        c_jb,
        Some(pg_jb_shift),
        "SM-0001",
        "Zulhilmi Bin Bakar",
        NaiveDate::from_ymd_opt(1990, 9, 12),
        "male",
        "malay",
        "Malaysian",
        "citizen",
        "married",
        false,
        2,
        Some("A"),
        true,
        3_000,
        0,
        0,
        "Production",
        "Night Shift Lead",
        "permanent",
        NaiveDate::from_ymd_opt(2021, 5, 1).unwrap(),
        None,
        None,
        None,
        None,
        360_000,
        None,
        None,
        "2210-1001-0001",
        true,
        "zulhilmi@selatanmfg.test",
    )
    .await?;
    let emp_jb_shift2 = insert_employee(
        &pool,
        "jb-s2",
        c_jb,
        Some(pg_jb_shift),
        "SM-0002",
        "Suresh Krishnan",
        NaiveDate::from_ymd_opt(1994, 11, 20),
        "male",
        "indian",
        "Malaysian",
        "citizen",
        "single",
        false,
        0,
        Some("A"),
        false,
        0,
        0,
        0,
        "Production",
        "CNC Operator",
        "permanent",
        NaiveDate::from_ymd_opt(2023, 7, 1).unwrap(),
        None,
        None,
        None,
        None,
        290_000,
        None,
        None,
        "2210-1001-0002",
        true,
        "suresh@selatanmfg.test",
    )
    .await?;
    let _emp_jb_mgmt = insert_employee(
        &pool,
        "jb-mgmt",
        c_jb,
        Some(pg_jb_mgmt),
        "SM-0003",
        "Tan Bee Leng",
        NaiveDate::from_ymd_opt(1985, 4, 18),
        "female",
        "chinese",
        "Malaysian",
        "citizen",
        "married",
        true,
        1,
        Some("A"),
        false,
        0,
        0,
        0,
        "Management",
        "Plant Director",
        "permanent",
        NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
        None,
        None,
        None,
        None,
        950_000,
        None,
        None,
        "2210-1001-0003",
        true,
        "beeleng@selatanmfg.test",
    )
    .await?;

    let emp_sg1 = insert_employee(
        &pool,
        "sg-1",
        c_sg,
        Some(pg_sg_core),
        "MD-0001",
        "Marcus Tan",
        NaiveDate::from_ymd_opt(1988, 7, 14),
        "male",
        "chinese",
        "Singaporean",
        "citizen",
        "single",
        false,
        0,
        Some("A"),
        false,
        0,
        0,
        0,
        "Platform",
        "Staff SRE",
        "permanent",
        NaiveDate::from_ymd_opt(2022, 10, 1).unwrap(),
        None,
        None,
        None,
        None,
        1_100_000,
        None,
        None,
        "3310-8001-0001",
        true,
        "marcus@merliondigital.test",
    )
    .await?;
    let _emp_sg2 = insert_employee(
        &pool,
        "sg-2",
        c_sg,
        Some(pg_sg_core),
        "MD-0002",
        "Cheryl Koh",
        NaiveDate::from_ymd_opt(1993, 1, 29),
        "female",
        "chinese",
        "Singaporean",
        "citizen",
        "single",
        false,
        0,
        Some("A"),
        false,
        0,
        0,
        0,
        "Sales",
        "Regional AE",
        "permanent",
        NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(),
        None,
        None,
        None,
        None,
        780_000,
        None,
        None,
        "3310-8001-0002",
        true,
        "cheryl@merliondigital.test",
    )
    .await?;

    // Link manager & employee users to Employee records
    let kl_manager_id = insert_user(
        &pool,
        "kl-manager",
        "manager@aureliatech.test",
        &pw_hash,
        "Farid Manager",
        Some(c_kl),
        Some(emp_married_zakat),
        &["manager"],
        true,
        false,
        false,
        &[c_kl],
    )
    .await?;
    let kl_employee_user_id = insert_user(
        &pool,
        "kl-employee",
        "employee@aureliatech.test",
        &pw_hash,
        "Aisyah Binti Ahmad",
        Some(c_kl),
        Some(emp_std),
        &["employee"],
        true,
        false,
        false,
        &[c_kl],
    )
    .await?;
    let jb_employee_user_id = insert_user(
        &pool,
        "jb-employee",
        "zulhilmi@selatanmfg.test",
        &pw_hash,
        "Zulhilmi Bin Bakar",
        Some(c_jb),
        Some(emp_jb_shift1),
        &["employee"],
        true,
        false,
        false,
        &[c_jb],
    )
    .await?;
    let _sg_employee_user_id = insert_user(
        &pool,
        "sg-employee",
        "marcus@merliondigital.test",
        &pw_hash,
        "Marcus Tan",
        Some(c_sg),
        Some(emp_sg1),
        &["employee"],
        true,
        false,
        false,
        &[c_sg],
    )
    .await?;

    // 9. User Groups, Members (after user_companies!), Non-Platform Permissions
    let ug_kl_approvers = det_id("ug:kl-approvers");
    sqlx::query(
        r#"
        INSERT INTO user_groups (id, company_id, name, description, created_by)
        VALUES ($1, $2, 'Leave & Claim Reviewers', 'Additive approval permissions for managers', $3)
        ON CONFLICT (company_id, name) DO NOTHING
        "#,
    )
    .bind(ug_kl_approvers)
    .bind(c_kl)
    .bind(kl_admin_id)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO user_group_members (group_id, user_id, added_by) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(ug_kl_approvers)
    .bind(kl_manager_id)
    .bind(kl_admin_id)
    .execute(&pool)
    .await?;

    for perm in ["view_approvals", "manage_approvals", "view_employees"] {
        sqlx::query(
            "INSERT INTO user_group_permissions (group_id, permission, granted_by) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        )
        .bind(ug_kl_approvers)
        .bind(perm)
        .bind(kl_admin_id)
        .execute(&pool)
        .await?;
    }

    // 10. Auth & MFA State (TOTP, Backup Codes, Passkeys, OAuth2, Challenges, Sessions, Tokens)
    let totp_active_secret = encrypt_secret("JBSWY3DPEHPK3PXP", &totp_key)?;
    let totp_locked_secret = encrypt_secret("KRSXG5CTMVRXEZLU", &totp_key)?;
    let totp_pending_secret = encrypt_secret("MFRGGZDFMZTWQ2LK", &totp_key)?;

    for (ukey, uid, sec, enabled, confirmed, failed, locked_until) in [
        (
            "sa",
            super_admin_id,
            &totp_active_secret,
            true,
            true,
            0,
            None,
        ),
        (
            "fin",
            kl_finance_id,
            &totp_locked_secret,
            true,
            true,
            5,
            Some(Utc::now() + ChronoDuration::minutes(15)),
        ),
        (
            "emp",
            kl_employee_user_id,
            &totp_pending_secret,
            false,
            false,
            0,
            None,
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO user_totp (
                id, user_id, secret_encrypted, enabled, confirmed_at,
                failed_attempts, locked_until
            ) VALUES ($1, $2, $3, $4, CASE WHEN $5 THEN NOW() - INTERVAL '10 days' ELSE NULL END, $6, $7)
            ON CONFLICT (user_id) DO NOTHING
            "#,
        )
        .bind(det_id(&format!("totp:{ukey}")))
        .bind(uid)
        .bind(sec)
        .bind(enabled)
        .bind(confirmed)
        .bind(failed)
        .bind(locked_until)
        .execute(&pool)
        .await?;
    }

    let unused_code = "ABCD-1234-UNUSED";
    let used_code = "EFGH-5678-USED";
    let backup_hash_unused = bcrypt::hash(unused_code, 4)?;
    let backup_hash_used = bcrypt::hash(used_code, 4)?;
    let backup_lookup_unused = backup_code_lookup_hash(&totp_key, super_admin_id, unused_code);
    let backup_lookup_used = backup_code_lookup_hash(&totp_key, super_admin_id, used_code);
    for (bkey, hash, lookup_hash, used) in [
        ("unused", &backup_hash_unused, &backup_lookup_unused, false),
        ("used", &backup_hash_used, &backup_lookup_used, true),
    ] {
        sqlx::query(
            r#"
            INSERT INTO user_totp_backup_codes (id, user_id, code_hash, used_at, code_lookup_hash)
            VALUES ($1, $2, $3, CASE WHEN $4 THEN NOW() - INTERVAL '1 day' ELSE NULL END, $5)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(det_id(&format!("totp-bc:{bkey}")))
        .bind(super_admin_id)
        .bind(hash)
        .bind(used)
        .bind(lookup_hash)
        .execute(&pool)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO passkey_credentials (id, user_id, credential_name, credential_json, last_used_at)
        VALUES ($1, $2, 'MacBook Touch ID', $3, NOW() - INTERVAL '1 day')
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(det_id("passkey:sa"))
    .bind(super_admin_id)
    .bind(json!({"cred": {"id": "dev-seed-passkey-id", "public_key_alg": -7}}))
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO passkey_challenges (id, user_id, challenge_type, state_json, email, expires_at)
        VALUES ($1, $2, 'authentication', $3, 'employee@aureliatech.test', NOW() + INTERVAL '5 minutes')
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(det_id("pk-chal:emp"))
    .bind(kl_employee_user_id)
    .bind(json!({"challenge": "dev-seed-webauthn-state"}))
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO oauth2_accounts (id, user_id, provider, provider_user_id, provider_email, provider_name)
        VALUES ($1, $2, 'google', 'google-sub-dev-seed-001', 'aisyah.google@example.test', 'Aisyah Google')
        ON CONFLICT (provider, provider_user_id) DO NOTHING
        "#,
    )
    .bind(det_id("oauth2:emp"))
    .bind(kl_employee_user_id)
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO oauth2_states (state, code_verifier, expires_at)
        VALUES ('dev-seed-oauth2-state-001', 'dev-seed-pkce-verifier-001', NOW() + INTERVAL '10 minutes')
        ON CONFLICT (state) DO NOTHING
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO mfa_challenges (jti, user_id, attempts, expires_at)
        VALUES ($1, $2, 1, NOW() + INTERVAL '5 minutes')
        ON CONFLICT (jti) DO NOTHING
        "#,
    )
    .bind(det_id("mfa-chal:sa"))
    .bind(super_admin_id)
    .execute(&pool)
    .await?;

    for (pkey, uid, st, rev) in [
        ("pend", kl_employee_user_id, "pending", None),
        ("app", kl_hr_manager_id, "approved", Some(kl_admin_id)),
        ("rej", kl_manager_id, "rejected", Some(kl_admin_id)),
        (
            "comp",
            kl_payroll_admin_id,
            "completed",
            Some(super_admin_id),
        ),
        ("exp", inactive_sa_id, "expired", None),
    ] {
        sqlx::query(
            r#"
            INSERT INTO password_reset_requests (id, user_id, status, reviewed_by, reviewed_at, reset_token_hash, reset_token_expires_at)
            VALUES ($1, $2, $3, $4, CASE WHEN $4 IS NOT NULL THEN NOW() - INTERVAL '2 days' ELSE NULL END,
                    $5, NOW() + INTERVAL '1 hour')
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(det_id(&format!("pwr:{pkey}")))
        .bind(uid)
        .bind(st)
        .bind(rev)
        .bind(if st == "approved" { Some(sha256_hex("dev-seed-reset-token")) } else { None })
        .execute(&pool)
        .await?;
    }

    let sess_active_sa = det_id("sess:sa-active");
    let sess_active_emp = det_id("sess:emp-active");
    let sess_revoked_emp = det_id("sess:emp-revoked");
    let sess_admin = det_id("sess:kl-admin-expired");
    sqlx::query(
        r#"
        INSERT INTO user_sessions (id, user_id, user_agent, ip_address, expires_at, revoked_at)
        VALUES
            ($1, $2, 'Mozilla/5.0 SeedBrowser/1.0', '203.0.113.10'::inet, NOW() + INTERVAL '7 days', NULL),
            ($3, $4, 'Mozilla/5.0 EmpMobile/1.0', '203.0.113.11'::inet, NOW() + INTERVAL '7 days', NULL),
            ($5, $4, 'Mozilla/5.0 OldDevice/1.0', '198.51.100.25'::inet, NOW() + INTERVAL '7 days', NOW() - INTERVAL '1 day'),
            ($6, $7, 'Mozilla/5.0 AdminBrowser/1.0', '203.0.113.12'::inet, NOW() + INTERVAL '7 days', NULL)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(sess_active_sa)
    .bind(super_admin_id)
    .bind(sess_active_emp)
    .bind(kl_employee_user_id)
    .bind(sess_revoked_emp)
    .bind(sess_admin)
    .bind(kl_admin_id)
    .execute(&pool)
    .await?;

    for (tkey, uid, sid, raw_tok, revoked, exp_days) in [
        (
            "active",
            super_admin_id,
            sess_active_sa,
            "dev-seed-rt-active",
            false,
            7_i64,
        ),
        (
            "revoked",
            kl_employee_user_id,
            sess_revoked_emp,
            "dev-seed-rt-revoked",
            true,
            7_i64,
        ),
        (
            "expired",
            kl_admin_id,
            sess_admin,
            "dev-seed-rt-expired",
            false,
            -5_i64,
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at, revoked, session_id, revoked_at)
            VALUES ($1, $2, $3, NOW() + ($4 || ' days')::interval, $5, $6, CASE WHEN $5 THEN NOW() ELSE NULL END)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(det_id(&format!("rt:{tkey}")))
        .bind(uid)
        .bind(sha256_hex(raw_tok))
        .bind(exp_days)
        .bind(revoked)
        .bind(sid)
        .execute(&pool)
        .await?;
    }

    // 11. Employee Allowances, Work Schedules, Salary History, TP3, Teams & Team Members, Leave Balances
    sqlx::query(
        r#"
        INSERT INTO employee_allowances (id, employee_id, company_id, category, name, amount, is_taxable, is_recurring, effective_from, is_active)
        VALUES
            ($1, $2, $3, 'earning', 'Transport Allowance', 35000, TRUE, TRUE, DATE '2026-01-01', TRUE),
            ($4, $5, $3, 'earning', 'Medical Optical Exempt', 20000, FALSE, TRUE, DATE '2026-01-01', TRUE),
            ($6, $7, $3, 'deduction', 'Excess Recovery', 200000, FALSE, TRUE, DATE '2026-01-01', TRUE)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(det_id("ea:std-transport"))
    .bind(emp_std)
    .bind(c_kl)
    .bind(det_id("ea:std-med"))
    .bind(emp_married_zakat)
    .bind(det_id("ea:negnet"))
    .bind(emp_neg_net)
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO employee_work_schedules (id, employee_id, company_id, day_of_week, start_time, end_time, grace_minutes, is_active)
        VALUES ($1, $2, $3, 1, TIME '08:30:00', TIME '17:30:00', 10, TRUE)
        ON CONFLICT (employee_id, day_of_week) DO NOTHING
        "#,
    )
    .bind(det_id("ews:std-mon"))
    .bind(emp_std)
    .bind(c_kl)
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO salary_history (id, employee_id, company_id, old_salary, new_salary, effective_date, reason, change_type, created_by, approved_by, approved_at)
        VALUES
            ($1, $2, $3, 1000000, 1200000, DATE '2026-07-01', 'Mid-year performance review', 'increment', $4, $5, TIMESTAMPTZ '2026-06-25T10:00:00Z'),
            ($6, $7, $3, 0, 450000, DATE '2022-01-10', 'Initial offer', 'new_hire', $4, $5, TIMESTAMPTZ '2022-01-05T10:00:00Z')
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(det_id("salhist:tp3"))
    .bind(emp_tp3_inc)
    .bind(c_kl)
    .bind(kl_hr_manager_id)
    .bind(kl_finance_id)
    .bind(det_id("salhist:std"))
    .bind(emp_std)
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO tp3_records (id, employee_id, company_id, tax_year, previous_employer_name, previous_income_ytd, previous_epf_ytd, previous_pcb_ytd, previous_socso_ytd, previous_zakat_ytd, created_by)
        VALUES ($1, $2, $3, 2026, 'Previous FinTech Sdn Bhd', 2400000, 264000, 145000, 19800, 0, $4)
        ON CONFLICT (employee_id, tax_year) DO NOTHING
        "#,
    )
    .bind(det_id("tp3:meiling-2026"))
    .bind(emp_tp3_inc)
    .bind(c_kl)
    .bind(kl_payroll_admin_id)
    .execute(&pool)
    .await?;

    let team_kl_eng = det_id("team:kl-eng");
    sqlx::query(
        "INSERT INTO teams (id, company_id, name, description, tag, created_by) VALUES ($1, $2, 'Core Platform', 'Backend & Payroll Engineers', 'engineering', $3) ON CONFLICT (company_id, name) DO NOTHING",
    )
    .bind(team_kl_eng)
    .bind(c_kl)
    .bind(kl_hr_manager_id)
    .execute(&pool)
    .await?;

    for (mkey, eid, role) in [
        ("lead", emp_married_zakat, "lead"),
        ("mem1", emp_std, "member"),
        ("mem2", emp_high, "member"),
    ] {
        sqlx::query(
            "INSERT INTO team_members (id, team_id, employee_id, role) VALUES ($1, $2, $3, $4) ON CONFLICT (team_id, employee_id) DO NOTHING",
        )
        .bind(det_id(&format!("tmem:{mkey}")))
        .bind(team_kl_eng)
        .bind(eid)
        .bind(role)
        .execute(&pool)
        .await?;
    }

    for (yr, emp_id, _cid, lt_id, ent, taken, pend, cf) in [
        (
            2025,
            emp_std,
            c_kl,
            kl_lt_annual,
            dec!(16),
            dec!(10),
            dec!(0),
            dec!(2),
        ),
        (
            2026,
            emp_std,
            c_kl,
            kl_lt_annual,
            dec!(16),
            dec!(3),
            dec!(1),
            dec!(5),
        ),
        (
            2026,
            emp_std,
            c_kl,
            kl_lt_medical,
            dec!(14),
            dec!(2),
            dec!(0),
            dec!(0),
        ),
        (
            2026,
            emp_std,
            c_kl,
            kl_lt_repl,
            dec!(2),
            dec!(0),
            dec!(0),
            dec!(0),
        ),
        (
            2026,
            emp_married_zakat,
            c_kl,
            kl_lt_annual,
            dec!(18),
            dec!(4),
            dec!(0),
            dec!(4),
        ),
        (
            2026,
            emp_jb_shift1,
            c_jb,
            jb_lt_annual,
            dec!(14),
            dec!(2),
            dec!(0),
            dec!(2),
        ),
        (
            2026,
            emp_sg1,
            c_sg,
            sg_lt_annual,
            dec!(18),
            dec!(5),
            dec!(0),
            dec!(3),
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO leave_balances (id, employee_id, leave_type_id, year, entitled_days, taken_days, pending_days, carried_forward)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (employee_id, leave_type_id, year) DO NOTHING
            "#,
        )
        .bind(det_id(&format!("lb:{yr}:{emp_id}:{lt_id}")))
        .bind(emp_id)
        .bind(lt_id)
        .bind(yr)
        .bind(ent)
        .bind(taken)
        .bind(pend)
        .bind(cf)
        .execute(&pool)
        .await?;
    }

    // 12. Uploaded Files, Leave Requests, Overtime Applications, Claims, Documents
    for (ukey, stored_name, cid, up_by) in [
        (
            "receipt1",
            "dev-seed-receipt-2026-01.pdf",
            c_kl,
            kl_employee_user_id,
        ),
        ("mc1", "dev-seed-mc-2026-02.pdf", c_kl, kl_employee_user_id),
        (
            "contract1",
            "dev-seed-employment-contract.pdf",
            c_kl,
            kl_hr_manager_id,
        ),
        (
            "expat1",
            "dev-seed-work-permit-expiry.pdf",
            c_kl,
            kl_hr_manager_id,
        ),
    ] {
        let _ = ukey;
        sqlx::query(
            "INSERT INTO uploaded_files (stored_name, company_id, uploaded_by) VALUES ($1, $2, $3) ON CONFLICT (stored_name) DO NOTHING",
        )
        .bind(stored_name)
        .bind(cid)
        .bind(up_by)
        .execute(&pool)
        .await?;
    }

    let lr_dec_unpaid = det_id("lr:std-unpaid-dec");
    for (lkey, lid, eid, cid, lt_id, sd, ed, days, status, att) in [
        (
            "std-ann-app",
            det_id("lr:std-ann-app"),
            emp_std,
            c_kl,
            kl_lt_annual,
            NaiveDate::from_ymd_opt(2026, 2, 10).unwrap(),
            NaiveDate::from_ymd_opt(2026, 2, 11).unwrap(),
            dec!(2),
            "approved",
            None,
        ),
        (
            "std-mc-app",
            det_id("lr:std-mc-app"),
            emp_std,
            c_kl,
            kl_lt_medical,
            NaiveDate::from_ymd_opt(2026, 5, 6).unwrap(),
            NaiveDate::from_ymd_opt(2026, 5, 6).unwrap(),
            dec!(1),
            "approved",
            Some("/api/uploads/dev-seed-mc-2026-02.pdf"),
        ),
        (
            "std-pend",
            det_id("lr:std-pend"),
            emp_std,
            c_kl,
            kl_lt_annual,
            NaiveDate::from_ymd_opt(2026, 12, 28).unwrap(),
            NaiveDate::from_ymd_opt(2026, 12, 28).unwrap(),
            dec!(1),
            "pending",
            None,
        ),
        (
            "std-rej",
            det_id("lr:std-rej"),
            emp_contract,
            c_kl,
            kl_lt_annual,
            NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
            NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
            dec!(1),
            "rejected",
            None,
        ),
        (
            "std-canc",
            det_id("lr:std-canc"),
            emp_min_wage,
            c_kl,
            kl_lt_annual,
            NaiveDate::from_ymd_opt(2026, 4, 2).unwrap(),
            NaiveDate::from_ymd_opt(2026, 4, 2).unwrap(),
            dec!(1),
            "cancelled",
            None,
        ),
        (
            "std-unpaid-dec",
            lr_dec_unpaid,
            emp_min_wage,
            c_kl,
            kl_lt_unpaid,
            NaiveDate::from_ymd_opt(2026, 12, 7).unwrap(),
            NaiveDate::from_ymd_opt(2026, 12, 8).unwrap(),
            dec!(2),
            "approved",
            None,
        ),
    ] {
        let _ = lkey;
        sqlx::query(
            r#"
            INSERT INTO leave_requests (
                id, employee_id, company_id, leave_type_id, start_date, end_date,
                days, reason, status, attachment_url, reviewed_by, reviewed_at, created_by
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, 'Scenario leave request', $8, $9,
                CASE WHEN $8 IN ('approved','rejected') THEN $10 ELSE NULL END,
                CASE WHEN $8 IN ('approved','rejected') THEN NOW() - INTERVAL '3 days' ELSE NULL END,
                $11
            )
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(lid)
        .bind(eid)
        .bind(cid)
        .bind(lt_id)
        .bind(sd)
        .bind(ed)
        .bind(days)
        .bind(status)
        .bind(att)
        .bind(kl_hr_manager_id)
        .bind(kl_employee_user_id)
        .execute(&pool)
        .await?;
    }

    let ot_dec_app = det_id("ot:std-dec-app");
    for (oid, eid, cid, date, st, et, hrs, otype, status) in [
        (
            ot_dec_app,
            emp_std,
            c_kl,
            NaiveDate::from_ymd_opt(2026, 12, 3).unwrap(),
            NaiveTime::from_hms_opt(18, 30, 0).unwrap(),
            NaiveTime::from_hms_opt(21, 30, 0).unwrap(),
            dec!(3.0),
            "normal",
            "approved",
        ),
        (
            det_id("ot:jb-overnight"),
            emp_jb_shift2,
            c_jb,
            NaiveDate::from_ymd_opt(2026, 11, 14).unwrap(),
            NaiveTime::from_hms_opt(22, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(2, 0, 0).unwrap(),
            dec!(4.0),
            "rest_day",
            "pending",
        ),
        (
            det_id("ot:ph-rej"),
            emp_min_wage,
            c_kl,
            NaiveDate::from_ymd_opt(2026, 8, 31).unwrap(),
            NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(13, 0, 0).unwrap(),
            dec!(4.0),
            "public_holiday",
            "rejected",
        ),
        (
            det_id("ot:canc"),
            emp_contract,
            c_kl,
            NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
            NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(21, 0, 0).unwrap(),
            dec!(2.0),
            "normal",
            "cancelled",
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO overtime_applications (
                id, employee_id, company_id, ot_date, start_time, end_time,
                hours, ot_type, reason, status, reviewed_by, reviewed_at, created_by
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, 'Production release support', $9,
                CASE WHEN $9 IN ('approved','rejected') THEN $10 ELSE NULL END,
                CASE WHEN $9 IN ('approved','rejected') THEN NOW() - INTERVAL '2 days' ELSE NULL END,
                $11
            )
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(oid)
        .bind(eid)
        .bind(cid)
        .bind(date)
        .bind(st)
        .bind(et)
        .bind(hrs)
        .bind(otype)
        .bind(status)
        .bind(if cid == c_kl {
            kl_hr_manager_id
        } else {
            jb_admin_id
        })
        .bind(if cid == c_kl {
            kl_employee_user_id
        } else {
            jb_admin_id
        })
        .execute(&pool)
        .await?;
    }

    // 13. Real Payroll Runs + Lifecycle Transitions (Jan..=Nov 2026)
    //     Step 13a: Process Jan..=Mar 2026 BEFORE soft-deleting Q1 resignee (Priya Nair)
    for month in 1..=3_i32 {
        let pay_date = NaiveDate::from_ymd_opt(2026, month as u32, 28).unwrap();
        ensure_active_payroll_run(
            &pool,
            c_kl,
            pg_kl_exec,
            2026,
            month,
            pay_date,
            kl_payroll_admin_id,
            "payroll@aureliatech.test",
            kl_finance_id,
            "finance@aureliatech.test",
            "paid",
            "Monthly executive run",
        )
        .await?;
    }

    // Soft-delete Priya Nair AFTER March 2026 run so EA form tests retain paid soft-deleted staff!
    sqlx::query(
        "UPDATE employees SET is_active = FALSE, deleted_at = COALESCE(deleted_at, TIMESTAMPTZ '2026-04-05T09:00:00Z'), updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(emp_q1_deleted)
    .execute(&pool)
    .await?;

    // Process Apr..=Jun 2026 BEFORE updating Jason Lim's bank account (verifying payslip bank snapshot)
    for month in 4..=6_i32 {
        let pay_date = NaiveDate::from_ymd_opt(2026, month as u32, 28).unwrap();
        ensure_active_payroll_run(
            &pool,
            c_kl,
            pg_kl_exec,
            2026,
            month,
            pay_date,
            kl_payroll_admin_id,
            "payroll@aureliatech.test",
            kl_finance_id,
            "finance@aureliatech.test",
            "paid",
            "Monthly executive run",
        )
        .await?;
    }

    sqlx::query("UPDATE employees SET bank_account_number = '1140-9001-0005-NEW', updated_at = NOW() WHERE id = $1")
        .bind(emp_high)
        .execute(&pool)
        .await?;

    // Jul, Aug (v1 cancelled -> v2 paid), Sep (approved), Oct (pending_approval), Nov (processed)
    let kl_jul_run = ensure_active_payroll_run(
        &pool,
        c_kl,
        pg_kl_exec,
        2026,
        7,
        NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
        kl_payroll_admin_id,
        "payroll@aureliatech.test",
        kl_finance_id,
        "finance@aureliatech.test",
        "paid",
        "Monthly executive run (post-increment)",
    )
    .await?;
    let _kl_aug_run = ensure_active_payroll_run(
        &pool,
        c_kl,
        pg_kl_exec,
        2026,
        8,
        NaiveDate::from_ymd_opt(2026, 8, 28).unwrap(),
        kl_payroll_admin_id,
        "payroll@aureliatech.test",
        kl_finance_id,
        "finance@aureliatech.test",
        "cancel_then_paid",
        "August revised payroll run",
    )
    .await?;
    let _kl_sep_run = ensure_active_payroll_run(
        &pool,
        c_kl,
        pg_kl_exec,
        2026,
        9,
        NaiveDate::from_ymd_opt(2026, 9, 28).unwrap(),
        kl_payroll_admin_id,
        "payroll@aureliatech.test",
        kl_finance_id,
        "finance@aureliatech.test",
        "approved",
        "September approved run",
    )
    .await?;
    let _kl_oct_run = ensure_active_payroll_run(
        &pool,
        c_kl,
        pg_kl_exec,
        2026,
        10,
        NaiveDate::from_ymd_opt(2026, 10, 28).unwrap(),
        kl_payroll_admin_id,
        "payroll@aureliatech.test",
        kl_finance_id,
        "finance@aureliatech.test",
        "pending_approval",
        "October awaiting finance sign-off",
    )
    .await?;
    let _kl_nov_run = ensure_active_payroll_run(
        &pool,
        c_kl,
        pg_kl_exec,
        2026,
        11,
        NaiveDate::from_ymd_opt(2026, 11, 28).unwrap(),
        kl_payroll_admin_id,
        "payroll@aureliatech.test",
        kl_finance_id,
        "finance@aureliatech.test",
        "processed",
        "November processed draft for QA inspection",
    )
    .await?;

    // Operations, Selatan JB & Merlion SG payroll runs
    ensure_active_payroll_run(
        &pool,
        c_kl,
        pg_kl_ops,
        2026,
        9,
        NaiveDate::from_ymd_opt(2026, 9, 28).unwrap(),
        kl_payroll_admin_id,
        "payroll@aureliatech.test",
        kl_finance_id,
        "finance@aureliatech.test",
        "paid",
        "Operations September",
    )
    .await?;
    ensure_active_payroll_run(
        &pool,
        c_kl,
        pg_kl_ops,
        2026,
        10,
        NaiveDate::from_ymd_opt(2026, 10, 28).unwrap(),
        kl_payroll_admin_id,
        "payroll@aureliatech.test",
        kl_finance_id,
        "finance@aureliatech.test",
        "paid",
        "Operations October",
    )
    .await?;
    ensure_active_payroll_run(
        &pool,
        c_kl,
        pg_kl_ops,
        2026,
        11,
        NaiveDate::from_ymd_opt(2026, 11, 28).unwrap(),
        kl_payroll_admin_id,
        "payroll@aureliatech.test",
        kl_finance_id,
        "finance@aureliatech.test",
        "processed",
        "Operations November",
    )
    .await?;

    ensure_active_payroll_run(
        &pool,
        c_jb,
        pg_jb_shift,
        2026,
        9,
        NaiveDate::from_ymd_opt(2026, 9, 28).unwrap(),
        jb_admin_id,
        "admin@selatanmfg.test",
        kl_finance_id,
        "finance@aureliatech.test",
        "paid",
        "Plant Shift September",
    )
    .await?;
    ensure_active_payroll_run(
        &pool,
        c_jb,
        pg_jb_shift,
        2026,
        10,
        NaiveDate::from_ymd_opt(2026, 10, 28).unwrap(),
        jb_admin_id,
        "admin@selatanmfg.test",
        kl_finance_id,
        "finance@aureliatech.test",
        "paid",
        "Plant Shift October",
    )
    .await?;
    ensure_active_payroll_run(
        &pool,
        c_jb,
        pg_jb_shift,
        2026,
        11,
        NaiveDate::from_ymd_opt(2026, 11, 28).unwrap(),
        jb_admin_id,
        "admin@selatanmfg.test",
        kl_finance_id,
        "finance@aureliatech.test",
        "pending_approval",
        "Plant Shift November",
    )
    .await?;

    ensure_active_payroll_run(
        &pool,
        c_sg,
        pg_sg_core,
        2026,
        10,
        NaiveDate::from_ymd_opt(2026, 10, 28).unwrap(),
        sg_admin_id,
        "admin@merliondigital.test",
        kl_finance_id,
        "finance@aureliatech.test",
        "paid",
        "SG Core October",
    )
    .await?;
    ensure_active_payroll_run(
        &pool,
        c_sg,
        pg_sg_core,
        2026,
        11,
        NaiveDate::from_ymd_opt(2026, 11, 28).unwrap(),
        sg_admin_id,
        "admin@merliondigital.test",
        kl_finance_id,
        "finance@aureliatech.test",
        "approved",
        "SG Core November",
    )
    .await?;

    // 14. Staged Unprocessed Payroll Entries for Dec 2026 (including OT/unpaid links)
    for (ekey, eid, cat, itype, desc, amt, qty, rate, tax, ot_id, lr_id) in [
        (
            "bonus",
            emp_std,
            "earning",
            "bonus",
            "Year-End Performance Bonus",
            200_000_i64,
            None,
            None,
            true,
            None,
            None,
        ),
        (
            "comm",
            emp_married_zakat,
            "earning",
            "commission",
            "Q4 Delivery Milestone Commission",
            150_000,
            None,
            None,
            true,
            None,
            None,
        ),
        (
            "ot",
            emp_std,
            "earning",
            "overtime",
            "Approved Dec Overtime (3h x 1.5)",
            9_730,
            Some(dec!(3.0)),
            Some(3_243),
            true,
            Some(ot_dec_app),
            None,
        ),
        (
            "unpaid",
            emp_min_wage,
            "deduction",
            "unpaid_leave",
            "Approved Unpaid Leave (2 days)",
            11_538,
            Some(dec!(2.0)),
            Some(5_769),
            false,
            None,
            Some(lr_dec_unpaid),
        ),
        (
            "loan",
            emp_high,
            "deduction",
            "loan_deduction",
            "Laptop Purchase Installment",
            30_000,
            None,
            None,
            false,
            None,
            None,
        ),
        (
            "ma",
            emp_tp3_inc,
            "earning",
            "monthly_allowance",
            "December Client Hosting Stipend",
            40_000,
            None,
            None,
            true,
            None,
            None,
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO payroll_entries (
                id, employee_id, company_id, period_year, period_month,
                category, item_type, description, amount, quantity, rate,
                is_taxable, is_processed, created_by, overtime_application_id, leave_request_id
            ) VALUES ($1, $2, $3, 2026, 12, $4, $5, $6, $7, $8, $9, $10, FALSE, $11, $12, $13)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(det_id(&format!("pe-dec:{ekey}")))
        .bind(eid)
        .bind(c_kl)
        .bind(cat)
        .bind(itype)
        .bind(desc)
        .bind(amt)
        .bind(qty)
        .bind(rate)
        .bind(tax)
        .bind(kl_payroll_admin_id)
        .bind(ot_id)
        .bind(lr_id)
        .execute(&pool)
        .await?;
    }

    // 15. Claims (all 6 lifecycle statuses including processed linked to Jul 2026 run)
    for (ckey, eid, title, amt, cat, edate, st, receipt, pr_id) in [
        (
            "draft",
            emp_std,
            "Co-working Day Pass",
            4_500_i64,
            "other",
            NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
            "draft",
            None,
            None,
        ),
        (
            "pend",
            emp_std,
            "Grab Airport Ride",
            7_800,
            "travel",
            NaiveDate::from_ymd_opt(2026, 11, 22).unwrap(),
            "pending",
            Some("/api/uploads/dev-seed-receipt-2026-01.pdf"),
            None,
        ),
        (
            "app",
            emp_min_wage,
            "Client Lunch Receipt",
            12_400,
            "meal",
            NaiveDate::from_ymd_opt(2026, 11, 15).unwrap(),
            "approved",
            None,
            None,
        ),
        (
            "rej",
            emp_contract,
            "Unapproved Hardware",
            45_000,
            "equipment",
            NaiveDate::from_ymd_opt(2026, 10, 18).unwrap(),
            "rejected",
            None,
            None,
        ),
        (
            "proc",
            emp_std,
            "AWS Summit Kuala Lumpur",
            65_000,
            "training",
            NaiveDate::from_ymd_opt(2026, 7, 12).unwrap(),
            "processed",
            Some("/api/uploads/dev-seed-receipt-2026-01.pdf"),
            Some(kl_jul_run),
        ),
        (
            "canc",
            emp_high,
            "Withdrawn Mileage",
            9_000,
            "travel",
            NaiveDate::from_ymd_opt(2026, 9, 4).unwrap(),
            "cancelled",
            None,
            None,
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO claims (
                id, employee_id, company_id, title, amount, category, receipt_url,
                expense_date, status, submitted_at, reviewed_by, reviewed_at, payroll_run_id, created_by
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9,
                CASE WHEN $9 <> 'draft' THEN NOW() - INTERVAL '5 days' ELSE NULL END,
                CASE WHEN $9 IN ('approved','rejected','processed') THEN $10 ELSE NULL END,
                CASE WHEN $9 IN ('approved','rejected','processed') THEN NOW() - INTERVAL '2 days' ELSE NULL END,
                $11, $12
            )
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(det_id(&format!("claim:{ckey}")))
        .bind(eid)
        .bind(c_kl)
        .bind(title)
        .bind(amt)
        .bind(cat)
        .bind(receipt)
        .bind(edate)
        .bind(st)
        .bind(kl_finance_id)
        .bind(pr_id)
        .bind(kl_employee_user_id)
        .execute(&pool)
        .await?;
    }

    // 16. Document Categories & Documents
    let cat_hr = det_id("dcat:hr");
    let cat_id = det_id("dcat:id");
    for (id, name, desc) in [
        (
            cat_hr,
            "Employment Contracts",
            "Signed offers and addendums",
        ),
        (
            cat_id,
            "Visas & Work Permits",
            "Expatriate and statutory permits",
        ),
    ] {
        sqlx::query(
            "INSERT INTO document_categories (id, company_id, name, description) VALUES ($1, $2, $3, $4) ON CONFLICT (company_id, name) DO NOTHING",
        )
        .bind(id)
        .bind(c_kl)
        .bind(name)
        .bind(desc)
        .execute(&pool)
        .await?;
    }

    for (dkey, eid, cat_id_ref, title, fname, url, status, exp, conf, del) in [
        (
            "active",
            Some(emp_std),
            cat_hr,
            "Aisyah Employment Contract",
            "employment-contract.pdf",
            "/api/uploads/dev-seed-employment-contract.pdf",
            "active",
            Some(NaiveDate::from_ymd_opt(2026, 12, 8).unwrap()),
            true,
            false,
        ),
        (
            "expired",
            Some(emp_contract),
            cat_id,
            "Annual Work Pass",
            "work-permit-expiry.pdf",
            "/api/uploads/dev-seed-work-permit-expiry.pdf",
            "expired",
            Some(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()),
            false,
            false,
        ),
        (
            "archived",
            None,
            cat_hr,
            "2024 Company Handbook",
            "handbook-2024.pdf",
            "https://example.invalid/handbook-2024.pdf",
            "archived",
            None,
            false,
            false,
        ),
        (
            "deleted",
            Some(emp_std),
            cat_hr,
            "Superseded Draft Letter",
            "superseded.pdf",
            "/api/uploads/dev-seed-employment-contract.pdf",
            "archived",
            None,
            false,
            true,
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO documents (
                id, company_id, employee_id, category_id, title, file_name, file_url,
                file_size, mime_type, status, expiry_date, is_confidential, deleted_at, created_by
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, 42100, 'application/pdf', $8::document_status,
                $9, $10, CASE WHEN $11 THEN NOW() - INTERVAL '1 day' ELSE NULL END, $12
            )
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(det_id(&format!("doc:{dkey}")))
        .bind(c_kl)
        .bind(eid)
        .bind(cat_id_ref)
        .bind(title)
        .bind(fname)
        .bind(url)
        .bind(status)
        .bind(exp)
        .bind(conf)
        .bind(del)
        .bind(kl_hr_manager_id)
        .execute(&pool)
        .await?;
    }

    // 17. Attendance Kiosk Credentials, QR Tokens, Observations & Records
    let kiosk_kl = det_id("kiosk:kl-lobby");
    let kiosk_rev = det_id("kiosk:kl-revoked");
    for (kid, cid, label, raw_tok, rev) in [
        (
            kiosk_kl,
            c_kl,
            "KLCC Ground Lobby iPad",
            "ksk-live-dev-seed-001",
            false,
        ),
        (
            kiosk_rev,
            c_kl,
            "Decommissioned Reception Terminal",
            "ksk-rev-dev-seed-002",
            true,
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO attendance_kiosk_credentials (id, company_id, label, token_hash, token_prefix, created_by, revoked_at)
            VALUES ($1, $2, $3, $4, $5, $6, CASE WHEN $7 THEN NOW() - INTERVAL '14 days' ELSE NULL END)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(kid)
        .bind(cid)
        .bind(label)
        .bind(sha256_hex(raw_tok))
        .bind(&raw_tok[..8])
        .bind(kl_admin_id)
        .bind(rev)
        .execute(&pool)
        .await?;
    }

    let qr_kiosk_live = det_id("qr:kiosk-live");
    for (qid, tok, used, exp_mins, kcred) in [
        (
            qr_kiosk_live,
            "dev-seed-qr-kiosk-live",
            false,
            5_i64,
            Some(kiosk_kl),
        ),
        (
            det_id("qr:admin-live"),
            "dev-seed-qr-admin-live",
            false,
            5,
            None,
        ),
        (
            det_id("qr:expired"),
            "dev-seed-qr-expired",
            false,
            -60,
            Some(kiosk_kl),
        ),
        (
            det_id("qr:revoked"),
            "dev-seed-qr-revoked",
            true,
            5,
            Some(kiosk_kl),
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO attendance_qr_tokens (id, company_id, token, expires_at, used, kiosk_credential_id)
            VALUES ($1, $2, $3, NOW() + ($4 || ' minutes')::interval, $5, $6)
            ON CONFLICT (token) DO NOTHING
            "#,
        )
        .bind(qid)
        .bind(c_kl)
        .bind(tok)
        .bind(exp_mins)
        .bind(used)
        .bind(kcred)
        .execute(&pool)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO attendance_network_observations (
            id, company_id, employee_id, network, prefix_len,
            observation_count, anchored_count, denied_count
        )
        VALUES
            ($1, $2, $3, '198.51.100.0', 24, 6, 4, 0),
            ($4, $2, $5, '203.0.115.0', 24, 3, 0, 2)
        ON CONFLICT (company_id, employee_id, network, prefix_len) DO NOTHING
        "#,
    )
    .bind(det_id("nobs:anchored"))
    .bind(c_kl)
    .bind(emp_std)
    .bind(det_id("nobs:denied"))
    .bind(emp_min_wage)
    .execute(&pool)
    .await?;

    // Attendance records covering qr_code, face_id, manual, auto-absent, overnight, outside geofence, open shift
    let kl_cin = Utc.with_ymd_and_hms(2026, 11, 23, 1, 0, 0).unwrap();
    for (
        akey,
        cid,
        eid,
        cin,
        cout,
        method,
        st,
        lat,
        lng,
        clat,
        clng,
        notes,
        qid,
        cby,
        hw,
        ot,
        ogeo,
        onet,
    ) in [
        (
            "kl-present",
            c_kl,
            emp_std,
            kl_cin,
            Some(kl_cin + ChronoDuration::hours(9)),
            "qr_code",
            "present",
            Some(3.1578),
            Some(101.7119),
            Some(3.1579),
            Some(101.7120),
            Some("On-site QR check-in"),
            Some(qr_kiosk_live),
            None,
            Some(dec!(8.0)),
            Some(dec!(0.0)),
            false,
            Some(false),
        ),
        (
            "kl-late-ogeo",
            c_kl,
            emp_min_wage,
            kl_cin + ChronoDuration::days(1) + ChronoDuration::minutes(35),
            Some(kl_cin + ChronoDuration::days(1) + ChronoDuration::hours(11)),
            "face_id",
            "late",
            Some(3.0500),
            Some(101.5500),
            Some(3.1578),
            Some(101.7119),
            Some("Late & off-site"),
            None,
            None,
            Some(dec!(9.5)),
            Some(dec!(1.5)),
            true,
            Some(true),
        ),
        (
            "kl-half",
            c_kl,
            emp_contract,
            kl_cin + ChronoDuration::days(2),
            Some(kl_cin + ChronoDuration::days(2) + ChronoDuration::hours(4)),
            "manual",
            "half_day",
            None,
            None,
            None,
            None,
            Some("Admin half-day entry"),
            None,
            Some(kl_hr_manager_id),
            Some(dec!(4.0)),
            Some(dec!(0.0)),
            false,
            None,
        ),
        (
            "kl-autoabs",
            c_kl,
            emp_married_zakat,
            Utc.with_ymd_and_hms(2026, 11, 20, 0, 0, 0).unwrap(),
            Some(Utc.with_ymd_and_hms(2026, 11, 20, 0, 0, 0).unwrap()),
            "manual",
            "absent",
            None,
            None,
            None,
            None,
            Some("Auto-marked absent (no check-in recorded)"),
            None,
            None,
            Some(dec!(0.0)),
            Some(dec!(0.0)),
            false,
            None,
        ),
        (
            "jb-overnight",
            c_jb,
            emp_jb_shift1,
            Utc.with_ymd_and_hms(2026, 11, 24, 14, 0, 0).unwrap(),
            Some(Utc.with_ymd_and_hms(2026, 11, 24, 23, 0, 0).unwrap()),
            "face_id",
            "present",
            Some(1.4726),
            Some(103.9029),
            Some(1.4726),
            Some(103.9029),
            Some("Night shift completed"),
            None,
            None,
            Some(dec!(8.0)),
            Some(dec!(1.0)),
            false,
            Some(false),
        ),
        (
            "kl-open",
            c_kl,
            emp_std,
            Utc::now() - ChronoDuration::hours(2),
            None,
            "qr_code",
            "present",
            Some(3.1578),
            Some(101.7119),
            None,
            None,
            Some("Currently clocked in"),
            Some(qr_kiosk_live),
            None,
            None,
            None,
            false,
            Some(false),
        ),
    ] {
        let _ = akey;
        sqlx::query(
            r#"
            INSERT INTO attendance_records (
                id, company_id, employee_id, check_in_at, check_out_at, method, status,
                latitude, longitude, checkout_latitude, checkout_longitude, notes,
                qr_token_id, created_by, hours_worked, overtime_hours,
                is_outside_geofence, is_offsite_network
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(det_id(&format!("att:{akey}")))
        .bind(cid)
        .bind(eid)
        .bind(cin)
        .bind(cout)
        .bind(method)
        .bind(st)
        .bind(lat)
        .bind(lng)
        .bind(clat)
        .bind(clng)
        .bind(notes)
        .bind(qid)
        .bind(cby)
        .bind(hw)
        .bind(ot)
        .bind(ogeo)
        .bind(onet)
        .execute(&pool)
        .await?;
    }

    // 18. Email Templates & Email Logs
    let tmpl_welcome = det_id("etmpl:kl-welcome");
    sqlx::query(
        r#"
        INSERT INTO email_templates (id, company_id, name, letter_type, subject, body_html, created_by)
        VALUES ($1, $2, 'Standard Welcome Letter', 'welcome', 'Welcome to {{company_name}}', '<p>Welcome {{employee_name}}</p>', $3)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(tmpl_welcome)
    .bind(c_kl)
    .bind(kl_hr_manager_id)
    .execute(&pool)
    .await?;

    for (ekey, st, att, next_retry, err) in [
        ("pend-new", "pending", 0, None, None),
        (
            "pend-retry",
            "pending",
            1,
            Some(Utc::now() + ChronoDuration::minutes(5)),
            Some("Temporary 421 SMTP backoff"),
        ),
        ("sent", "sent", 1, None, None),
        (
            "failed",
            "failed",
            3,
            None,
            Some("550 5.1.1 Mailbox unavailable"),
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO email_logs (
                id, company_id, employee_id, template_id, letter_type,
                recipient_email, recipient_name, subject, body_html,
                status, error_message, sent_at, created_by, attempts, next_retry_at
            ) VALUES (
                $1, $2, $3, $4, 'welcome', 'aisyah@aureliatech.test', 'Aisyah Binti Ahmad',
                'Welcome to Aurelia Tech', '<p>Welcome Aisyah</p>', $5, $6,
                CASE WHEN $5 = 'sent' THEN NOW() - INTERVAL '1 day' ELSE NULL END,
                $7, $8, $9
            )
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(det_id(&format!("elog:{ekey}")))
        .bind(c_kl)
        .bind(emp_std)
        .bind(tmpl_welcome)
        .bind(st)
        .bind(err)
        .bind(kl_hr_manager_id)
        .bind(att)
        .bind(next_retry)
        .execute(&pool)
        .await?;
    }

    // 19. Notifications, Web Push Subscriptions & Outbox
    let notif_unread = det_id("notif:emp-unread");
    let notif_read = det_id("notif:emp-read");
    sqlx::query(
        r#"
        INSERT INTO notifications (id, user_id, company_id, notification_type, title, message, is_read, read_at)
        VALUES
            ($1, $2, $3, 'payslip_ready', 'Payslip Available', 'Your August 2026 payslip is ready.', FALSE, NULL),
            ($4, $2, $3, 'leave_approved', 'Leave Approved', 'Your annual leave has been approved.', TRUE, NOW() - INTERVAL '1 day')
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(notif_unread)
    .bind(kl_employee_user_id)
    .bind(c_kl)
    .bind(notif_read)
    .execute(&pool)
    .await?;

    let (p256_a, auth_a) = synthetic_p256dh_and_auth(1);
    let (p256_b, auth_b) = synthetic_p256dh_and_auth(2);
    let push_sub_live = det_id("psub:emp-fcm");
    let push_sub_rev = det_id("psub:sa-moz-revoked");
    sqlx::query(
        r#"
        INSERT INTO push_subscriptions (id, user_id, company_id, session_id, endpoint, p256dh, auth, user_agent, revoked_at)
        VALUES
            ($1, $2, $3, $4, 'https://fcm.googleapis.com/fcm/send/dev-seed-sub-001', $5, $6, 'Chrome/130 Android', NULL),
            ($7, $8, $3, $9, 'https://updates.push.services.mozilla.com/wpush/v2/dev-seed-sub-002', $10, $11, 'Firefox/132', NOW() - INTERVAL '3 days')
        ON CONFLICT (endpoint) DO NOTHING
        "#,
    )
    .bind(push_sub_live)
    .bind(kl_employee_user_id)
    .bind(c_kl)
    .bind(sess_active_emp)
    .bind(&p256_a)
    .bind(&auth_a)
    .bind(push_sub_rev)
    .bind(super_admin_id)
    .bind(sess_active_sa)
    .bind(&p256_b)
    .bind(&auth_b)
    .execute(&pool)
    .await?;

    for (okey, sub_id, uid, nid, st, att, err) in [
        (
            "pend",
            push_sub_live,
            kl_employee_user_id,
            Some(notif_unread),
            "pending",
            0,
            None,
        ),
        (
            "sent",
            push_sub_live,
            kl_employee_user_id,
            Some(notif_read),
            "sent",
            1,
            None,
        ),
        (
            "fail",
            push_sub_live,
            kl_employee_user_id,
            None,
            "failed",
            4,
            Some("503 Push gateway timeout"),
        ),
        (
            "gone",
            push_sub_rev,
            super_admin_id,
            None,
            "gone",
            1,
            Some("410 Gone"),
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO push_outbox (
                id, subscription_id, user_id, company_id, notification_id,
                title, body, status, attempts, last_error, sent_at
            ) VALUES (
                $1, $2, $3, $4, $5, 'Payroll Notification', 'Dev seed push event',
                $6, $7, $8, CASE WHEN $6 = 'sent' THEN NOW() - INTERVAL '1 day' ELSE NULL END
            )
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(det_id(&format!("pout:{okey}")))
        .bind(sub_id)
        .bind(uid)
        .bind(c_kl)
        .bind(nid)
        .bind(st)
        .bind(att)
        .bind(err)
        .execute(&pool)
        .await?;
    }

    // 20. Bulk Import Sessions, Background Jobs & Audit Logs
    for (bkey, st, rcnt, vcnt) in [
        ("pend", "pending", 10, 10),
        ("conf", "confirmed", 25, 25),
        ("exp", "expired", 12, 11),
        ("fail", "failed", 8, 2),
    ] {
        sqlx::query(
            r#"
            INSERT INTO bulk_import_sessions (
                id, company_id, user_id, file_name, row_count, valid_count,
                validated_data, status, confirmed_at, expires_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, '[{"employee_number":"IMP-001"}]'::jsonb, $7,
                CASE WHEN $7 = 'confirmed' THEN NOW() - INTERVAL '2 days' ELSE NULL END,
                NOW() + INTERVAL '2 hours'
            )
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(det_id(&format!("bimp:{bkey}")))
        .bind(c_kl)
        .bind(kl_hr_manager_id)
        .bind(format!("employees_{bkey}.csv"))
        .bind(rcnt)
        .bind(vcnt)
        .bind(st)
        .execute(&pool)
        .await?;
    }

    for (jkey, jtype, st, pdone, ptot, group_tag, month_tag, err) in [
        (
            "pend",
            "payroll_run",
            "pending",
            0,
            11,
            "pg-sim-p",
            "12",
            None,
        ),
        (
            "run",
            "employee_import",
            "running",
            12,
            25,
            "imp-r",
            "12",
            None,
        ),
        (
            "succ",
            "payroll_run",
            "succeeded",
            11,
            11,
            "pg-sim-s",
            "10",
            None,
        ),
        (
            "fail",
            "employee_import",
            "failed",
            3,
            10,
            "imp-f",
            "11",
            Some("CSV row 4 missing date_joined"),
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO background_jobs (
                id, company_id, job_type, status, payload, progress_done, progress_total,
                error, created_by, started_at, finished_at
            ) VALUES (
                $1, $2, $3, $4,
                jsonb_build_object('payroll_group_id', $5, 'period_year', '2026', 'period_month', $6),
                $7, $8, $9, $10,
                CASE WHEN $4 <> 'pending' THEN NOW() - INTERVAL '5 minutes' ELSE NULL END,
                CASE WHEN $4 IN ('succeeded','failed') THEN NOW() - INTERVAL '1 minute' ELSE NULL END
            )
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(det_id(&format!("bgj:{jkey}")))
        .bind(c_kl)
        .bind(jtype)
        .bind(st)
        .bind(group_tag)
        .bind(month_tag)
        .bind(pdone)
        .bind(ptot)
        .bind(err)
        .bind(kl_payroll_admin_id)
        .execute(&pool)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO audit_logs (id, user_id, company_id, action, entity_type, entity_id, new_values, description)
        VALUES ($1, $2, $3, 'seed_scenario', 'company', $3, '{"source":"seed_scenarios"}'::jsonb, '[dev-seed] Scenario dataset initialized')
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(det_id("audit:init"))
    .bind(super_admin_id)
    .bind(c_kl)
    .execute(&pool)
    .await?;

    // Optional --scale=perf bulk dataset
    if opts.scale == Scale::Perf {
        seed_perf_scale(&pool, c_kl, pg_kl_exec, kl_lt_annual, kl_employee_user_id).await?;
    }

    println!();
    println!("=== Scenario Seeder Complete ===");
    println!("Password (all dev accounts): {SEED_PASSWORD_PLAIN}");
    println!("Companies:");
    println!(
        "  1. Aurelia Tech Sdn Bhd (KL, Asia/Kuala_Lumpur, geofence=enforce, network=enforce) [{c_kl}]"
    );
    println!("  2. Selatan Manufacturing Sdn Bhd (JB Sun-Thu, overnight shift) [{c_jb}]");
    println!("  3. Merlion Digital Pte Ltd (SG, Asia/Singapore) [{c_sg}]");
    println!("Key Logins:");
    println!(
        "  super_admin   : superadmin@payrollmy.test (all 3 companies, TOTP active + passkey)"
    );
    println!("  admin         : admin@aureliatech.test (KL)");
    println!("  payroll_admin : payroll@aureliatech.test (all 3 companies)");
    println!("  hr_manager    : hr@aureliatech.test (KL)");
    println!("  finance       : finance@aureliatech.test (all 3 companies, TOTP locked)");
    println!("  exec          : exec@aureliatech.test (all 3 companies, read-only)");
    println!("  manager       : manager@aureliatech.test (Farid + additive approver group)");
    println!("  force-pwd-chg : newbie@aureliatech.test (must_change_password=true)");
    println!(
        "  employee      : employee@aureliatech.test (Aisyah, unconfirmed TOTP + Google OAuth2)"
    );
    println!("  JB/SG admins  : admin@selatanmfg.test | admin@merliondigital.test");
    println!(
        "Scale mode      : {}",
        if opts.scale == Scale::Perf {
            "perf"
        } else {
            "standard"
        }
    );

    // Confirm reference to jb_employee_user_id not unused if optimized away later
    let _ = jb_employee_user_id;

    Ok(())
}
