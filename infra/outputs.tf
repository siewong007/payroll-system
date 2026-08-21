output "cloudfront_domain" {
  description = "CloudFront distribution domain name"
  value       = aws_cloudfront_distribution.frontend.domain_name
}

output "cloudfront_distribution_id" {
  description = "CloudFront distribution ID"
  value       = aws_cloudfront_distribution.frontend.id
}

output "frontend_bucket" {
  description = "S3 bucket for frontend static files"
  value       = aws_s3_bucket.frontend.id
}

output "api_url" {
  description = "API URL (served from the Lightsail VPS, not AWS)"
  value       = local.has_domain ? "https://${var.api_subdomain}.${var.domain_name}" : "http://${var.backend_vps_ip}"
}

output "frontend_url" {
  description = "Frontend URL"
  value       = local.has_domain ? "https://${var.domain_name}" : "https://${aws_cloudfront_distribution.frontend.domain_name}"
}

output "payroll_backups_bucket" {
  description = "Off-host encrypted backup bucket (see deploy/payroll-backup.sh header)"
  value       = aws_s3_bucket.payroll_backups.id
}

output "backup_iam_user" {
  description = "IAM user whose access keys go into /opt/payroll/secrets.env as BACKUP_AWS_*"
  value       = aws_iam_user.backup_agent.arn
}
