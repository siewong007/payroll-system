#!/bin/bash
set -euo pipefail

echo "=== Installing Caddy ==="
curl -fsSL "https://caddyserver.com/api/download?os=linux&arch=arm64" -o /usr/local/bin/caddy
chmod +x /usr/local/bin/caddy
/usr/local/bin/caddy version

echo "=== Creating Caddyfile ==="
mkdir -p /etc/caddy
cat > /etc/caddy/Caddyfile <<'EOF'
api.payrollmy.com {
    reverse_proxy localhost:8080
}
EOF

echo "=== Creating deploy script ==="
cat > /opt/deploy.sh <<'DEPLOY'
#!/bin/bash
set -euo pipefail
REGION=ap-southeast-1
ECR_REPO=371726673750.dkr.ecr.ap-southeast-1.amazonaws.com/payroll-dev-backend
JWT_SECRET_ARN=arn:aws:secretsmanager:ap-southeast-1:371726673750:secret:payroll-dev/jwt-secret-SqcVxB
DB_URL_ARN=arn:aws:secretsmanager:ap-southeast-1:371726673750:secret:payroll-dev/database-url-JjdqVe
FRONTEND_URL=https://payrollmy.com
S3_BUCKET=payroll-dev-uploads

aws ecr get-login-password --region "$REGION" | docker login --username AWS --password-stdin "${ECR_REPO%%/*}"
docker pull "$ECR_REPO:latest"

JWT_SECRET=$(aws secretsmanager get-secret-value --secret-id "$JWT_SECRET_ARN" --region "$REGION" --query SecretString --output text)
DATABASE_URL=$(aws secretsmanager get-secret-value --secret-id "$DB_URL_ARN" --region "$REGION" --query SecretString --output text)

docker stop backend 2>/dev/null || true
docker rm backend 2>/dev/null || true

docker run -d \
  --name backend \
  --restart always \
  -p 8080:8080 \
  -e SERVER_HOST=0.0.0.0 \
  -e SERVER_PORT=8080 \
  -e "RUST_LOG=info,payroll_system=debug" \
  -e "JWT_SECRET=$JWT_SECRET" \
  -e "DATABASE_URL=$DATABASE_URL" \
  -e "FRONTEND_URL=$FRONTEND_URL" \
  -e "S3_BUCKET=$S3_BUCKET" \
  "$ECR_REPO:latest"

docker image prune -f
DEPLOY
chmod +x /opt/deploy.sh

echo "=== Running deploy ==="
bash /opt/deploy.sh

echo "=== Starting Caddy ==="
# Create systemd service for caddy
cat > /etc/systemd/system/caddy.service <<'SVC'
[Unit]
Description=Caddy
After=network.target

[Service]
ExecStart=/usr/local/bin/caddy run --config /etc/caddy/Caddyfile
Restart=always

[Install]
WantedBy=multi-user.target
SVC

systemctl daemon-reload
systemctl enable caddy
systemctl start caddy

echo "=== Checking ==="
docker ps
systemctl status caddy --no-pager || true
echo "=== Done ==="
