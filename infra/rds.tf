resource "random_password" "db_password" {
  length  = 32
  special = false
}

resource "aws_db_subnet_group" "postgres" {
  name       = "${local.name_prefix}-db-subnet"
  subnet_ids = aws_subnet.private[*].id

  tags = { Name = "${local.name_prefix}-db-subnet" }
}

# Intentional exception: local, CI, and Lightsail target PostgreSQL 19 Beta 2,
# but standard Amazon RDS does not yet offer a production PostgreSQL 19 engine.
# Keep this at 18.4/postgres18 until AWS publishes a standard (non-Preview)
# PostgreSQL 19 release, then upgrade engine and parameter-group family together.
resource "aws_db_parameter_group" "postgres18" {
  name   = "${local.name_prefix}-pg18"
  family = "postgres18"

  parameter {
    name  = "log_statement"
    value = "ddl"
  }

  parameter {
    name  = "log_min_duration_statement"
    value = "1000"
  }

  # PostgreSQL 18 ships asynchronous I/O. "worker" is the portable default;
  # switch to "io_uring" only on kernels that support it. io_method is a
  # postmaster-level GUC, so it only takes effect after a reboot.
  parameter {
    name         = "io_method"
    value        = "worker"
    apply_method = "pending-reboot"
  }

  tags = { Name = "${local.name_prefix}-pg18-params" }
}

resource "aws_db_instance" "postgres" {
  identifier = "${local.name_prefix}-db"

  # See the provider-availability exception above and docs/database.md.
  engine         = "postgres"
  engine_version = "18.4"
  instance_class = var.db_instance_class

  # Required for the 16 -> 18 major-version upgrade to apply in place.
  allow_major_version_upgrade = true

  allocated_storage     = var.db_allocated_storage
  max_allocated_storage = var.db_allocated_storage * 2
  storage_encrypted     = true

  db_name  = "payroll_db"
  username = "payroll"
  password = random_password.db_password.result

  multi_az               = var.enable_multi_az
  db_subnet_group_name   = aws_db_subnet_group.postgres.name
  vpc_security_group_ids = [aws_security_group.rds.id]
  parameter_group_name   = aws_db_parameter_group.postgres18.name

  backup_retention_period = 7
  backup_window           = "03:00-04:00"
  maintenance_window      = "sun:04:00-sun:05:00"

  deletion_protection = var.environment == "prod"
  skip_final_snapshot = var.environment != "prod"

  final_snapshot_identifier = var.environment == "prod" ? "${local.name_prefix}-final-snapshot" : null

  tags = { Name = "${local.name_prefix}-db" }
}
