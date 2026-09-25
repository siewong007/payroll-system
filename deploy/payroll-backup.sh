#!/usr/bin/env bash
# Nightly local database backup (plan item 1).
#
# Dumps the payroll_db volume the same way the pre-deploy path does
# (`pg_dump --format=custom --no-owner --no-acl`), verifies the archive is
# readable with `pg_restore --list`, then keeps the newest $LOCAL_KEEP dumps in
# /opt/payroll/backups.
#
# Local-only by owner decision (2026-09-25): the AWS account that held the
# off-host S3 copy was emptied, and no other cloud destination is used. A loss
# of the VPS therefore loses the live database AND these backups together;
# copy a dump off the host by hand before any risky maintenance.
#
# Restore: follow docs/database.md's pg_restore guidance against a fresh
# volume, using a nightly-*.dump from /opt/payroll/backups.
set -o errexit
set -o pipefail
set -o nounset

readonly APP_DIR="/opt/payroll"
readonly BACKUP_DIR="$APP_DIR/backups"
readonly LOG_DIR="$APP_DIR/logs"
readonly LOCAL_KEEP=14
readonly CONTAINER="payroll-db"

log() { printf '%s [backup] %s\n' "$(date -u +%Y-%m-%dT%H%M%SZ)" "$*" >> "$LOG_DIR/backup.log"; }
die() { log "FATAL: $*"; echo "payroll-backup: $*" >&2; exit 1; }

install -d -m 0700 "$BACKUP_DIR" -o root -g root
touch "$LOG_DIR/backup.log" && chmod 0640 "$LOG_DIR/backup.log" 2>/dev/null || true

# Verified before the lock: if flock were missing, `flock … || exit 0` below
# would misread every failure as "already running" and skip backups forever,
# silently.
command -v flock >/dev/null || die "flock is not installed"

# One backup at a time, ever. A second concurrent run would race the prune.
exec 9>/run/payroll-backup.lock
flock -n 9 || exit 0

docker inspect --format '{{.State.Running}}' "$CONTAINER" 2>/dev/null \
  | grep -q true || die "$CONTAINER is not running"

timestamp=$(date -u +%Y%m%dT%H%M%SZ)
final="$BACKUP_DIR/nightly-$timestamp.dump"
tmp_dump=$(mktemp "$BACKUP_DIR/.nightly.XXXXXX")
cleanup() { rm -f -- "$tmp_dump"; }
trap cleanup EXIT

log "Starting database dump"
# Same invocation as the pre-deploy backup so either artefact restores the
# same way. The container-local unix socket authenticates peer trust; no
# password crosses anything.
docker exec "$CONTAINER" \
  pg_dump --format=custom --no-owner --no-acl -U payroll payroll_db \
  > "$tmp_dump"

dump_bytes=$(stat -c '%s' "$tmp_dump")
(( dump_bytes > 512 )) || die "dump is suspiciously small ($dump_bytes bytes)"

# A dump that pg_restore cannot list is not a backup.
tables=$(docker exec -i "$CONTAINER" pg_restore --list < "$tmp_dump" \
  | grep -c 'TABLE DATA' || true)
(( tables > 0 )) || die "dump contains no table data"

chmod 0600 "$tmp_dump"
mv -- "$tmp_dump" "$final"
trap - EXIT
log "Verified $final ($dump_bytes bytes, $tables tables with data)"

mapfile -t old < <(
  find "$BACKUP_DIR" -maxdepth 1 -type f -name 'nightly-*.dump' -printf '%T@ %p\n' \
    | sort -nr | cut -d' ' -f2-
)
for ((i = LOCAL_KEEP; i < ${#old[@]}; i++)); do
  rm -f -- "${old[$i]}"
done

log "Backup complete: $(basename "$final")"
