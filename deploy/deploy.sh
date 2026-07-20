#!/usr/bin/env bash
# Deploy the payroll backend on the shared Lightsail VPS.
#
# Usage (as root):
#   deploy.sh <40-character-git-sha> <extracted-release-directory>
#
# The GitHub workflow builds a linux/amd64 image and transfers it over SSH.
# This script verifies and loads that image, preserves host-generated secrets
# and application data, waits for every service to become healthy, and rolls
# the backend image back when a release fails. It never contacts ECR, S3,
# SSM, Secrets Manager, Route53, or another billable deployment service.
#
# secrets.env is intentionally NOT auto-generated here (unlike sibling apps
# on this VPS): payroll's database was migrated from a pre-existing native
# Postgres cluster, and POSTGRES_PASSWORD/JWT_SECRET must be the values
# chosen during that migration (JWT_SECRET reused from the native process so
# existing sessions keep validating). A missing secrets.env fails loudly
# instead of inventing values that would point at an empty database.
set -Eeuo pipefail
umask 077

readonly APP_DIR=/opt/payroll
readonly RELEASES_DIR="$APP_DIR/releases"
readonly COMPOSE_FILE="$APP_DIR/docker-compose.prod.yml"
readonly SECRETS_FILE="$APP_DIR/secrets.env"
readonly CURRENT_TAG_FILE="$APP_DIR/current-tag"
readonly BACKUP_DIR="$APP_DIR/backups"
readonly LOCK_FILE="$APP_DIR/deploy.lock"
readonly CADDY_FILE=/etc/caddy/Caddyfile
readonly CADDY_SITE_FILE=/etc/caddy/payroll.Caddyfile
readonly API_DOMAIN="${PAYROLL_API_DOMAIN:-api.payrollmy.com}"
readonly POSTGRES_IMAGE=postgres:19beta2
readonly COMPOSE_VERSION=v5.1.4
readonly COMPOSE_SHA256=33b208d7e76639db742fae84b966cc01dacae58ca3fc4dabbc907045aefdf0c4

TAG="${1:-}"
RELEASE_DIR="${2:-}"
COMPOSE_COMMAND=()

log() {
  printf '[payroll-deploy] %s\n' "$*"
}

die() {
  printf '[payroll-deploy] ERROR: %s\n' "$*" >&2
  exit 1
}

[[ $EUID -eq 0 ]] || die "run this script as root (sudo)"
[[ "$TAG" =~ ^[0-9a-f]{40}$ ]] || die "image tag must be a 40-character lowercase Git commit SHA"
[[ -n "$RELEASE_DIR" && -d "$RELEASE_DIR" ]] || die "release directory does not exist: $RELEASE_DIR"

install -d -m 0750 "$APP_DIR" "$RELEASES_DIR"
exec 9>"$LOCK_FILE"
flock -n 9 || die "another payroll deployment is already running"

required_payload=(
  deploy.sh
  docker-compose.prod.yml
  SHA256SUMS
  images/backend.tar.gz
)
for payload in "${required_payload[@]}"; do
  [[ -f "$RELEASE_DIR/$payload" ]] || die "release payload is missing $payload"
done

(
  cd "$RELEASE_DIR"
  sha256sum --check SHA256SUMS
) || die "release checksum verification failed"

