# --- Route53 Hosted Zone (data source — assumes zone already exists) ---

data "aws_route53_zone" "main" {
  count = local.has_domain ? 1 : 0
  name  = var.domain_name
}

# --- DNS Validation Records for ACM (frontend cert only) ---

resource "aws_route53_record" "frontend_cert_validation" {
  for_each = local.has_domain ? {
    for dvo in aws_acm_certificate.frontend[0].domain_validation_options : dvo.domain_name => {
      name   = dvo.resource_record_name
      record = dvo.resource_record_value
      type   = dvo.resource_record_type
    }
  } : {}

  allow_overwrite = true
  name            = each.value.name
  records         = [each.value.record]
  ttl             = 60
  type            = each.value.type
  zone_id         = data.aws_route53_zone.main[0].zone_id
}

# --- A Records ---

resource "aws_route53_record" "frontend" {
  count   = local.has_domain ? 1 : 0
  zone_id = data.aws_route53_zone.main[0].zone_id
  name    = var.domain_name
  type    = "A"

  alias {
    name                   = aws_cloudfront_distribution.frontend.domain_name
    zone_id                = aws_cloudfront_distribution.frontend.hosted_zone_id
    evaluate_target_health = false
  }
}

resource "aws_route53_record" "api" {
  count   = local.has_domain ? 1 : 0
  zone_id = data.aws_route53_zone.main[0].zone_id
  name    = "${var.api_subdomain}.${var.domain_name}"
  type    = "A"
  ttl     = 300
  records = [aws_eip.backend.public_ip]
}
