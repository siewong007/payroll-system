#!/bin/bash
set -euo pipefail
exec > /var/log/user-data.log 2>&1

# --- Install Docker ---
dnf install -y docker
systemctl enable docker
systemctl start docker

# --- Install Caddy ---
dnf install -y 'dnf-command(copr)'
dnf copr enable -y @caddy/caddy
dnf install -y caddy

# --- Install SSM Agent (for remote management) ---
dnf install -y amazon-ssm-agent
systemctl enable amazon-ssm-agent
systemctl start amazon-ssm-agent

# --- Create deploy script ---
cat > /opt/deploy.sh <<'DEPLOY'
#!/bin/bash
set -euo pipefail

REGION="${aws_region}"
ECR_REPO="${ecr_repo_url}"
JWT_SECRET_ARN="${jwt_secret_arn}"
DB_URL_ARN="${db_url_arn}"
FRONTEND_URL="${frontend_url}"
S3_BUCKET="${s3_bucket}"

# Image tag to deploy (commit SHA passed by CI). ECR is immutable; there is no
# `latest` tag, so a deploy must always name a specific tag.
IMAGE_TAG="$${1:-latest}"

# Login to ECR
aws ecr get-login-password --region "$REGION" | docker login --username AWS --password-stdin "$${ECR_REPO%%/*}"

# Pull the requested image
docker pull "$ECR_REPO:$${IMAGE_TAG}"

# Get secrets
JWT_SECRET=$(aws secretsmanager get-secret-value --secret-id "$JWT_SECRET_ARN" --region "$REGION" --query SecretString --output text)
DATABASE_URL=$(aws secretsmanager get-secret-value --secret-id "$DB_URL_ARN" --region "$REGION" --query SecretString --output text)

# Stop existing container
docker stop backend 2>/dev/null || true
docker rm backend 2>/dev/null || true

# Run backend
docker run -d \
  --name backend \
  --restart always \
  -p 8080:8080 \
  -e SERVER_HOST=0.0.0.0 \
  -e SERVER_PORT=8080 \
  -e RUST_LOG=info \
  -e JWT_SECRET="$JWT_SECRET" \
  -e DATABASE_URL="$DATABASE_URL" \
  -e FRONTEND_URL="$FRONTEND_URL" \
  -e S3_BUCKET="$S3_BUCKET" \
  "$ECR_REPO:$${IMAGE_TAG}"

# Prune old images
docker image prune -f
DEPLOY
chmod +x /opt/deploy.sh

# --- Configure Caddy ---
%{ if api_domain != "" }
cat > /etc/caddy/Caddyfile <<CADDYEOF
${api_domain} {
    reverse_proxy localhost:8080
}
CADDYEOF
%{ else }
cat > /etc/caddy/Caddyfile <<CADDYEOF
:80 {
    reverse_proxy localhost:8080
}
CADDYEOF
%{ endif }

systemctl enable caddy
systemctl start caddy

# --- Initial deploy ---
/opt/deploy.sh || echo "Initial deploy skipped (no image yet)"
