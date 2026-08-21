#!/usr/bin/env bash
# Nightly encrypted off-host database backup (plan item 1).
#
# Dumps the payroll_db volume the same way the pre-deploy path does
# (`pg_dump --format=custom --no-owner --no-acl`), encrypts it client-side
# with `age`, uploads exactly one object to a versioned, lifecycle-managed
# S3 bucket, verifies the stored byte count, then prunes local copies.
# A host loss therefore costs at most one day of payroll history instead of
# "everything since the last merge", and the dump on S3 is unreadable there.
#
# One-time setup (operator):
#   1. terraform apply            -> creates the bucket and the IAM user
#   2. age-keygen -o /root/backup-age.key    # keep the PRIVATE key OFF this
#      host (password manager / offline); only its public recipient belongs
#      here, otherwise encryption is theatre
#   3. Append to /opt/payroll/secrets.env:
#        BACKUP_S3_BUCKET=<outputs.payroll_backups_bucket>
#        BACKUP_AWS_REGION=ap-southeast-1
#        AGE_RECIPIENT=<age1... public key>
#        BACKUP_AWS_ACCESS_KEY_ID=<key for outputs.backup_iam_user>
#        BACKUP_AWS_SECRET_ACCESS_KEY=<secret for that key>
#   4. systemctl start payroll-backup.service   # one manual run to prove it
#
# The restore path is deliberately manual too: decrypt with the private key,
# then follow docs/database.md's pg_restore guidance against a fresh volume.
set -o errexit
set -o pipefail
set -o nounset

readonly APP_DIR="/opt/payroll"
readonly SECRETS_FILE="$APP_DIR/secrets.env"
readonly BACKUP_DIR="$APP_DIR/backups"
readonly LOG_DIR="$APP_DIR/logs"
readonly LOCAL_KEEP=7
readonly CONTAINER="payroll-db"

log() { printf '%s [backup] %s\n' "$(date -u +%Y-%m-%dT%H%M%SZ)" "$*" >> "$LOG_DIR/backup.log"; }
die() { log "FATAL: $*"; exit 1; }

install -d -m 0700 "$BACKUP_DIR" -o root -g root
touch "$LOG_DIR/backup.log" && chmod 0640 "$LOG_DIR/backup.log" 2>/dev/null || true

# One backup at a time, ever. A second concurrent run would race the prune.
exec 9>/run/payroll-backup.lock
flock -n 9 || exit 0

[[ -f "$SECRETS_FILE" ]] || die "$SECRETS_FILE missing; see the header of this script"
chmod 0600 "$SECRETS_FILE"
set -a
# shellcheck disable=SC1090
source "$SECRETS_FILE"
set +a

: "${BACKUP_S3_BUCKET:?BACKUP_S3_BUCKET not configured in $SECRETS_FILE}"
: "${AGE_RECIPIENT:?AGE_RECIPIENT not configured in $SECRETS_FILE}"
: "${BACKUP_AWS_ACCESS_KEY_ID:?BACKUP_AWS_ACCESS_KEY_ID not configured}"
: "${BACKUP_AWS_SECRET_ACCESS_KEY:?BACKUP_AWS_SECRET_ACCESS_KEY not configured}"
export AWS_REGION="${BACKUP_AWS_REGION:-ap-southeast-1}"

command -v age >/dev/null || die "age is not installed"
command -v aws >/dev/null || die "aws cli is not installed"
# Verified before the lock: if flock were missing, `flock … || exit 0` below
# would misread every failure as "already running" and skip backups forever,
# silently.
command -v flock >/dev/null || die "flock is not installed"
docker inspect --format '{{.State.Running}}' "$CONTAINER" 2>/dev/null \
  | grep -q true || die "$CONTAINER is not running"

