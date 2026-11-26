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
  raw_webhook_line=$(grep -m1 -E '^[[:space:]]*ALERT_WEBHOOK_URL=' "$SECRETS_FILE" 2>/dev/null || true)
  if [[ -n "$raw_webhook_line" ]]; then
    raw_webhook=${raw_webhook_line#*=}
    raw_webhook=${raw_webhook%$'\r'}
    # Trim leading/trailing whitespace and matching single/double quotes without executing the file.
    raw_webhook=$(sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' <<<"$raw_webhook")
    if [[ ( $raw_webhook == \"*\" && $raw_webhook == *\" ) || ( $raw_webhook == \'*\' && $raw_webhook == *\' ) ]]; then
      raw_webhook=${raw_webhook:1:${#raw_webhook}-2}
    fi
    ALERT_WEBHOOK_URL=$raw_webhook
  fi
fi

log_msg() {
  local level=$1
  shift
  local ts
  ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  printf '%s [%s] %s\n' "$ts" "$level" "$*" | tee -a "$LOG_FILE" >&2 || true
  logger -t payroll-watchdog "[$level] $*" || true
}

json_escape_string() {
  local s=$1
  s=${s//\\/\\\\}
  s=${s//\"/\\\"}
  s=${s//$'\n'/\\n}
  s=${s//$'\r'/\\r}
  s=${s//$'\t'/\\t}
  # Strip remaining ASCII control characters (0x00-0x1f and 0x7f).
  s=$(LC_ALL=C tr -d '\000-\037\177' <<<"$s")
  printf '%s' "$s"
}

notify_webhook() {
  local message=$1
  [[ -n "${ALERT_WEBHOOK_URL:-}" ]] || return 0
  if [[ "$ALERT_WEBHOOK_URL" != https://* ]]; then
    log_msg WARN "Refusing insecure non-HTTPS ALERT_WEBHOOK_URL"
    return 0
  fi
  local payload=""
  if command -v jq >/dev/null 2>&1; then
    payload=$(jq -nc --arg text "$message" '{service: "payroll-backend", text: $text}' 2>/dev/null) || payload=""
  fi
  if [[ -z "$payload" ]]; then
    local escaped
    escaped=$(json_escape_string "$message")
    payload="{\"service\":\"payroll-backend\",\"text\":\"${escaped}\"}"
  fi
  if ! curl --fail --silent --show-error --max-time 10 \
    --proto =https --proto-redir =https --max-redirs 0 \
    -X POST \
    -H 'content-type: application/json' \
    --data-raw "$payload" \
    -- "$ALERT_WEBHOOK_URL" >/dev/null 2>&1; then
    log_msg WARN "Failed to deliver watchdog webhook alert"
  fi
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
