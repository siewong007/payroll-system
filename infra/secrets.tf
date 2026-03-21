resource "random_password" "jwt_secret" {
  length  = 64
  special = true
}

resource "aws_secretsmanager_secret" "jwt_secret" {
  name        = "${local.name_prefix}/jwt-secret"
  description = "JWT signing secret for the payroll system"

  tags = { Name = "${local.name_prefix}-jwt-secret" }
}

resource "aws_secretsmanager_secret_version" "jwt_secret" {
  secret_id     = aws_secretsmanager_secret.jwt_secret.id
  secret_string = random_password.jwt_secret.result
}

resource "aws_secretsmanager_secret" "database_url" {
  name        = "${local.name_prefix}/database-url"
  description = "PostgreSQL connection string for the payroll system"

  tags = { Name = "${local.name_prefix}-database-url" }
}

resource "aws_secretsmanager_secret_version" "database_url" {
  secret_id = aws_secretsmanager_secret.database_url.id
  secret_string = format(
    "postgres://%s:%s@%s/%s",
    aws_db_instance.postgres.username,
    random_password.db_password.result,
    aws_db_instance.postgres.endpoint,
    aws_db_instance.postgres.db_name,
  )
}

resource "aws_secretsmanager_secret" "google_oauth" {
  count       = var.google_client_id != "" ? 1 : 0
  name        = "${local.name_prefix}/google-oauth"
  description = "Google OAuth2 credentials"

  tags = { Name = "${local.name_prefix}-google-oauth" }
}

resource "aws_secretsmanager_secret_version" "google_oauth" {
  count     = var.google_client_id != "" ? 1 : 0
  secret_id = aws_secretsmanager_secret.google_oauth[0].id
  secret_string = jsonencode({
    client_id     = var.google_client_id
    client_secret = var.google_client_secret
  })
}
