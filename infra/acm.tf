# --- ACM Certificate for Frontend (CloudFront — must be us-east-1) ---

resource "aws_acm_certificate" "frontend" {
  count             = local.has_domain ? 1 : 0
  provider          = aws.us_east_1
  domain_name       = var.domain_name
  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }

  tags = { Name = "${local.name_prefix}-frontend-cert" }
}

resource "aws_acm_certificate_validation" "frontend" {
  count                   = local.has_domain ? 1 : 0
  provider                = aws.us_east_1
  certificate_arn         = aws_acm_certificate.frontend[0].arn
  validation_record_fqdns = [for record in aws_route53_record.frontend_cert_validation : record.fqdn]
}
