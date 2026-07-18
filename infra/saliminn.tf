# --- saliminn.my DNS (Route53) ---
#
# The saliminn hotel app itself is built and deployed entirely from the
# github.com/siewong007/hotel-app repository (SSH release straight to the
# Lightsail VPS — no ECR/SSM/Secrets Manager). The ONLY AWS-side concern is
# DNS: this file owns a public hosted zone for saliminn.my and points it at
# the Lightsail host that already serves payroll.
#
# This config is intentionally decoupled from the rest of infra/ (no EC2/EIP,
# IAM, or account-id references) so the zone can be created on its own:
#   terraform apply \
#     -target=aws_route53_zone.saliminn \
#     -target=aws_route53_record.saliminn_apex \
#     -target=aws_route53_record.saliminn_www
#
# After the first apply, run `terraform output saliminn_nameservers` and enter
# those four values as custom nameservers for saliminn.my at Spaceship. Once
# DNS propagates, the host Caddy on the VPS obtains a Let's Encrypt cert
# automatically (the hotel-app deploy adds the saliminn.my site block).

variable "saliminn_domain_name" {
  description = "Root domain for the saliminn hotel app"
  type        = string
  default     = "saliminn.my"
}

variable "saliminn_vps_ip" {
  description = "Public IPv4 of the Lightsail VPS that serves saliminn.my (and payroll)"
  type        = string
  default     = "13.251.162.88"
}

# The zone is created here (not a data source): the domain is registered at
# Spaceship, whose nameservers are switched to Route53 after the first apply.
resource "aws_route53_zone" "saliminn" {
  name = var.saliminn_domain_name

  tags = { Name = "saliminn-zone" }
}

resource "aws_route53_record" "saliminn_apex" {
  zone_id = aws_route53_zone.saliminn.zone_id
  name    = var.saliminn_domain_name
  type    = "A"
  ttl     = 300
  records = [var.saliminn_vps_ip]
}

resource "aws_route53_record" "saliminn_www" {
  zone_id = aws_route53_zone.saliminn.zone_id
  name    = "www.${var.saliminn_domain_name}"
  type    = "A"
  ttl     = 300
  records = [var.saliminn_vps_ip]
}

output "saliminn_nameservers" {
  description = "Route53 nameservers for saliminn.my — enter these four as custom nameservers at the registrar (Spaceship)"
  value       = aws_route53_zone.saliminn.name_servers
}
