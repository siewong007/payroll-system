# Lightsail backend deploy — PostgreSQL 16 → 18 upgrade & migration re-baseline plan

> Status: **PLAN ONLY — do not execute until reviewed.** The frontend has already been
> deployed (S3 `payroll-dev-frontend` + CloudFront `ED4843A8VKOA2`). The backend has **not**
> been touched. Deploying the latest backend binary as-is would crash production — this
> document is the safe path to ship it.

## 1. Current production reality (hand-built, not the `infra/` Terraform)

| Piece | Reality |
|---|---|
| Host | Lightsail `Ubuntu-1` @ `13.251.162.88` (static IP `payroll-prod`), ~900 MB RAM, 34 GB free |
| Backend | Native binary `/usr/local/bin/payroll-system`, systemd `payroll-backend.service` (user `payroll-app`, env `/etc/payroll-backend/env`, WorkingDirectory `/var/lib/payroll-backend`) → `127.0.0.1:8080`. **Built elsewhere & copied in** (no Docker, no Rust, no source on box). |
| Proxy | Caddy → `api.payrollmy.com` → `127.0.0.1:8080` |
| Database | **Local PostgreSQL 16.14**, cluster `16/main` on `:5432`, DB `payroll_db`, **12 MB**, 49 tables |
| Migrations applied | 26 rows in `_sqlx_migrations` (the *old* flat `NNN_*.sql` set, versions 1–26, applied Apr 8–25) |
| Backups | `payroll-backup.timer` enabled, daily ~18:00 UTC to Cloudflare R2 (last run confirmed) |
| Frontend | S3 `payroll-dev-frontend` + CloudFront `ED4843A8VKOA2` (`payrollmy.com`) — already updated |

## 2. Why a plain "rebuild + copy binary + restart" breaks production

The latest code runs `sqlx::migrate!("./migrations/schema")` on every startup, against a
**consolidated/squashed** migration set:

```
backend/migrations/schema/001_schema.sql      # full schema, ids DEFAULT uuidv7()
backend/migrations/schema/002_drop_users_role.sql  # ALTER TABLE users DROP COLUMN role
backend/migrations/seed/001_seed.sql          # NOT run by migrate! (fresh-install seeding only)
```

Two independent failures, either of which crash-loops the backend and takes
`api.payrollmy.com` down:

1. **Migration-history mismatch.** Prod `_sqlx_migrations` has versions **1–26** (old names,
   old checksums). The new binary embeds only **v1 `schema` + v2 `drop users role`**. On
   startup sqlx validates applied-vs-embedded by checksum → **v1 checksum mismatch →
   `VersionMismatch` → migrate fails → process exits.**
2. **PG18-only SQL on PG16.** `001_schema.sql` uses `uuidv7()`, which only exists in
   **PostgreSQL 18**. The box is **PG16** (confirmed: `pg_proc` has no `uuidv7`). Even a clean
   apply would fail.

Confirmed drift that the re-baseline must account for:
- `users.role` column **still present** in prod → `002` is a genuine change that must be applied.
- Prod id defaults: `uuid_generate_v4()` (36 tables, needs `uuid-ossp` ext) + `gen_random_uuid()`
  (8) + none (1). Canonical squashed schema uses `uuidv7()` everywhere → defaults differ on
  every id column.

## 3. Guiding principles

- **Back up first, twice** (custom-format dump + confirmed R2 backup). Keep the current binary.
- **Dry-run the entire cutover on a scratch PG18 DB on the same box** before touching `payroll_db`.
- **Schema-diff gate (§6):** the squash is only safe to re-baseline if it is *structurally
  identical* to the accumulated 26-migration schema (defaults aside). Diff and reconcile before cutover.
- **Maintenance window:** backend stopped during the cutover. Expected downtime ≈ a few minutes (12 MB DB).
- **Reversible:** keep the old binary + a pre-change dump so we can roll back to PG16 in minutes.

## 4. Step 0 — Build the amd64 Linux binary (do this anytime, off-box)

The box is `x86-64` and has no toolchain, so cross-build on the Mac via Docker (Docker is
installed locally but currently **not running** — start Docker Desktop first):

