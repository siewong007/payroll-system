use std::env;

use anyhow::{Context, bail};
use payroll_system::core::db;
use payroll_system::services::auth_service;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let database_url = required_env("DATABASE_URL")?;
    let company_name = required_env("BOOTSTRAP_COMPANY_NAME")?;
    let admin_name = required_env("BOOTSTRAP_ADMIN_NAME")?;
    let admin_email = required_env("BOOTSTRAP_ADMIN_EMAIL")?.to_lowercase();
    let admin_password = required_env("BOOTSTRAP_ADMIN_PASSWORD")?;

    auth_service::validate_password_strength(&admin_password)
        .context("BOOTSTRAP_ADMIN_PASSWORD does not meet the password policy")?;
    if !admin_email.contains('@') {
        bail!("BOOTSTRAP_ADMIN_EMAIL must be a valid email address");
    }

    let password_hash = bcrypt::hash(&admin_password, 12)
        .context("failed to hash bootstrap administrator password")?;
    let pool = db::create_pool(&database_url).await;
    db::run_migrations(&pool).await;

    let mut tx = pool.begin().await?;
    // Serialize bootstrap attempts without creating persistent lock state.
    sqlx::query("SELECT pg_advisory_xact_lock(706_179_001)")
        .execute(&mut *tx)
        .await?;

    let super_admin_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (\
             SELECT 1 FROM users \
             WHERE 'super_admin' = ANY(roles) AND deleted_at IS NULL\
         )",
    )
    .fetch_one(&mut *tx)
    .await?;
    if super_admin_exists {
        bail!("an active super administrator already exists; bootstrap refused");
    }

    let email_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (\
             SELECT 1 FROM users \
             WHERE lower(btrim(email)) = lower(btrim($1))\
         )",
    )
    .bind(&admin_email)
    .fetch_one(&mut *tx)
    .await?;
    if email_exists {
        bail!("a user with BOOTSTRAP_ADMIN_EMAIL already exists");
    }

    let company_id = Uuid::now_v7();
    let admin_id = Uuid::now_v7();

    sqlx::query(
        "INSERT INTO companies (id, name, created_by, updated_by) \
         VALUES ($1, $2, $3, $3)",
    )
    .bind(company_id)
    .bind(&company_name)
    .bind(admin_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO users (\
             id, email, password_hash, full_name, roles, company_id, must_change_password\
         ) VALUES ($1, $2, $3, $4, ARRAY['super_admin']::varchar(50)[], $5, FALSE)",
    )
    .bind(admin_id)
    .bind(&admin_email)
    .bind(password_hash)
    .bind(&admin_name)
    .bind(company_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("INSERT INTO user_companies (user_id, company_id) VALUES ($1, $2)")
        .bind(admin_id)
        .bind(company_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("SELECT public.provision_company_defaults($1, $2)")
        .bind(company_id)
        .bind(admin_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    println!("Created the initial company and super administrator ({admin_email}).");
    println!("Unset BOOTSTRAP_ADMIN_PASSWORD before starting the API.");
    Ok(())
}

fn required_env(name: &str) -> anyhow::Result<String> {
    let value = env::var(name).with_context(|| format!("{name} must be set"))?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("{name} must not be empty");
    }
    Ok(trimmed.to_owned())
}
