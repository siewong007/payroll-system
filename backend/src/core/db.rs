use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

pub async fn create_pool(database_url: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
        .expect("Failed to create database pool")
}

pub async fn run_migrations(pool: &PgPool) {
    // Versions 1-4 were the pre-consolidation migration chain. The canonical
    // PostgreSQL 19 baseline starts at 1000 so existing installations can keep
    // their audit history while fresh databases only need the two current files.
    let mut migrator = sqlx::migrate!("./migrations");
    reject_unknown_migration_history(pool, &migrator).await;
    migrator.set_ignore_missing(true);
    migrator
        .run(pool)
        .await
        .expect("Failed to run database migrations");
}

/// `ignore_missing` is required for databases that retain the retired v1-v4
/// audit rows. Refuse any other absent migration before changing the schema so
/// a mistakenly deleted future migration cannot be silently accepted.
async fn reject_unknown_migration_history(pool: &PgPool, migrator: &sqlx::migrate::Migrator) {
    let history_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NOT NULL")
            .fetch_one(pool)
            .await
            .expect("Failed to inspect SQLx migration history");

    if !history_exists {
        return;
    }

    let applied_versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM public._sqlx_migrations ORDER BY version")
            .fetch_all(pool)
            .await
            .expect("Failed to validate SQLx migration history");
    let embedded_versions: Vec<i64> = migrator.iter().map(|migration| migration.version).collect();
    let unexpected_versions = unexpected_migration_versions(&applied_versions, &embedded_versions);

    assert!(
        unexpected_versions.is_empty(),
        "Database contains migrations absent from this build: {unexpected_versions:?}"
    );
}

fn unexpected_migration_versions(applied: &[i64], embedded: &[i64]) -> Vec<i64> {
    const RETIRED_VERSIONS: [i64; 4] = [1, 2, 3, 4];

    applied
        .iter()
        .copied()
        .filter(|version| !RETIRED_VERSIONS.contains(version) && !embedded.contains(version))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::unexpected_migration_versions;

    #[test]
    fn migration_history_allows_retired_and_all_current_embedded_versions() {
        let applied = [1, 2, 3, 4, 1000, 1001, 1002];
        let embedded = [1000, 1001, 1002];

        assert!(unexpected_migration_versions(&applied, &embedded).is_empty());
    }

    #[test]
    fn migration_history_rejects_non_retired_versions_absent_from_build() {
        let applied = [1, 1000, 1001, 1002, 9000];
        let embedded = [1000, 1001, 1002];

        assert_eq!(unexpected_migration_versions(&applied, &embedded), [9000]);
    }
}
