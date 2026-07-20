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
#   sudo ./migrate-database.sh rollback   # only if something goes wrong after cutover
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
#     result was PASS.
#   - `cutover` only stops the native BACKEND process. The native Postgres
#     cluster is left running, untouched, as an instant rollback target.
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

  set -a
  # shellcheck disable=SC1090
  source "$SECRETS_FILE"
  set +a
  export IMAGE_TAG="restore-placeholder"

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
stage_verify() {
  [[ -f "$DISCOVERY_FILE" ]] || die "run 'discover' first"
  local native_db
  native_db=$(grep -oP '(?<=^native_database_url_dbname=).*' "$DISCOVERY_FILE")

  local tables=(companies employees users payroll_runs _sqlx_migrations)
  local mismatches=0 table native_count docker_count

  for table in "${tables[@]}"; do
    native_count=$(sudo -u postgres psql -d "$native_db" -tAc "SELECT count(*) FROM $table" 2>/dev/null || echo "ERR")
    docker_count=$(docker exec payroll-db psql -U payroll -d payroll_db -tAc "SELECT count(*) FROM $table" 2>/dev/null || echo "ERR")
    if [[ "$native_count" == "ERR" || "$docker_count" == "ERR" ]]; then
      warn "$table: could not read count from one side (native=$native_count docker=$docker_count) — table may not exist, check manually"
      mismatches=$((mismatches + 1))
      continue
    fi
    if [[ "$native_count" != "$docker_count" ]]; then
      warn "$table: MISMATCH native=$native_count docker=$docker_count"
      mismatches=$((mismatches + 1))
    else
      log "$table: OK ($native_count rows both sides)"
    fi
  done

  if (( mismatches == 0 )); then
    printf 'PASS %s\n' "$(date -u +%FT%TZ)" > "$VERIFY_RESULT_FILE"
    log "VERIFY PASSED — all checked tables match. Safe to run: sudo $0 cutover"
  else
    printf 'FAIL %s\n' "$(date -u +%FT%TZ)" > "$VERIFY_RESULT_FILE"
    die "VERIFY FAILED ($mismatches mismatch(es)) — do NOT cut over. The native cluster is untouched; investigate the restored copy."
  fi
  chmod 0600 "$VERIFY_RESULT_FILE"
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
  [[ -f "$VERIFY_RESULT_FILE" ]] && grep -q '^PASS' "$VERIFY_RESULT_FILE" \
    || die "verify has not passed — refusing to cut over. Run 'verify' first."
  [[ -f "$DISCOVERY_FILE" ]] || die "run 'discover' first"

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

  log "Cutover complete. Verify https://api.payrollmy.com/api/health and a real login before considering this done."
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
  echo "=== /etc/caddy references to payroll/api domain ==="
  grep -rniE 'payrollmy|payroll|api\.' /etc/caddy/ 2>/dev/null || echo "(no matches)"
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
# rollback: stop the dockerized stack, restart the native backend. Native
# Postgres was never touched, so this is just restarting one service.
# ---------------------------------------------------------------------------
stage_rollback() {
  [[ -f "$DISCOVERY_FILE" ]] || die "no discovery record found — nothing to roll back to"
  local unit
  unit=$(grep -oP '(?<=^unit=).*' "$DISCOVERY_FILE")

  log "Stopping the dockerized stack..."
  compose stop backend 2>/dev/null || true

  if [[ -n "$unit" && "$unit" != "unknown" ]]; then
    log "Restarting native backend service: $unit"
    systemctl enable "$unit" || true
    systemctl start "$unit"
    systemctl status "$unit" --no-pager || true
  else
    die "no native unit recorded — restart the native backend manually"
  fi
  log "Rollback complete. Native Postgres was never stopped, so no database action was needed."
}

case "${1:-}" in
  discover) stage_discover ;;
  backup)   stage_backup ;;
  restore)  stage_restore ;;
  verify)   stage_verify ;;
  inspect)  stage_inspect ;;
  cutover)  stage_cutover ;;
  rollback) stage_rollback ;;
  *) die "usage: $0 {discover|backup|restore|verify|inspect|cutover|rollback}" ;;
esac
