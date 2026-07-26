#!/usr/bin/env bash
#
# Lightsail metric alarms for the payroll VPS.
#
# These are NOT in infra/*.tf on purpose: the AWS Terraform provider has no
# `aws_lightsail_alarm` resource (it covers instance, disk, lb, distribution,
# bucket, certificate, domain, key_pair, static_ip and container_service, but
# not alarms), and the Lightsail instance itself was never imported into state
# — infra/ manages only the S3/CloudFront/ACM/Route53 frontend path and the
# deploy IAM role. This script is the reproducible substitute: idempotent,
# re-runnable, and reviewable in the same repo as everything else.
#
# Run from anywhere with AWS credentials that can reach Lightsail:
#
#   ./deploy/lightsail-alarms.sh you@example.com [instance-name]
#
# The first run sends a confirmation email to the address you pass. Alarms do
# not fire until you click the link in it — contact methods are PER-REGION,
# so verifying in ap-southeast-1 does nothing for an alarm in us-east-1.
#
# Cost: $0. Lightsail alarms and email contact methods are included.

set -euo pipefail

readonly AWS_REGION="${AWS_REGION:-ap-southeast-1}"

log() { printf '[alarms] %s\n' "$*"; }
die() { printf '[alarms] ERROR: %s\n' "$*" >&2; exit 1; }

readonly ALERT_EMAIL="${1:-${ALERT_EMAIL:-}}"
[[ -n "$ALERT_EMAIL" ]] \
  || die "usage: $0 <alert-email> [instance-name]   (or set ALERT_EMAIL)"
[[ "$ALERT_EMAIL" == *@*.* ]] \
  || die "'$ALERT_EMAIL' does not look like an email address"

command -v aws >/dev/null 2>&1 \
  || die "aws CLI not found; install it or run this from a machine that has it"

# Resolve the instance. With one instance in the region we can infer it; with
# several, guessing would attach alarms to the wrong box, so require the name.
INSTANCE="${2:-${LIGHTSAIL_INSTANCE:-}}"
if [[ -z "$INSTANCE" ]]; then
  mapfile -t names < <(
    aws lightsail get-instances --region "$AWS_REGION" \
      --query 'instances[].name' --output text 2>/dev/null | tr '\t' '\n'
  )
  case "${#names[@]}" in
    0) die "no Lightsail instances in $AWS_REGION; check AWS_REGION and your credentials" ;;
    1) INSTANCE="${names[0]}"; log "discovered sole instance: $INSTANCE" ;;
    *) die "several instances in $AWS_REGION (${names[*]}); pass the name explicitly" ;;
  esac
fi
readonly INSTANCE

log "region=$AWS_REGION instance=$INSTANCE notify=$ALERT_EMAIL"

# Idempotent: re-registering an already-verified address is a no-op and does
# NOT re-send the confirmation, so this is safe to re-run.
log "registering email contact method (check your inbox on first run)"
aws lightsail create-contact-method \
  --region "$AWS_REGION" \
  --protocol Email \
  --contact-endpoint "$ALERT_EMAIL" >/dev/null 2>&1 \
  || log "contact method already present — continuing"

# put-alarm is upsert-by-name, so re-running updates thresholds in place.
put_alarm() {
  local name="$1"; shift
  log "put-alarm $name"
  aws lightsail put-alarm \
    --region "$AWS_REGION" \
    --alarm-name "$name" \
    --monitored-resource-name "$INSTANCE" \
    --contact-protocols Email \
    "$@" >/dev/null
}

# 1. Burst capacity — the best "heavily loaded" signal on a burstable instance.
#    Reaching 0% throttles every app on this box to baseline CPU at once.
put_alarm payroll-burst-capacity-low \
  --metric-name BurstCapacityPercentage \
  --comparison-operator LessThanThreshold --threshold 25 \
  --evaluation-periods 3 --datapoints-to-alarm 2 \
  --treat-missing-data missing \
  --notification-triggers ALARM OK

# 2. Sustained CPU. Threshold is deliberately LOW: the containers are capped at
#    0.45 + 0.30 vCPU (docker-compose.prod.yml), so payroll alone can only ever
#    reach ~38% of a 2-vCPU box. 60% means payroll is pinned AND a neighbour is
#    busy. A conventional 80% alarm would never fire for a payroll-only wedge.
put_alarm payroll-cpu-sustained \
  --metric-name CPUUtilization \
  --comparison-operator GreaterThanOrEqualToThreshold --threshold 60 \
  --evaluation-periods 3 --datapoints-to-alarm 3 \
  --treat-missing-data missing \
  --notification-triggers ALARM OK

# 3. Instance gone or wedged at the hypervisor level.
put_alarm payroll-status-check-failed \
  --metric-name StatusCheckFailed \
  --comparison-operator GreaterThanOrEqualToThreshold --threshold 1 \
  --evaluation-periods 2 --datapoints-to-alarm 2 \
  --notification-triggers ALARM

# 4. Egress spike — 500 MB in 5 min. Today this is the ONLY signal for bulk
#    data exfiltration: handlers/backup.rs writes no audit row, and uploads are
#    served without authentication. Re-tune after a week of real baseline:
#      aws lightsail get-instance-metric-data --region "$AWS_REGION" \
#        --instance-name "$INSTANCE" --metric-name NetworkOut \
#        --period 300 --unit Bytes --statistics Maximum \
#        --start-time "$(date -u -d '7 days ago' +%Y-%m-%dT%H:%M:%SZ)" \
#        --end-time "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
put_alarm payroll-network-out-spike \
  --metric-name NetworkOut \
  --comparison-operator GreaterThanThreshold --threshold 524288000 \
  --evaluation-periods 1 --datapoints-to-alarm 1 \
  --notification-triggers ALARM

log "done. Current state:"
aws lightsail get-alarms --region "$AWS_REGION" \
  --query 'alarms[?contains(name, `payroll-`)].{name:name,state:state,metric:metricName,notify:notificationEnabled}' \
  --output table

cat <<'NEXT'

Next:
  1. Click the confirmation link emailed to you, or every alarm stays silent.
     Verify with: aws lightsail get-contact-methods --region <region>
  2. These are hypervisor metrics. They CANNOT see memory, disk, container
     restarts, OOM kills or Postgres connections — see the host-side gap-filler
     cron script in the monitoring design for those.
NEXT
