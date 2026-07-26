#!/usr/bin/env bash
# One-time migration of the payroll backend + database from a native/manual
# deployment on the Lightsail VPS onto the dockerized stack in
# deploy/docker-compose.prod.yml.
#
# Run this BY HAND, as root, on the VPS itself (13.251.162.88). It is staged
# so you can run one stage at a time and stop between them:
#
#   sudo ./migrate-database.sh discover
#   sudo ./migrate-database.sh backup
#   sudo ./migrate-database.sh restore
#   sudo ./migrate-database.sh verify
#   sudo ./migrate-database.sh cutover
#   sudo ./migrate-database.sh rollback [--force] [--skip-dump]
#
# Data safety rules this script follows:
#   - The native Postgres cluster is NEVER stopped, altered, or written to.
#     Everything up through `verify` only reads from it (pg_dump is a
#     consistent read-only snapshot).
#   - No stage prints POSTGRES_PASSWORD or JWT_SECRET to stdout. Discovered
#     secrets are written straight into /opt/payroll/secrets.env (root-only,
#     0600) so they can be safely reported on without ever appearing in a
#     terminal scrollback or being pasted into chat.
#   - `verify` is a hard gate: `cutover` refuses to run unless verify's last
#     result was PASS *for the dump file that is still on disk*, and unless that
#     verdict is recent.
#   - `cutover` only stops the native BACKEND process. The native Postgres
#     cluster is left running, untouched.
#
# What `verify` does and does NOT prove:
#   It proves the dump restored faithfully — the two sides hold the same tables,
#   the same row counts, and the same created_at/updated_at high-water marks as
#   of the instant `backup` ran. It cannot see anything the native backend
#   committed AFTER that instant, because the native backend is still serving.
#   That window is the real data-loss risk in this migration, so backup ->
#   restore -> verify -> cutover must be run back to back; the gate in `cutover`
#   enforces a 60-minute bound on the verdict's age for exactly that reason.
#
# Rollback after cutover is NOT free:
#   The native cluster is a rollback target for the SCHEMA, not for the data.
#   Once the dockerized stack is serving, every row it commits lives only in the
#   payroll_postgres_data volume; the native cluster has none of them. `rollback`
#   therefore dumps the dockerized database first and refuses without --force
#   when that database holds rows written after the cutover.
set -Eeuo pipefail
umask 077

readonly APP_DIR=/opt/payroll
readonly BACKUP_DIR="$APP_DIR/backups"
readonly SECRETS_FILE="$APP_DIR/secrets.env"
readonly DISCOVERY_FILE="$APP_DIR/.migration-discovery"
readonly VERIFY_RESULT_FILE="$APP_DIR/.migration-verify-result"
readonly DUMP_FILE="$BACKUP_DIR/native-cutover.dump"
readonly COMPOSE_FILE="$APP_DIR/docker-compose.prod.yml"
readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# How stale a PASS may be and still open the cutover gate. The staged workflow
# is one click per stage, so a verdict from days ago certified a dump the native
# backend has long since diverged from.
readonly MAX_VERIFY_AGE_SECONDS=3600

log()  { printf '[migrate] %s\n' "$*"; }
die()  { printf '[migrate] ERROR: %s\n' "$*" >&2; exit 1; }
warn() { printf '[migrate] WARNING: %s\n' "$*" >&2; }

[[ $EUID -eq 0 ]] || die "run this script as root (sudo)"
install -d -m 0750 "$APP_DIR" "$BACKUP_DIR"

compose() {
  if docker compose version >/dev/null 2>&1; then
    docker compose --project-name payroll --file "$COMPOSE_FILE" "$@"
  else
    docker-compose --project-name payroll --file "$COMPOSE_FILE" "$@"
  fi
}

# docker-compose.prod.yml interpolates ${POSTGRES_PASSWORD:?} and ${IMAGE_TAG:?}
# as MANDATORY variables, and Compose resolves them when it LOADS the file — for
# every subcommand, `stop` included. A stage that forgets to export them fails
# before it touches a container, which is how `compose stop backend` could
# "succeed" while payroll-backend kept holding 127.0.0.1:8080.
load_compose_env() {
  [[ -f "$COMPOSE_FILE" ]] || die "$COMPOSE_FILE is missing — cannot address the dockerized stack"
  [[ -f "$SECRETS_FILE" ]] || die "$SECRETS_FILE is missing — cannot address the dockerized stack"
  set -a
  # shellcheck disable=SC1090
  . "$SECRETS_FILE"
  set +a
  export IMAGE_TAG="${IMAGE_TAG:-$(cat "$APP_DIR/current-tag" 2>/dev/null || printf 'compose-noop')}"
}

