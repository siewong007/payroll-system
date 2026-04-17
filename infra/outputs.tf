output "vpc_id" {
  description = "VPC ID"
  value       = aws_vpc.main.id
}

output "ec2_public_ip" {
  description = "EC2 backend Elastic IP"
  value       = aws_eip.backend.public_ip
}

output "ec2_instance_id" {
  description = "EC2 backend instance ID (for SSM)"
  value       = aws_instance.backend.id
}

output "cloudfront_domain" {
  description = "CloudFront distribution domain name"
  value       = aws_cloudfront_distribution.frontend.domain_name
}

output "cloudfront_distribution_id" {
  description = "CloudFront distribution ID"
  value       = aws_cloudfront_distribution.frontend.id
}

output "ecr_repository_url" {
  description = "ECR repository URL for backend Docker images"
  value       = aws_ecr_repository.backend.repository_url
}

output "rds_endpoint" {
  description = "RDS PostgreSQL endpoint"
  value       = aws_db_instance.postgres.endpoint
  sensitive   = true
}

output "frontend_bucket" {
  description = "S3 bucket for frontend static files"
  value       = aws_s3_bucket.frontend.id
}

output "uploads_bucket" {
  description = "S3 bucket for user uploads"
  value       = aws_s3_bucket.uploads.id
}

output "jwt_secret_arn" {
  description = "Secrets Manager ARN for JWT secret"
  value       = aws_secretsmanager_secret.jwt_secret.arn
}

output "api_url" {
  description = "API URL"
  value       = local.has_domain ? "https://${var.api_subdomain}.${var.domain_name}" : "http://${aws_eip.backend.public_ip}"
}

output "frontend_url" {
  description = "Frontend URL"
  value       = local.has_domain ? "https://${var.domain_name}" : "https://${aws_cloudfront_distribution.frontend.domain_name}"
}
