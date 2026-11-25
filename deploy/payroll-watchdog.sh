#!/usr/bin/env bash
# Periodic operational watchdog for the payroll API on the AIC VPS (items 25 & 70).
#
# Checks:
#   1. Local readiness endpoint http://127.0.0.1:8080/api/health/ready responds HTTP 200.
#   2. Reported overall status is not "degraded" and no background job is marked stale or failed.
#   3. Filesystem usage under /opt/payroll does not exceed 85%.
#
# Optional webhook notification: if ALERT_WEBHOOK_URL is configured in
# /opt/payroll/secrets.env, failures POST a compact JSON payload (without leaking
# secrets) using curl. Failures are always written to syslog (logger) and
# /opt/payroll/logs/watchdog.log so systemd journal + logrotate capture them.
set -Eeuo pipefail

readonly APP_DIR=/opt/payroll
readonly LOG_FILE="$APP_DIR/logs/watchdog.log"
readonly SECRETS_FILE="$APP_DIR/secrets.env"
readonly HEALTH_URL="http://127.0.0.1:8080/api/health/ready"

mkdir -p "$APP_DIR/logs"

ALERT_WEBHOOK_URL=""
if [[ -r "$SECRETS_FILE" ]]; then
  set +u
  # shellcheck disable=SC1090
  source "$SECRETS_FILE" || true
  set -u
fi

log_msg() {
  local level=$1
  shift
  local ts
  ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  printf '%s [%s] %s\n' "$ts" "$level" "$*" | tee -a "$LOG_FILE" >&2 || true
  logger -t payroll-watchdog "[$level] $*" || true
}

notify_webhook() {
  local message=$1
  [[ -n "${ALERT_WEBHOOK_URL:-}" ]] || return 0
  curl --silent --show-error --max-time 10 \
    -X POST \
    -H 'content-type: application/json' \
    -d "{\"service\":\"payroll-backend\",\"text\":\"${message//\"/\\\"}\"}" \
    "$ALERT_WEBHOOK_URL" >/dev/null 2>&1 || true
}

issues=()

if ! ready_json=$(curl -fsS --max-time 10 "$HEALTH_URL" 2>/dev/null); then
  issues+=("API readiness probe failed on $HEALTH_URL")
else
  if grep -Eq '"status"[[:space:]]*:[[:space:]]*"degraded"' <<<"$ready_json"; then
    issues+=("API readiness reports degraded status: $ready_json")
  fi
  if grep -Eq '"stale"[[:space:]]*:[[:space:]]*true' <<<"$ready_json"; then
    issues+=("One or more background jobs are stale")
  fi
  if grep -Eq '"ok"[[:space:]]*:[[:space:]]*false' <<<"$ready_json"; then
    issues+=("One or more background jobs reported failure")
  fi
fi

disk_pct=$(df --output=pcent "$APP_DIR" | tail -n 1 | tr -dc '0-9')
if [[ -n "$disk_pct" ]] && (( disk_pct >= 85 )); then
  issues+=("Disk utilization under $APP_DIR is ${disk_pct}% (>= 85%)")
fi

if (( ${#issues[@]} > 0 )); then
  summary=$(printf '%s; ' "${issues[@]}")
  summary=${summary%; }
  log_msg ERROR "$summary"
  notify_webhook "PayrollMY VPS alert: $summary"
  exit 1
fi

exit 0
