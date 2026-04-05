#!/bin/bash
set -euo pipefail
DB_URL=$(aws secretsmanager get-secret-value --secret-id arn:aws:secretsmanager:ap-southeast-1:371726673750:secret:payroll-dev/database-url-JjdqVe --region ap-southeast-1 --query SecretString --output text)

echo "=== Before ==="
psql "$DB_URL" -c "SELECT version, description FROM _sqlx_migrations WHERE version >= 29 ORDER BY version"

echo "=== Removing migration 30 (oauth2_states) ==="
psql "$DB_URL" -c "DELETE FROM _sqlx_migrations WHERE version = 30"

echo "=== Dropping oauth2_states table ==="
psql "$DB_URL" -c "DROP TABLE IF EXISTS oauth2_states CASCADE"
psql "$DB_URL" -c "DROP TABLE IF EXISTS passkey_credentials CASCADE"
psql "$DB_URL" -c "DROP TABLE IF EXISTS passkey_challenges CASCADE"

echo "=== After ==="
psql "$DB_URL" -c "SELECT version, description FROM _sqlx_migrations WHERE version >= 29 ORDER BY version"

echo "=== Restarting backend ==="
docker stop backend 2>/dev/null || true
docker rm backend 2>/dev/null || true
bash /opt/deploy.sh

sleep 5
docker logs backend --tail 5 2>&1
curl -sf http://localhost:8080/api/health && echo " - HEALTHY" || echo " - NOT HEALTHY"