# ---------------------------------------------------------------------------
# discover: read-only. Finds the native backend's systemd unit and its
# DATABASE_URL/JWT_SECRET, and confirms the native Postgres port. Writes
# secrets straight to secrets.env (never echoed) plus a fresh, dedicated
# POSTGRES_PASSWORD for the new container-local DB user.
# ---------------------------------------------------------------------------
stage_discover() {
  log "Searching for a native payroll backend systemd unit..."
  local unit=""
  unit=$(systemctl list-units --type=service --all --no-legend 2>/dev/null \
    | awk '{print $1}' | grep -i payroll | head -n1 || true)
  if [[ -z "$unit" ]]; then
    warn "No systemd unit matched 'payroll'. Checking what's listening on :8080 instead."
    ss -tlnp 2>/dev/null | grep ':8080' || warn "Nothing is listening on :8080 either."
  else
    log "Found candidate unit: $unit"
    systemctl show "$unit" -p FragmentPath -p EnvironmentFiles 2>/dev/null || true
  fi

  log "Checking what's listening on :5432 (native Postgres)..."
  ss -tlnp 2>/dev/null | grep ':5432' || warn "Nothing listening on :5432 — confirm the native cluster's actual port manually."

  log "Attempting to locate the backend's environment file..."
  local env_candidates=()
  if [[ -n "$unit" ]]; then
    local frag env_file
    frag=$(systemctl show "$unit" -p FragmentPath --value 2>/dev/null || true)
    env_file=$(systemctl show "$unit" -p EnvironmentFiles --value 2>/dev/null | awk '{print $1}' || true)
    [[ -n "$env_file" ]] && env_candidates+=("$env_file")
    [[ -n "$frag" ]] && env_candidates+=("$frag")
  fi
  env_candidates+=(/opt/payroll/.env /opt/payroll/env /etc/payroll/env /etc/payroll.env /home/ubuntu/payroll/.env)

  local found_database_url="" found_jwt_secret=""
  local candidate
  for candidate in "${env_candidates[@]}"; do
    [[ -f "$candidate" ]] || continue
    log "Reading $candidate"
    if [[ -z "$found_database_url" ]]; then
      found_database_url=$(grep -oP '(?<=^DATABASE_URL=).*' "$candidate" 2>/dev/null | head -n1 | tr -d '"'"'" || true)
    fi
    if [[ -z "$found_jwt_secret" ]]; then
      found_jwt_secret=$(grep -oP '(?<=^JWT_SECRET=).*' "$candidate" 2>/dev/null | head -n1 | tr -d '"'"'" || true)
    fi
  done

  if [[ -z "$found_database_url" || -z "$found_jwt_secret" ]]; then
    die "Could not auto-discover DATABASE_URL/JWT_SECRET. Find the native backend's env file manually, then create $SECRETS_FILE by hand (POSTGRES_PASSWORD=<new value>, JWT_SECRET=<copied from the native config>) before continuing to 'backup'."
  fi

  local new_pg_password
  new_pg_password=$(openssl rand -hex 32)

  # Emit a value as a shell single-quoted literal so `source secrets.env`
  # reads it verbatim. The reused native JWT_SECRET can contain shell
  # metacharacters ({ } $ ? etc.) that would otherwise be expanded.
  sq() {
    local s=$1
    s=${s//\'/\'\\\'\'}
    printf "'%s'" "$s"
  }

  install -d -m 0750 "$APP_DIR"
  {
    printf 'POSTGRES_PASSWORD=%s\n' "$(sq "$new_pg_password")"
    printf 'JWT_SECRET=%s\n' "$(sq "$found_jwt_secret")"
  } > "$SECRETS_FILE"
  chmod 0600 "$SECRETS_FILE"

  # Record ONLY non-secret metadata for you to review/share.
  {
    printf 'unit=%s\n' "${unit:-unknown}"
    printf 'native_database_url_host=%s\n' "$(printf '%s' "$found_database_url" | sed -E 's#.*@([^/]+)/.*#\1#')"
    printf 'native_database_url_dbname=%s\n' "$(printf '%s' "$found_database_url" | sed -E 's#.*/([^/?]+).*#\1#')"
    printf 'jwt_secret_length=%s\n' "${#found_jwt_secret}"
    printf 'secrets_file_written=%s\n' "$SECRETS_FILE"
    printf 'discovered_at=%s\n' "$(date -u +%FT%TZ)"
  } > "$DISCOVERY_FILE"
  chmod 0600 "$DISCOVERY_FILE"

  log "Done. Safe summary (no secret values):"
  cat "$DISCOVERY_FILE"
  log "The native JWT_SECRET (reused so existing sessions keep validating) and a"
  log "fresh POSTGRES_PASSWORD for the new container DB were written to"
  log "$SECRETS_FILE (root-only, 0600). backup/restore read the native cluster"
  log "over its local postgres socket, not from this file."
  log "Review the summary above, then run: sudo $0 backup"
}

# ---------------------------------------------------------------------------
# backup: read-only against the native cluster. Consistent snapshot dump.
# ---------------------------------------------------------------------------
stage_backup() {
  [[ -f "$DISCOVERY_FILE" ]] || die "run 'discover' first"
  local native_db
  native_db=$(grep -oP '(?<=^native_database_url_dbname=).*' "$DISCOVERY_FILE")
  [[ -n "$native_db" ]] || die "could not determine the native database name from $DISCOVERY_FILE"

  log "Dumping native database '$native_db' (read-only snapshot; the native cluster is not modified)"
  local tmp
  tmp=$(mktemp "$BACKUP_DIR/.native-cutover.XXXXXX")
  if ! sudo -u postgres pg_dump --format=custom --no-owner --no-acl "$native_db" > "$tmp" 2>"$tmp.err"; then
    cat "$tmp.err" >&2
    rm -f "$tmp" "$tmp.err"
    die "pg_dump failed against the native cluster; nothing was touched"
  fi
  rm -f "$tmp.err"
  chmod 0600 "$tmp"
  mv "$tmp" "$DUMP_FILE"

  local size checksum
  size=$(wc -c < "$DUMP_FILE" | tr -d ' ')
  checksum=$(sha256sum "$DUMP_FILE" | awk '{print $1}')
  log "Backup complete: $DUMP_FILE ($size bytes, sha256:$checksum)"
  log "This checksum is safe to share for confirmation. Next: sudo $0 restore"
}

# ---------------------------------------------------------------------------
# restore: starts the dockerized postgres EMPTY and restores the dump into
# it. Does not touch the native cluster.
# ---------------------------------------------------------------------------
stage_restore() {
  [[ -f "$DUMP_FILE" ]] || die "run 'backup' first"
  [[ -f "$SECRETS_FILE" ]] || die "run 'discover' first (secrets.env is missing)"
  [[ -f "$COMPOSE_FILE" ]] || install -m 0644 "$SCRIPT_DIR/docker-compose.prod.yml" "$COMPOSE_FILE"

  if docker inspect payroll-db >/dev/null 2>&1; then
    die "a payroll-db container already exists — this looks like restore already ran. Inspect manually before re-running (it will not overwrite an existing volume)."
  fi

  # Only postgres is started here, so any resolvable value satisfies compose's
  # mandatory ${IMAGE_TAG:?} interpolation.
  export IMAGE_TAG="restore-placeholder"
  load_compose_env

  log "Starting an empty dockerized Postgres 19beta2..."
  compose up --detach postgres
  log "Waiting for it to become healthy..."
  local deadline=$((SECONDS + 120))
  while (( SECONDS < deadline )); do
    [[ "$(docker inspect --format '{{.State.Health.Status}}' payroll-db 2>/dev/null)" == "healthy" ]] && break
    sleep 3
  done
  [[ "$(docker inspect --format '{{.State.Health.Status}}' payroll-db 2>/dev/null)" == "healthy" ]] \
    || die "dockerized Postgres did not become healthy"

  log "Restoring the dump into it..."
  docker exec -i payroll-db pg_restore \
    --username=payroll --dbname=payroll_db --no-owner --no-acl \
    < "$DUMP_FILE" \
    || die "pg_restore reported errors — inspect before proceeding to verify"

  log "Restore complete. Next: sudo $0 verify"
}

# ---------------------------------------------------------------------------
# verify: hard gate. Compares native vs restored copy. Writes PASS/FAIL.
# ---------------------------------------------------------------------------

# The projection compared on both sides for one table.
#
# A bare count(*) cannot see an UPDATE, and this schema is full of them: a leave
# approval flips leave_requests.status, an attendance correction rewrites
# check_out_at, a payroll run transitions status. Folding in whichever of
# created_at/updated_at the table actually has makes an edit-only divergence
# visible. _sqlx_migrations is compared by applied ceiling instead — its row
# count changes legitimately whenever the chain is rebaselined.
table_snapshot_sql() {
  local table=$1 timestamp_columns=$2 projection

  if [[ "$table" == "_sqlx_migrations" ]]; then
    printf "SELECT coalesce(max(version)::text, '') FROM public.\"%s\"" "$table"
    return 0
  fi

  projection="count(*)::text"
  case ",$timestamp_columns," in
    *,created_at,*) projection="$projection || '|' || coalesce(max(created_at)::text, '')" ;;
    *)              projection="$projection || '|'" ;;
  esac
  case ",$timestamp_columns," in
    *,updated_at,*) projection="$projection || '|' || coalesce(max(updated_at)::text, '')" ;;
    *)              projection="$projection || '|'" ;;
  esac
  printf 'SELECT %s FROM public."%s"' "$projection" "$table"
}