```bash
cd "/Users/goaltosuceed/Personal Projects/payroll-system"
# Box is x86-64; Mac is arm64 → force the target platform.
docker build --platform linux/amd64 -f infra/Dockerfile -t payroll-be:amd64 .
cid=$(docker create --platform linux/amd64 payroll-be:amd64)
docker cp "$cid":/app/payroll-system ./payroll-system.amd64
docker rm "$cid"
file ./payroll-system.amd64   # expect: ELF 64-bit LSB ... x86-64 ... for GNU/Linux
```

Gate it on a clean local check first: `cd backend && cargo fmt --check && cargo clippy -- -D warnings`
(tests need a DB). Do **not** install it on the box yet — that happens at cutover (§7).

## 5. Step 1 — Backups & safety net (on the box)

```bash
ssh -i <lightsail-key> ubuntu@13.251.162.88
sudo install -d -m 0700 /var/backups/payroll
# Full logical backup (schema + data + _sqlx_migrations)
sudo -u postgres pg_dump -Fc payroll_db | sudo tee /var/backups/payroll/payroll_db_pre18.dump >/dev/null
# Keep the current known-good binary for instant rollback
sudo cp -a /usr/local/bin/payroll-system /var/backups/payroll/payroll-system.pre18
# Force a fresh R2 backup and confirm it lands
sudo systemctl start payroll-backup.service && journalctl -u payroll-backup.service -n 30 --no-pager
```

## 6. Step 2 — Stand up PostgreSQL 18 + capture canonical references (no prod changes yet)

```bash
sudo apt-get update && sudo apt-get install -y postgresql-18   # 18.4 is in the pgdg repo
# New cluster comes up as 18/main on the next free port (e.g. :5433). Confirm:
pg_lsclusters
```

**Capture sqlx's canonical migration rows + the canonical schema** by running the *new* binary
against a throwaway PG18 DB. This avoids any hand-computed checksums.

```bash
# scratch DB on the PG18 cluster (adjust port to pg_lsclusters output)
sudo -u postgres psql -p 5433 -c "CREATE DATABASE scratch;"
# run new binary once, pointed at scratch, then stop it
sudo DATABASE_URL='postgres://postgres@localhost:5433/scratch' JWT_SECRET=x \
     /var/backups/payroll/payroll-system.new  &   # it auto-applies schema/001+002, then serves
sleep 5; kill %1
# Canonical migration table (transplanted into prod in §7):
sudo -u postgres psql -p 5433 -d scratch -c \
  "SELECT version, description, success, encode(checksum,'hex') FROM _sqlx_migrations ORDER BY version;"
# Canonical schema for the diff gate:
sudo -u postgres pg_dump -p 5433 --schema-only --no-owner scratch | sudo tee /var/backups/payroll/schema_canonical.sql >/dev/null
```

