#!/bin/bash
set -euo pipefail

# Update deploy script with Gmail App Password
sed -i 's/SMTP_PASSWORD=Chou12345?/SMTP_PASSWORD=qnws hrhb acci fnyk/' /opt/deploy.sh

# Redeploy with new config
bash /opt/deploy.sh

sleep 5
curl -sf http://localhost:8080/api/health && echo " - HEALTHY" || echo " - NOT HEALTHY"