timestamp=$(date -u +%Y%m%dT%H%M%SZ)
object_key="db/nightly-$timestamp.dump.age"
tmp_dump=$(mktemp "$BACKUP_DIR/.nightly.XXXXXX")
tmp_enc="$tmp_dump.age"
cleanup() { rm -f -- "$tmp_dump" "$tmp_enc"; }
trap cleanup EXIT

log "Starting database dump"
# Same invocation as the pre-deploy backup so either artefact restores the
# same way. The container-local unix socket authenticates peer trust; no
# password crosses anything.
docker exec "$CONTAINER" \
  pg_dump --format=custom --no-owner --no-acl -U payroll payroll_db \
  > "$tmp_dump"

dump_bytes=$(stat -c '%s' "$tmp_dump")
(( dump_bytes > 512 )) || die "dump is suspiciously small ($dump_bytes bytes); refusing to upload"

log "Encrypting ($dump_bytes bytes)"
age -r "$AGE_RECIPIENT" -o "$tmp_enc" "$tmp_dump"
rm -f -- "$tmp_dump"
chmod 0600 "$tmp_enc"

log "Uploading s3://$BACKUP_S3_BUCKET/$object_key"
if ! aws s3 cp --only-show-errors "$tmp_enc" "s3://$BACKUP_S3_BUCKET/$object_key"; then
  die "S3 upload failed"
fi

remote_bytes=$(aws s3api head-object \
  --bucket "$BACKUP_S3_BUCKET" --key "$object_key" \
  --query 'ContentLength' --output text)
[[ "$remote_bytes" =~ ^[0-9]+$ ]] || die "could not read back uploaded size"
local_enc_bytes=$(stat -c '%s' "$tmp_enc")
[[ "$remote_bytes" == "$local_enc_bytes" ]] \
  || die "uploaded size mismatch: local=$local_enc_bytes remote=$remote_bytes"
log "Verified remote object ($remote_bytes bytes)"

# Local retention: the off-host copy is the recovery mechanism; these are
# convenience only, matching the pre-deploy dir's bounded-growth pattern.
mapfile -t old < <(
  find "$BACKUP_DIR" -maxdepth 1 -type f -name 'nightly-*.dump.age' -printf '%T@ %p\n' \
    | sort -nr | cut -d' ' -f2-
)
for ((i = LOCAL_KEEP; i < ${#old[@]}; i++)); do
  rm -f -- "${old[$i]}"
done

# ── Secrets escrow (plan item 3) ────────────────────────────────────────────
# A dump without POSTGRES_PASSWORD/JWT_SECRET/SMTP/OAuth credentials cannot be
# restored into a working system, so the same nightly flight carries an
# encrypted copy of secrets.env itself.
#
# Deliberate circularity, understood and accepted: the escrowed file contains
# BACKUP_AWS_ACCESS_KEY_ID/SECRET, so restoring requires (a) AWS account
# access to list/fetch the object AND (b) the age PRIVATE key to decrypt it —
# which never lives on this host. Neither alone is enough, and together they
# are exactly the two things an operator escrows after a fire.
secrets_enc="$BACKUP_DIR/secrets-$timestamp.env.age"
if [[ -s "$SECRETS_FILE" ]] \
    && age -r "$AGE_RECIPIENT" -o "$secrets_enc" "$SECRETS_FILE"; then
  chmod 0600 "$secrets_enc"
  if aws s3 cp --only-show-errors "$secrets_enc" \
      "s3://$BACKUP_S3_BUCKET/secrets/secrets-$timestamp.env.age"; then
    log "Secrets escrow uploaded"
  else
    log "WARNING: secrets escrow upload failed (database backup is still valid)"
  fi
  # One recent local copy is plenty; the bucket keeps the history.
  find "$BACKUP_DIR" -maxdepth 1 -type f -name 'secrets-*.env.age' \
    ! -name "$(basename "$secrets_enc")" -exec rm -f -- {} +
else
  log "WARNING: secrets escrow skipped (age encryption failed)"
fi

log "Backup complete: $object_key"