ensure_host_runtime() {
  [[ $(uname -m) == x86_64 ]] || die "this release targets the confirmed x86-64 Lightsail host"

  local packages=()
  command -v curl >/dev/null 2>&1 || packages+=(ca-certificates curl)
  command -v docker >/dev/null 2>&1 || packages+=(docker.io)
  command -v gzip >/dev/null 2>&1 || packages+=(gzip)
  command -v logrotate >/dev/null 2>&1 || packages+=(logrotate)

  if (( ${#packages[@]} > 0 )); then
    log "Installing the Docker runtime prerequisites (first deployment only)"
    apt-get update
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends "${packages[@]}"
  fi

  systemctl enable --now docker

  if docker compose version >/dev/null 2>&1; then
    COMPOSE_COMMAND=(docker compose)
  else
    log "Installing the pinned Docker Compose plugin (first deployment only)"
    local plugin_dir=/usr/local/lib/docker/cli-plugins
    local plugin_tmp
    install -d -m 0755 "$plugin_dir"
    plugin_tmp=$(mktemp "$plugin_dir/docker-compose.XXXXXX")
    curl --fail --silent --show-error --location \
      "https://github.com/docker/compose/releases/download/$COMPOSE_VERSION/docker-compose-linux-x86_64" \
      --output "$plugin_tmp"
    printf '%s  %s\n' "$COMPOSE_SHA256" "$plugin_tmp" | sha256sum --check --status - \
      || die "downloaded Docker Compose plugin failed checksum verification"
    chmod 0755 "$plugin_tmp"
    mv "$plugin_tmp" "$plugin_dir/docker-compose"
    docker compose version >/dev/null 2>&1 || die "Docker Compose plugin installation failed"
    COMPOSE_COMMAND=(docker compose)
  fi

  command -v caddy >/dev/null 2>&1 || die "host Caddy is missing; this VPS depends on it, so install/repair it manually"
  [[ -f "$CADDY_FILE" ]] || die "host Caddyfile is missing: $CADDY_FILE"
}

ensure_capacity() {
  local available_kib
  available_kib=$(df --output=avail -k "$APP_DIR" | tail -n 1 | tr -d ' ')
  [[ "$available_kib" =~ ^[0-9]+$ ]] || die "could not determine free disk space"
  (( available_kib >= 4194304 )) \
    || die "at least 4 GiB of free disk is required for images, rollback, and backup"
}

# secrets.env must already exist, seeded during the one-time migration runbook
# with the real POSTGRES_PASSWORD chosen for the new container-local DB user
# and the JWT_SECRET copied from the native backend's existing configuration.
ensure_secrets() {
  [[ -f "$SECRETS_FILE" ]] \
    || die "$SECRETS_FILE is missing. Seed it first (see the migration runbook) with POSTGRES_PASSWORD and JWT_SECRET; this script will not invent credentials for a production database."

  chmod 0600 "$SECRETS_FILE"
  set -a
  # This root-owned file is populated by the migration runbook, not by this script.
  # shellcheck disable=SC1090
  source "$SECRETS_FILE"
  set +a

  [[ "${POSTGRES_PASSWORD:-}" =~ ^[A-Za-z0-9._~-]{32,}$ ]] \
    || die "POSTGRES_PASSWORD in $SECRETS_FILE must be at least 32 URL-safe characters"
  local jwt_value=${JWT_SECRET:-}
  (( ${#jwt_value} >= 32 )) || die "JWT_SECRET in $SECRETS_FILE must be at least 32 characters"
}

install_release_files() {
  install -m 0644 "$RELEASE_DIR/docker-compose.prod.yml" "$COMPOSE_FILE"
  install -m 0750 "$RELEASE_DIR/deploy.sh" "$APP_DIR/deploy.sh"

  # The backend image runs as uid/gid 10001 (see infra/Dockerfile). Bind-mounted
  # application state must stay writable by that non-root user.
  install -d -m 0750 -o 10001 -g 10001 \
    "$APP_DIR/data" \
    "$APP_DIR/data/uploads"

  cat > /etc/logrotate.d/payroll <<'LOGROTATE'
/opt/payroll/logs/*.log {
    daily
    maxsize 10M
    rotate 7
    missingok
    notifempty
    compress
    delaycompress
    copytruncate
}
LOGROTATE
  chmod 0644 /etc/logrotate.d/payroll
}

load_release_images() {
  log "Loading application image for $TAG"
  gzip -dc "$RELEASE_DIR/images/backend.tar.gz" | docker load >/dev/null
  docker image inspect "payroll-backend:$TAG" >/dev/null
  [[ $(docker image inspect --format '{{.Architecture}}' "payroll-backend:$TAG") == amd64 ]] \
    || die "backend image architecture is not amd64"

  if ! docker image inspect "$POSTGRES_IMAGE" >/dev/null 2>&1; then
    log "Pulling $POSTGRES_IMAGE (first deployment only)"
    docker pull "$POSTGRES_IMAGE" >/dev/null
  fi
}

compose() {
  "${COMPOSE_COMMAND[@]}" \
    --project-name payroll \
    --file "$COMPOSE_FILE" \
    "$@"
}

container_health() {
  docker inspect \
    --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' \
    "$1" 2>/dev/null || true
}

wait_for_healthy() {
  local container=$1
  local deadline=$((SECONDS + 240))
  local status
  while (( SECONDS < deadline )); do
    status=$(container_health "$container")
    case "$status" in
      healthy)
        log "$container is healthy"
        return 0
        ;;
      exited|dead|unhealthy)
        log "$container entered state: $status"
        return 1
        ;;
    esac
    sleep 3
  done
  log "$container did not become healthy before the timeout (last state: ${status:-missing})"
  return 1
}

deploy_tag() {
  local target_tag=$1
  export IMAGE_TAG=$target_tag
  compose config >/dev/null || return 1
  compose up --detach --remove-orphans || return 1
  wait_for_healthy payroll-db || return 1
  wait_for_healthy payroll-backend || return 1
  curl -fsS http://127.0.0.1:8080/api/health >/dev/null || return 1
}

show_diagnostics() {
  compose ps >&2 || true
  compose logs --no-color --tail 100 postgres backend >&2 || true
}

backup_existing_database() {
  [[ $(docker inspect --format '{{.State.Running}}' payroll-db 2>/dev/null || true) == true ]] || return 0

  install -d -m 0700 "$BACKUP_DIR"
  local timestamp backup_tmp backup_path
  timestamp=$(date -u +%Y%m%dT%H%M%SZ)
  backup_path="$BACKUP_DIR/predeploy-$timestamp.dump"
  backup_tmp=$(mktemp "$BACKUP_DIR/.predeploy.XXXXXX")
  log "Creating local pre-deploy database backup"
  if docker exec payroll-db \
    pg_dump --format=custom --no-owner --no-acl -U payroll payroll_db \
    > "$backup_tmp"; then
    chmod 0600 "$backup_tmp"
    mv "$backup_tmp" "$backup_path"
  else
    rm -f "$backup_tmp"
    die "database backup failed; refusing to deploy"
  fi

  local backups=() index
  mapfile -t backups < <(
    find "$BACKUP_DIR" -maxdepth 1 -type f -name 'predeploy-*.dump' -printf '%T@ %p\n' \
      | sort -nr \
      | cut -d' ' -f2-
  )
  for ((index = 5; index < ${#backups[@]}; index++)); do
    rm -f -- "${backups[$index]}"
  done
}

configure_caddy() {
  local site_tmp main_backup site_backup=""
  site_tmp=$(mktemp "$APP_DIR/.payroll.Caddyfile.XXXXXX")
  main_backup=$(mktemp "$APP_DIR/.Caddyfile.XXXXXX")
  cp -p "$CADDY_FILE" "$main_backup"
  if [[ -f "$CADDY_SITE_FILE" ]]; then
    site_backup=$(mktemp "$APP_DIR/.payroll.Caddyfile.previous.XXXXXX")
    cp -p "$CADDY_SITE_FILE" "$site_backup"
  fi

  cat > "$site_tmp" <<CADDY
${API_DOMAIN} {
    encode zstd gzip

    header ?Strict-Transport-Security "max-age=31536000; includeSubDomains"

    reverse_proxy 127.0.0.1:8080
}
CADDY

  install -m 0644 "$site_tmp" "$CADDY_SITE_FILE"
  if ! grep -Fqx "import $CADDY_SITE_FILE" "$CADDY_FILE"; then
    printf '\n# Payroll backend (managed by /opt/payroll/deploy.sh)\nimport %s\n' \
      "$CADDY_SITE_FILE" >> "$CADDY_FILE"
  fi

  if ! caddy validate --config "$CADDY_FILE"; then
    cp -p "$main_backup" "$CADDY_FILE"
    if [[ -n "$site_backup" ]]; then
      cp -p "$site_backup" "$CADDY_SITE_FILE"
    else
      rm -f "$CADDY_SITE_FILE"
    fi
    rm -f "$site_tmp" "$main_backup"
    [[ -z "$site_backup" ]] || rm -f "$site_backup"
    return 1
  fi

  if ! systemctl reload caddy; then
    cp -p "$main_backup" "$CADDY_FILE"
    if [[ -n "$site_backup" ]]; then
      cp -p "$site_backup" "$CADDY_SITE_FILE"
    else
      rm -f "$CADDY_SITE_FILE"
    fi
    systemctl reload caddy || true
    rm -f "$site_tmp" "$main_backup"
    [[ -z "$site_backup" ]] || rm -f "$site_backup"
    return 1
  fi

  rm -f "$site_tmp" "$main_backup"
  [[ -z "$site_backup" ]] || rm -f "$site_backup"
}

cleanup_old_releases() {
  local keep_current=$1 keep_previous=$2 directory basename
  for directory in "$RELEASES_DIR"/*; do
    [[ -d "$directory" ]] || continue
    basename=${directory##*/}
    if [[ "$basename" != "$keep_current" && "$basename" != "$keep_previous" ]]; then
      rm -rf -- "$directory"
    fi
  done
}

cleanup_old_images() {
  local repository=$1 keep_current=$2 keep_previous=$3 image_tag
  while IFS= read -r image_tag; do
    [[ -n "$image_tag" && "$image_tag" != '<none>' ]] || continue
    if [[ "$image_tag" != "$keep_current" && "$image_tag" != "$keep_previous" ]]; then
      docker image rm "$repository:$image_tag" >/dev/null 2>&1 || true
    fi
  done < <(docker image ls "$repository" --format '{{.Tag}}')
}

ensure_host_runtime
ensure_capacity
ensure_secrets
backup_existing_database
install_release_files
load_release_images

previous_tag=""
if [[ -s "$CURRENT_TAG_FILE" ]]; then
  read -r previous_tag < "$CURRENT_TAG_FILE"
  if [[ ! "$previous_tag" =~ ^[0-9a-f]{40}$ ]]; then
    log "Ignoring malformed previous release marker"
    previous_tag=""
  fi
fi

log "Starting release $TAG"
if deploy_tag "$TAG" && configure_caddy; then
  printf '%s\n' "$TAG" > "$CURRENT_TAG_FILE"
  chmod 0600 "$CURRENT_TAG_FILE"
  cleanup_old_images payroll-backend "$TAG" "$previous_tag"
  cleanup_old_releases "$TAG" "$previous_tag"
  log "Release $TAG is healthy on 127.0.0.1:8080"
  log "Caddy is configured for https://${API_DOMAIN}"
  exit 0
fi

log "Release $TAG failed; collecting diagnostics"
export IMAGE_TAG=$TAG
show_diagnostics

if [[ -n "$previous_tag" ]] \
  && docker image inspect "payroll-backend:$previous_tag" >/dev/null 2>&1; then
  log "Rolling the backend container back to $previous_tag"
  if [[ -f "$RELEASES_DIR/$previous_tag/docker-compose.prod.yml" ]]; then
    install -m 0644 "$RELEASES_DIR/$previous_tag/docker-compose.prod.yml" "$COMPOSE_FILE"
  fi
  if deploy_tag "$previous_tag"; then
    log "Rollback succeeded"
  else
    log "Rollback failed; manual intervention is required"
    show_diagnostics
  fi
else
  log "No complete previous release is available for automatic rollback"
  compose stop backend >/dev/null 2>&1 || true
fi

die "deployment failed"
