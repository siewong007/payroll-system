# --- Off-host backups (plan item 1) -----------------------------------------
#
# A versioned, lifecycle-managed, encrypted bucket that receives the nightly
# `age`-encrypted pg_dump from the Lightsail host (deploy/payroll-backup.sh).
# The application data it protects lives on a single Lightsail disk together
# with every local copy of itself; this bucket is the only copy that survives
# losing that host.
#
# Client-side encryption is the point: the dump is unreadable on AWS's side,
# and the age PRIVATE key never touches this host (only the public recipient
# does). Losing the private key loses the backups — keep it wherever secrets
# are escrowed, which is exactly what plan item 3 builds.

resource "aws_s3_bucket" "payroll_backups" {
  # Account-id suffix keeps the global name unique without a new variable.
  bucket = "${local.name_prefix}-backups-371726673750"
}

resource "aws_s3_bucket_public_access_block" "payroll_backups" {
  bucket = aws_s3_bucket.payroll_backups.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_versioning" "payroll_backups" {
  bucket = aws_s3_bucket.payroll_backups.id

  versioning_configuration {
    status = "Enabled"
  }
}

# Retention policy: a month of daily restores is far more than payroll
# recovery realistically needs, and unbounded growth on an SME budget is its
# own failure mode. Current versions expire after 90 days; superseded
# (noncurrent) versions — including anything deleted or overwritten — die at
# 30. Incomplete multipart uploads are aborted so failed pushes cannot bill
# silently forever.
resource "aws_s3_bucket_lifecycle_configuration" "payroll_backups" {
  bucket = aws_s3_bucket.payroll_backups.id

  rule {
    id     = "db-nightly"
    status = "Enabled"

    filter {
      prefix = "db/"
    }

    expiration {
      days = 90
    }

    noncurrent_version_expiration {
      noncurrent_days = 30
    }

    abort_incomplete_multipart_upload {
      days_after_initiation = 7
    }
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "payroll_backups" {
  bucket = aws_s3_bucket.payroll_backups.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

# TLS-only: the objects are payroll history and are already client-side
# encrypted, but there is still no reason to accept plaintext fetches.
resource "aws_s3_bucket_policy" "payroll_backups_tls" {
  bucket = aws_s3_bucket.payroll_backups.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid       = "DenyInsecureTransport"
        Effect    = "Deny"
        Principal = "*"
        Action    = "s3:*"
        Resource = [
          aws_s3_bucket.payroll_backups.arn,
          "${aws_s3_bucket.payroll_backups.arn}/*",
        ]
        Condition = {
          Bool = { "aws:SecureTransport" = "false" }
        }
      },
    ]
  })
}

# Dedicated least-privilege identity for the host's backup script. Its access
# keys are created OUTSIDE Terraform (`aws iam create-access-key --user-name …`)
# and stored in /opt/payroll/secrets.env, so no secret material ever lands in
# the Terraform state.
resource "aws_iam_user" "backup_agent" {
  name = "${local.name_prefix}-backup-agent"
}

data "aws_iam_policy_document" "backup_agent" {
  statement {
    sid    = "ListBucket"
    effect = "Allow"
    actions = [
      "s3:ListBucket",
      "s3:GetBucketLocation",
    ]
    resources = [aws_s3_bucket.payroll_backups.arn]
  }

  statement {
    sid    = "ReadWriteObjects"
    effect = "Allow"
    actions = [
      "s3:GetObject",
      "s3:PutObject",
    ]
    resources = ["${aws_s3_bucket.payroll_backups.arn}/*"]
  }
}

resource "aws_iam_user_policy" "backup_agent" {
  name   = "${local.name_prefix}-backup-agent-s3"
  user   = aws_iam_user.backup_agent.name
  policy = data.aws_iam_policy_document.backup_agent.json
}