stage_verify() {
  [[ -f "$DISCOVERY_FILE" ]] || die "run 'discover' first"
  [[ -f "$DUMP_FILE" ]] || die "run 'backup' first — the verdict is bound to the dump it certifies"
  local native_db
  native_db=$(grep -oP '(?<=^native_database_url_dbname=).*' "$DISCOVERY_FILE")

  # Enumerate from the source rather than listing tables by hand. The previous
  # hardcoded five omitted every high-write table in the schema — attendance,
  # leave, claims, payroll items, audit logs — so a PASS certified almost
  # nothing, and any table a later migration adds was silently excluded too.
  local table_list_sql="SELECT tablename FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename"
  local native_tables=() docker_tables=()
  mapfile -t native_tables < <(sudo -u postgres psql -d "$native_db" -tAc "$table_list_sql")
  mapfile -t docker_tables < <(docker exec payroll-db psql -U payroll -d payroll_db -tAc "$table_list_sql")
  (( ${#native_tables[@]} > 0 )) || die "could not enumerate tables on the native cluster"
  (( ${#docker_tables[@]} > 0 )) || die "could not enumerate tables in the restored container database"

  local mismatches=0 missing extra
  missing=$(comm -23 \
    <(printf '%s\n' "${native_tables[@]}" | sort) \
    <(printf '%s\n' "${docker_tables[@]}" | sort))
  extra=$(comm -13 \
    <(printf '%s\n' "${native_tables[@]}" | sort) \
    <(printf '%s\n' "${docker_tables[@]}" | sort))
  if [[ -n "$missing" ]]; then
    warn "tables present natively but MISSING from the restored copy: $(tr '\n' ' ' <<< "$missing")"
    mismatches=$((mismatches + 1))
  fi
  if [[ -n "$extra" ]]; then
    warn "tables present in the restored copy but not natively: $(tr '\n' ' ' <<< "$extra")"
    mismatches=$((mismatches + 1))
  fi

  # One catalogue query for every table, so the per-table loop stays two round
  # trips rather than four.
  local -A timestamp_columns=()
  local catalogue_table catalogue_columns
  while IFS='|' read -r catalogue_table catalogue_columns; do
    [[ -n "$catalogue_table" ]] || continue
    timestamp_columns["$catalogue_table"]=$catalogue_columns
  done < <(sudo -u postgres psql -d "$native_db" -tAc "
    SELECT table_name || '|' || string_agg(column_name, ',' ORDER BY column_name)
    FROM information_schema.columns
    WHERE table_schema = 'public' AND column_name IN ('created_at', 'updated_at')
    GROUP BY table_name")

  local table snapshot_sql native_snapshot docker_snapshot
  for table in "${native_tables[@]}"; do
    [[ -n "$table" ]] || continue
    snapshot_sql=$(table_snapshot_sql "$table" "${timestamp_columns[$table]:-}")
    native_snapshot=$(sudo -u postgres psql -d "$native_db" -tAc "$snapshot_sql" 2>/dev/null || echo "ERR")
    docker_snapshot=$(docker exec payroll-db psql -U payroll -d payroll_db -tAc "$snapshot_sql" 2>/dev/null || echo "ERR")
    if [[ "$native_snapshot" == "ERR" || "$docker_snapshot" == "ERR" ]]; then
      warn "$table: could not read the comparison snapshot from one side (native=$native_snapshot docker=$docker_snapshot) — check manually"
      mismatches=$((mismatches + 1))
      continue
    fi
    if [[ "$native_snapshot" != "$docker_snapshot" ]]; then
      warn "$table: MISMATCH native=[$native_snapshot] docker=[$docker_snapshot] (count|max created_at|max updated_at)"
      mismatches=$((mismatches + 1))
    else
      log "$table: OK [$native_snapshot]"
    fi
  done

  # Bind the verdict to the artefact and to the clock. Without the checksum a
  # re-run of `backup` after a PASS still opened the gate, certifying a file
  # nobody compared; without the timestamp a verdict from last week did.
  local checksum
  checksum=$(sha256sum "$DUMP_FILE" | awk '{print $1}')
  if (( mismatches == 0 )); then
    printf 'PASS %s sha256=%s tables=%s\n' \
      "$(date -u +%FT%TZ)" "$checksum" "${#native_tables[@]}" > "$VERIFY_RESULT_FILE"
    chmod 0600 "$VERIFY_RESULT_FILE"
    log "VERIFY PASSED — ${#native_tables[@]} table(s) match on row count and timestamp watermarks."
    log "This verdict covers the native cluster as of the 'backup' snapshot only."
    log "Run the cutover now (the gate expires in $((MAX_VERIFY_AGE_SECONDS / 60)) minutes): sudo $0 cutover"
  else
    printf 'FAIL %s sha256=%s tables=%s\n' \
      "$(date -u +%FT%TZ)" "$checksum" "${#native_tables[@]}" > "$VERIFY_RESULT_FILE"
    chmod 0600 "$VERIFY_RESULT_FILE"
    die "VERIFY FAILED ($mismatches mismatch(es)) — do NOT cut over. The native cluster is untouched; investigate the restored copy."
  fi
}

# ---------------------------------------------------------------------------
# cutover: stops the NATIVE BACKEND ONLY (never the native Postgres), starts
# the full dockerized stack, points Caddy at it.
#
# This must be run from inside a CI-produced release directory
# (/opt/payroll/releases/<sha>/migrate-database.sh, bundled next to
# deploy.sh and images/backend.tar.gz by deploy-backend.yml) — the release
# directory's name IS the image tag, and that's the only place the loaded
# backend image actually lives. A bare `scp`'d copy of this one file will not
# work for this stage.
# ---------------------------------------------------------------------------
stage_cutover() {
  local force=0 argument
  for argument in "$@"; do
    case "$argument" in
      --force) force=1 ;;
      *) die "unknown cutover option: $argument (expected --force)" ;;
    esac
  done

  # This is a ONE-TIME native->docker cutover and production has already been
  # through it. Re-running it points Caddy at a stack whose database came from a
  # native dump that is now months stale. `restore` legitimately creates
  # payroll-db, so the signal that the cutover ALREADY happened is a backend
  # container or the release marker deploy.sh writes on success — not the
  # database container.
  if docker inspect payroll-backend >/dev/null 2>&1 || [[ -s "$APP_DIR/current-tag" ]]; then
    die "the dockerized backend has already been deployed on this host — the cutover is a one-time stage and has run. Use deploy/deploy.sh for ordinary releases, and 'rollback' only if you intend to hand service back to the native stack."
  fi

  [[ -f "$DISCOVERY_FILE" ]] || die "run 'discover' first"
  [[ -f "$DUMP_FILE" ]] || die "$DUMP_FILE is missing — run 'backup' first"
  [[ -f "$VERIFY_RESULT_FILE" ]] || die "verify has not run — refusing to cut over. Run 'verify' first."

  # The gate used to be `grep -q '^PASS'`, which a verdict from days ago passed,
  # and which was never tied to the dump it certified — re-running `backup`
  # after a PASS left the stale verdict authorising a file nobody compared.
  local verdict verdict_time verdict_sha dump_sha verdict_epoch age
  read -r verdict verdict_time verdict_sha _ < "$VERIFY_RESULT_FILE" || true
  [[ "$verdict" == "PASS" ]] \
    || die "verify's last result was '${verdict:-empty}', not PASS — refusing to cut over."

  verdict_sha=${verdict_sha#sha256=}
  dump_sha=$(sha256sum "$DUMP_FILE" | awk '{print $1}')
  [[ -n "$verdict_sha" && "$verdict_sha" == "$dump_sha" ]] \
    || die "the PASS on record does not match the dump now on disk (verified sha256=${verdict_sha:-none}, actual $dump_sha) — re-run 'verify'."

  verdict_epoch=$(date -u -d "$verdict_time" +%s 2>/dev/null || echo "")
  [[ -n "$verdict_epoch" ]] || die "could not parse the verify timestamp '$verdict_time' — re-run 'verify'."
  age=$(( $(date -u +%s) - verdict_epoch ))
  if (( age > MAX_VERIFY_AGE_SECONDS )) && (( ! force )); then
    die "the PASS is $((age / 60)) minutes old (limit $((MAX_VERIFY_AGE_SECONDS / 60))). The native backend has been committing since that snapshot and those rows are NOT in the dump. Re-run backup/restore/verify, or accept the loss with: sudo $0 cutover --force"
  fi

  # Locate the newest CI-produced release dir ourselves. This stage runs as
  # root, so it can read the 0750 /opt/payroll/releases tree (an unprivileged
  # ls cannot). The release bundle carries deploy.sh + images/backend.tar.gz.
  local release tag
  release=$(ls -1dt /opt/payroll/releases/*/ 2>/dev/null | head -n1 || true)
  release=${release%/}
  [[ -n "$release" && -d "$release" ]] \
    || die "no /opt/payroll/releases/<sha>/ found — let the Deploy Backend pipeline upload an image first"
  tag=$(basename "$release")
  [[ "$tag" =~ ^[0-9a-f]{40}$ ]] \
    || die "newest release dir name is not a 40-char SHA: $tag"
  [[ -f "$release/images/backend.tar.gz" ]] \
    || die "$release/images/backend.tar.gz is missing — incomplete release bundle"
  [[ -f "$release/deploy.sh" ]] \
    || die "$release/deploy.sh is missing — incomplete release bundle"

  local unit
  unit=$(grep -oP '(?<=^unit=).*' "$DISCOVERY_FILE")
  if [[ -n "$unit" && "$unit" != "unknown" ]]; then
    log "Stopping native backend service: $unit (native Postgres is left running)"
    systemctl stop "$unit"
    systemctl disable "$unit" || true
  else
    warn "No native backend unit was recorded — stop it manually before proceeding if one is still running on :8080"
  fi

  log "Starting the full dockerized stack (backend + postgres) with image tag $tag from $release ..."
  bash "$release/deploy.sh" "$tag" "$release" \
    || die "deploy.sh failed — the native backend service was already stopped; run 'sudo $0 rollback' to restore the native stack while you investigate"

  # From here on the two databases diverge. Recording the instant is what lets
  # `rollback` answer "what exists only in the container?" instead of guessing.
  printf 'cutover_at=%s\n' "$(date -u +%FT%TZ)" >> "$DISCOVERY_FILE"

  log "Cutover complete. Verify https://api.payrollmy.com/api/health and a real login before considering this done."
  log "The dockerized database is now authoritative. Every row committed from now on exists ONLY in the payroll_postgres_data volume — a rollback strands them (see 'rollback' below)."
}

# ---------------------------------------------------------------------------
# inspect: read-only. Dumps enough VPS state to plan the cutover safely
# (release dirs, docker state, native services/ports, existing Caddy config
# for the api domain). Changes nothing.
# ---------------------------------------------------------------------------
stage_inspect() {
  echo "=== /opt/payroll/releases (newest first) ==="
  ls -1dt /opt/payroll/releases/*/ 2>/dev/null || echo "(none)"
  local r
  r=$(ls -1dt /opt/payroll/releases/*/ 2>/dev/null | head -n1 || true)
  if [[ -n "$r" ]]; then
    echo "=== newest release contents ($r) ==="
    ls -la "${r%/}" 2>/dev/null || true
    ls -la "${r%/}/images" 2>/dev/null || true
  fi
  echo "=== docker containers ==="
  docker ps -a --format '{{.Names}}\t{{.Status}}\t{{.Ports}}' 2>/dev/null || echo "(docker unavailable)"
  echo "=== docker images (payroll-backend) ==="
  docker images payroll-backend --format '{{.Repository}}:{{.Tag}}' 2>/dev/null || true
  echo "=== native backend service ==="
  systemctl is-active payroll-backend.service 2>/dev/null || true
  systemctl is-enabled payroll-backend.service 2>/dev/null || true
  echo "=== listeners on :8080 and :5432 ==="
  ss -tlnp 2>/dev/null | grep -E ':8080|:5432' || echo "(nothing listening on 8080/5432)"
  echo "=== main Caddyfile: api.payrollmy.com block (first 14 lines of file) ==="
  sed -n '1,14p' /etc/caddy/Caddyfile 2>/dev/null || echo "(cannot read Caddyfile)"
  echo "=== main Caddyfile import lines ==="
  grep -n '^import\|import ' /etc/caddy/Caddyfile 2>/dev/null || echo "(no imports)"
  echo "=== secrets.env present? (values not shown) ==="
  if [[ -f "$SECRETS_FILE" ]]; then
    grep -oE '^[A-Z_]+=' "$SECRETS_FILE" 2>/dev/null | sed 's/=$//' | sed 's/^/  key: /'
  else
    echo "  (missing)"
  fi
}

# ---------------------------------------------------------------------------
# rollback: stop the dockerized backend, hand service back to the native one.
#
# This is NOT a free operation and never was. The native Postgres cluster was
# never stopped, but it also never received anything written after the cutover:
# every check-in, leave decision, claim approval and audit row since then lives
# only in the payroll_postgres_data volume. So this stage dumps the dockerized
# database first, counts what it is about to strand, and refuses without
# --force. It never removes the container or its volume.
#
#   --force      proceed even though post-cutover rows exist
#   --skip-dump  skip the pg_dump (only for a database confirmed to hold nothing)
# ---------------------------------------------------------------------------
stage_rollback() {
  local force=0 skip_dump=0 argument
  for argument in "$@"; do
    case "$argument" in
      --force) force=1 ;;
      --skip-dump) skip_dump=1 ;;
      *) die "unknown rollback option: $argument (expected --force and/or --skip-dump)" ;;
    esac
  done

  [[ -f "$DISCOVERY_FILE" ]] || die "no discovery record found — nothing to roll back to"
  local unit cutover_at
  unit=$(grep -oP '(?<=^unit=).*' "$DISCOVERY_FILE")
  # A discovery file written before cutover timestamping exists has no bookmark.
  # Degrade to "unknown", which the acknowledgement below treats as "assume rows
  # are stranded" rather than as "nothing to lose".
  cutover_at=$(grep -oP '(?<=^cutover_at=).*' "$DISCOVERY_FILE" | tail -n1 || true)
  [[ -n "$unit" && "$unit" != "unknown" ]] \
    || die "no native unit recorded — restart the native backend manually; nothing was changed"

  # Fatal, not best-effort. payroll-backend publishes 127.0.0.1:8080, so leaving
  # it up means the native unit cannot bind and the rollback silently achieves
  # nothing. Only `backend` is stopped: payroll-db holds the only copy of the
  # post-cutover data.
  log "Stopping the dockerized backend (payroll-db and its volume are left intact)..."
  load_compose_env
  compose stop backend \
    || die "could not stop the dockerized backend; it still publishes 127.0.0.1:8080 — do NOT start the native unit until it is stopped"

  local db_running=false
  if [[ "$(docker inspect --format '{{.State.Running}}' payroll-db 2>/dev/null || true)" == "true" ]]; then
    db_running=true
  fi

  # Dump while the writer is stopped, so the archive is quiescent.
  local dump="" stamp
  if (( skip_dump )); then
    warn "--skip-dump: the post-cutover database will NOT be dumped before it is abandoned."
  elif [[ "$db_running" != "true" ]]; then
    die "payroll-db is not running, so its contents cannot be dumped. Start it ('compose up -d postgres') and re-run, or pass --skip-dump if you have already confirmed it holds nothing."
  else
    stamp=$(date -u +%Y%m%dT%H%M%SZ)
    dump="$BACKUP_DIR/post-cutover-$stamp.dump"
    log "Dumping the post-cutover database before abandoning it..."
    if ! docker exec payroll-db \
      pg_dump --format=custom --no-owner --no-acl -U payroll payroll_db > "$dump.tmp"; then
      rm -f "$dump.tmp"
      die "could not dump the post-cutover database; refusing to roll back and strand it"
    fi
    chmod 0600 "$dump.tmp"
    mv "$dump.tmp" "$dump"
    log "Post-cutover dump: $dump"
  fi

  # Count what rolling back would strand. audit_logs alone makes this near
  # certain to trip after any real usage — which is the point: the operator has
  # to acknowledge the loss rather than read "no database action was needed".
  local stranded="unknown" probe_failures=0 candidate rows
  if [[ -n "$cutover_at" && "$db_running" == "true" ]]; then
    stranded=0
    for candidate in attendance_records leave_requests claims overtime_applications payroll_runs audit_logs; do
      rows=$(docker exec payroll-db psql -U payroll -d payroll_db -tAc \
        "SELECT count(*) FROM public.$candidate WHERE created_at > '$cutover_at'::timestamptz" 2>/dev/null || echo "")
      # A probe that cannot run must not read as a zero — that is the same
      # false reassurance this stage used to end on.
      if [[ ! "$rows" =~ ^[0-9]+$ ]]; then
        warn "  $candidate: could not be counted"
        probe_failures=$((probe_failures + 1))
        continue
      fi
      if (( rows > 0 )); then
        log "  $candidate: $rows row(s) written after the cutover"
      fi
      stranded=$((stranded + rows))
    done
  fi

  local must_acknowledge=0
  if [[ "$stranded" == "unknown" ]]; then
    warn "This discovery record predates cutover timestamping (or payroll-db is down), so what would be stranded cannot be counted. Assuming there is something."
    must_acknowledge=1
  elif (( probe_failures > 0 )); then
    warn "$probe_failures table(s) could not be counted, so $stranded is a floor and not a total."
    must_acknowledge=1
  elif (( stranded > 0 )); then
    must_acknowledge=1
  fi

  if (( must_acknowledge )) && (( ! force )); then
    log "The dockerized database holds $stranded row(s) written after ${cutover_at:-the cutover}."
    log "Rolling back hands service to the native cluster, which has none of them."
    if [[ -n "$dump" ]]; then
      log "They are preserved in: $dump"
    fi
    log "Re-run deliberately with: sudo $0 rollback --force"
    die "refusing to strand post-cutover data without an explicit --force. The dockerized backend is stopped; nothing else was changed."
  fi

  log "Restarting native backend service: $unit"
  systemctl enable "$unit" || true
  systemctl start "$unit"
  systemctl status "$unit" --no-pager || true

  log "Rollback complete. The native backend is serving again against the PRE-cutover native cluster."
  log "The dockerized database is NOT reconciled: $stranded row(s) written after ${cutover_at:-the cutover} exist ONLY in the payroll-db volume."
  if [[ -n "$dump" ]]; then
    log "A dump of that database is at: $dump"
  else
    log "No dump of it was taken (--skip-dump)."
  fi
  log "payroll-db and its volume were left intact. Replay or reconcile before calling this finished."
}

case "${1:-}" in
  discover) stage_discover ;;
  backup)   stage_backup ;;
  restore)  stage_restore ;;
  verify)   stage_verify ;;
  inspect)  stage_inspect ;;
  cutover)  shift; stage_cutover "$@" ;;
  rollback) shift; stage_rollback "$@" ;;
  *) die "usage: $0 {discover|backup|restore|verify|inspect|cutover [--force]|rollback [--force] [--skip-dump]}" ;;
esac
