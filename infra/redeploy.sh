#!/bin/bash
set -euo pipefail

# Update deploy script with WebAuthn env vars
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
  -e "WEBAUTHN_RP_ID=payrollmy.com" \
  -e "WEBAUTHN_RP_ORIGIN=https://payrollmy.com" \
  "$ECR_REPO:latest"

docker image prune -f
DEPLOY
chmod +x /opt/deploy.sh

# Run it
bash /opt/deploy.sh

# Verify
sleep 3
docker ps
curl -sf http://localhost:8080/api/health && echo " - API healthy" || echo " - API NOT healthy"
