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
/// audit rows. Refuse a *divergent* history before changing the schema so a
/// mistakenly deleted migration cannot be silently accepted — but tolerate a
/// history that is merely ahead of this build, which is what a rollback looks
/// like from the older binary's side.
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
    let (ahead_versions, divergent_versions) =
        classify_unexpected_migrations(&applied_versions, &embedded_versions);

    assert!(
        divergent_versions.is_empty(),
        "Database contains migrations absent from this build: {divergent_versions:?}"
    );

    if !ahead_versions.is_empty() {
        tracing::warn!(
            versions = ?ahead_versions,
            "Database schema is AHEAD of this build (rolled-back binary?); starting anyway. \
             Requests touching columns those migrations changed may fail until the newer \
             release is redeployed or the pre-deploy dump is restored."
        );
    }
}

/// Split applied-but-not-embedded versions into `(ahead, divergent)`.
///
/// A version above every embedded one means the DATABASE is newer than the
/// BINARY, which is precisely a rollback: release N applied it, N failed its
/// health gate, and `deploy.sh` brought the N-1 image back. Aborting there
/// crash-loops N-1 as well (`restart: unless-stopped` + migrations at startup),
/// so *neither* image boots and the operator has no live host to work from.
/// Degraded operation beats a total outage, so those only warn.
///
/// Anything else — a version interleaved with ones this build does know — is a
/// genuinely divergent history, i.e. a migration deleted from the middle of the
/// chain. That is what the guard exists for and it still aborts, before the
/// schema is touched.
fn classify_unexpected_migrations(applied: &[i64], embedded: &[i64]) -> (Vec<i64>, Vec<i64>) {
    const RETIRED_VERSIONS: [i64; 4] = [1, 2, 3, 4];

    let ceiling = embedded.iter().copied().max().unwrap_or(i64::MIN);

    applied
        .iter()
        .copied()
        .filter(|version| !RETIRED_VERSIONS.contains(version) && !embedded.contains(version))
        .partition(|version| *version > ceiling)
}

#[cfg(test)]
mod tests {
    use super::classify_unexpected_migrations;

    #[test]
    fn migration_history_allows_retired_and_all_current_embedded_versions() {
        let applied = [1, 2, 3, 4, 1000, 1001, 1002];
        let embedded = [1000, 1001, 1002];

        let (ahead, divergent) = classify_unexpected_migrations(&applied, &embedded);
        assert!(ahead.is_empty());
        assert!(divergent.is_empty());
    }

    #[test]
    fn migration_history_rejects_non_retired_versions_absent_from_build() {
        let applied = [1, 1000, 1001, 1002, 9000];
        let embedded = [1000, 1001, 1002];

        let (ahead, divergent) = classify_unexpected_migrations(&applied, &embedded);
        assert_eq!(ahead, [9000]);
        assert!(divergent.is_empty());
    }

    /// The rollback case. The N-1 binary must boot against the schema release N
    /// left behind, or the failed deployment takes the previous release down
    /// with it.
    #[test]
    fn a_version_past_the_newest_embedded_one_is_ahead_not_divergent() {
        let applied = [1, 2, 3, 4, 1000, 1013, 1014, 1015];
        let embedded = [1000, 1013, 1014];

        let (ahead, divergent) = classify_unexpected_migrations(&applied, &embedded);
        assert_eq!(ahead, [1015]);
        assert!(divergent.is_empty());
    }

    /// A gap *inside* the known range is the deleted-migration case the assert
    /// was written for, and stays fatal.
    #[test]
    fn a_version_inside_the_embedded_range_is_divergent() {
        let applied = [1000, 1005, 1014];
        let embedded = [1000, 1006, 1014];

        let (ahead, divergent) = classify_unexpected_migrations(&applied, &embedded);
        assert!(ahead.is_empty());
        assert_eq!(divergent, [1005]);
    }

    #[test]
    fn both_classes_are_reported_separately() {
        let applied = [1000, 1005, 1014, 1015];
        let embedded = [1000, 1006, 1014];

        let (ahead, divergent) = classify_unexpected_migrations(&applied, &embedded);
        assert_eq!(ahead, [1015]);
        assert_eq!(divergent, [1005]);
    }

    /// A build with no embedded migrations has no ceiling to compare against.
    /// It must not panic taking the max of an empty set.
    #[test]
    fn an_empty_embedded_set_does_not_panic() {
        let (ahead, divergent) = classify_unexpected_migrations(&[1, 2, 3, 4, 1000], &[]);
        assert_eq!(ahead, [1000]);
        assert!(divergent.is_empty());
    }
}