> Note: `/var/backups/payroll/payroll-system.new` is the amd64 binary from §4, scp'd to the box
> into that path (don't overwrite `/usr/local/bin` yet).

### Schema-diff GATE (must pass before cutover)

```bash
sudo -u postgres pg_dump --schema-only --no-owner payroll_db | sudo tee /var/backups/payroll/schema_prod.sql >/dev/null
diff <(sudo sed -E 's/DEFAULT (uuidv7|uuid_generate_v4|gen_random_uuid)\(\)//' /var/backups/payroll/schema_prod.sql) \
     <(sudo sed -E 's/DEFAULT (uuidv7|uuid_generate_v4|gen_random_uuid)\(\)//' /var/backups/payroll/schema_canonical.sql)
```

Expected/acceptable differences only:
- id-column **defaults** (`uuid_generate_v4`/`gen_random_uuid` vs `uuidv7`) — normalized out above.
- `users.role` present in prod, absent in canonical (handled by `002` in §7).
- `uuid-ossp` extension present in prod (harmless to keep).

**Any other structural difference** (table/column/type/constraint/index added, dropped, or
renamed) means the squash is NOT a faithful representation of prod → **STOP** and reconcile by
hand (add a forward migration, or fall back to the data-reload approach in the Appendix) before
proceeding.

## 7. Step 3 — Cutover (maintenance window)

```bash
# 1. Stop traffic to the app (Caddy can stay up; it'll 502 briefly)
sudo systemctl stop payroll-backend

# 2. Final pre-upgrade dump (data may have changed since §5)
sudo -u postgres pg_dump -Fc payroll_db | sudo tee /var/backups/payroll/payroll_db_final.dump >/dev/null

# 3. Upgrade the data 16 → 18 (12 MB → dump/restore is simplest & clean)
sudo pg_dropcluster --stop 18 main            # remove the empty PG18 cluster from §6 if it holds only scratch
sudo pg_upgradecluster 16 main                # migrates 16/main → new 18/main, preserving data + _sqlx_migrations
pg_lsclusters                                  # ensure 18/main is online and on :5432 (swap ports if needed)
sudo pg_dropcluster --stop 16 main            # only AFTER verifying 18/main is good

# 4. Apply the ONE genuine new change (002) by hand, since we re-baseline the history
sudo -u postgres psql -d payroll_db -c "ALTER TABLE public.users DROP COLUMN IF EXISTS role;"
# (optional, to match canonical going forward) re-point id defaults to uuidv7():
#   generate ALTERs from information_schema and run them — purely cosmetic for existing rows.

# 5. Re-baseline _sqlx_migrations to exactly what the new binary expects (use §6 capture)
sudo -u postgres psql -d payroll_db <<'SQL'
BEGIN;
DELETE FROM _sqlx_migrations;
INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time)
VALUES
 (1, 'schema',          now(), true, decode('<HEX-FROM-SCRATCH-v1>','hex'), 0),
 (2, 'drop users role', now(), true, decode('<HEX-FROM-SCRATCH-v2>','hex'), 0);
COMMIT;
SQL

# 6. Install the new binary and start
sudo install -o root -g root -m 0755 /var/backups/payroll/payroll-system.new /usr/local/bin/payroll-system
sudo systemctl start payroll-backend
sudo journalctl -u payroll-backend -n 40 --no-pager   # expect: migrate sees 0 pending, server binds :8080
```

## 8. Step 4 — Verify

```bash
curl -fsS http://127.0.0.1:8080/api/health           # on box
curl -fsS https://api.payrollmy.com/api/health        # through Caddy
```
Then a real smoke test from the browser: log in, list employees, open a payroll run / payslip,
attendance summary — i.e. exercise the previously-applied features (roles array, pending approval)
to confirm the re-baselined schema behaves.

## 9. Rollback (if any step fails)

```bash
sudo systemctl stop payroll-backend
sudo cp -a /var/backups/payroll/payroll-system.pre18 /usr/local/bin/payroll-system   # old binary
# Restore PG16: recreate 16/main, restore the final dump, point :5432 back to it
sudo -u postgres pg_restore -C -d postgres /var/backups/payroll/payroll_db_final.dump
sudo systemctl start payroll-backend
```
Because the old binary + old `_sqlx_migrations` (1–26) match the restored PG16 DB, this returns to
the exact pre-cutover state. Frontend is unaffected (it only talks to `/api`).

## 10. Open decisions to confirm before executing

1. **Maintenance window** — a few minutes of `api.payrollmy.com` downtime is required. When?
2. **Schema-diff result** — if §6 shows differences beyond defaults/`role`, decide: hand-written
   forward migration vs. data-reload (Appendix) vs. adjust the squash.
3. **id default policy** — leave existing `uuid_generate_v4`/`gen_random_uuid` defaults, or ALTER
   to `uuidv7()` to match canonical? (Affects only *future* inserts; existing rows keep their ids.)
4. **`/etc/payroll-backend/env`** — confirm it carries every var the new binary needs (the env file
   stays on the box and is untouched by this plan).

## Appendix — Alternative: clean rebuild + data reload

If the schema-diff gate reveals too much structural drift to reconcile, abandon in-place
re-baselining and instead: let the new binary build a *fresh* canonical schema on PG18, then load
**data only** from the old DB (`pg_dump --data-only --column-inserts`, excluding the dropped
`users.role`). This guarantees the canonical schema but shifts risk to data-load compatibility
(column/type matches), so it needs the same diff work — hence in-place is preferred for this 12 MB DB.

---

### Related infra note (separate from this deploy)
The `infra/` Terraform describes an EC2 + ECR + RDS + Docker architecture that is **not** what runs
in production (the hand-built Lightsail box above). Five dead EC2/Docker-era operational scripts
were removed in this change. The `.tf` files were left untouched pending an audit of which still
manage live resources (the S3 `payroll-dev-frontend` bucket + CloudFront `ED4843A8VKOA2` appear to
match the Terraform naming and may still be state-managed). Two removed scripts contained
**committed plaintext secrets** (an SMTP password and a Gmail app password) — rotate those, as they
remain in git history.
