# Lightsail PostgreSQL 18 to 19 Beta 2 upgrade record

> Status: executed successfully on 2026-07-19 MYT.
>
> PostgreSQL classifies beta releases as testing builds and advises against
> production use. This was an explicitly requested pre-release upgrade. Host,
> backup-object, binary, checksum, and network identifiers are intentionally
> retained only in the private operations log.

## Result

| Item | Result |
| --- | --- |
| Host | Ubuntu Lightsail instance (identifier omitted) |
| Database | PostgreSQL `19beta2` from PGDG |
| Active cluster | `19/main` on the standard PostgreSQL port, data checksums enabled |
| Rollback cluster | PostgreSQL 18 cluster stopped and retained on a private alternate port |
| Older rollback cluster | PostgreSQL 16 cluster stopped and retained |
| Downtime | Approximately 15 seconds |
| Health | Local and public API health checks remained successful after cutover |

The beta packages were explicitly held so unattended upgrades could not move
the cluster to another beta with a potentially incompatible catalog. The hold
should be removed only during a planned and backed-up beta, RC, or final upgrade.

## Safety artifacts

The following were completed and verified, with identifiers stored outside the
repository:

- an encrypted off-host logical backup before the maintenance window;
- a final logical dump while the application was stopped;
- a post-upgrade verification dump and encrypted off-host copy;
- retention of the pre-upgrade backend binary;
- retention of the stopped PostgreSQL 18 cluster for rollback.

## Verification

Before application restart, the PostgreSQL 19 restore matched the PostgreSQL 18
baseline for company, employee, user, migration, and public-table counts. The
application then completed its forward schema/data work. Account-link coverage,
index readiness, PostgreSQL service state, backend service state, reverse proxy,
backup timer, and local/public health were checked. The post-cutover
service/database error scan was empty.

The historical migrations applied at that deployment have since been
consolidated. Their final effects now live in `backend/migrations/1000_schema.sql`
and `1001_data.sql`; existing databases retain the retired version rows as audit
history.

## Rollback outline

Do not remove the retained PostgreSQL 18 cluster until the beta deployment has
completed an adequate observation period.

1. Stop the backend and PostgreSQL 19 cluster.
2. Restore the PostgreSQL 18 cluster to the application port.
3. Restore the retained pre-upgrade backend binary.
4. Start the backend and verify local/public health.
5. Reconcile post-cutover writes or restore a newer logical dump if any were
   accepted after the cutover.

## Future PostgreSQL 19 updates

Beta catalog formats can change. Before moving from Beta 2 to another beta, RC,
or the final 19.0 release, take and verify a logical dump, schedule a maintenance
window, explicitly unhold packages, and perform a major-style upgrade or
dump/restore if the release notes require it.

Official references:

- [PostgreSQL 19 Beta 2 release](https://www.postgresql.org/about/news/postgresql-19-beta-2-released-3350/)
- [PostgreSQL beta information](https://www.postgresql.org/developer/beta/)
- [PostgreSQL Apt beta FAQ](https://wiki.postgresql.org/wiki/Apt/FAQ)
