# --- ekowayhardware.com DNS (Route53) ---
#
# online-shopping-platform itself is built and deployed entirely from the
# github.com/siewong007/online-shopping-platform repository (SSH release
# straight to the Lightsail VPS — no ECR/SSM/Secrets Manager). The ONLY
# AWS-side concern is DNS: this file owns a public hosted zone for
# ekowayhardware.com and points it at the same Lightsail host that already
# serves payroll and saliminn.my.
#
# This config is intentionally decoupled from the rest of infra/ (no EC2/EIP,
# IAM, or account-id references) so the zone can be created on its own:
#   terraform apply \
#     -target=aws_route53_zone.ekowayhardware \
#     -target=aws_route53_record.ekowayhardware_apex \
#     -target=aws_route53_record.ekowayhardware_www
#
# After the first apply, run `terraform output ekowayhardware_nameservers`
# and enter those four values as custom nameservers for ekowayhardware.com at
# its registrar (currently dns7/dns7a/dns8/dns8a.serverfreak.biz — replace
# all four). Once DNS propagates, the host Caddy on the VPS obtains a Let's
# Encrypt cert automatically (the online-shopping-platform deploy adds the
# ekowayhardware.com site block).

variable "ekowayhardware_domain_name" {
  description = "Root domain for the online shopping platform"
  type        = string
  default     = "ekowayhardware.com"
}

variable "ekowayhardware_vps_ip" {
  description = "Public IPv4 of the Lightsail VPS that serves ekowayhardware.com (and payroll, saliminn.my)"
  type        = string
  default     = "13.251.162.88"
}

# The zone is created here (not a data source): the domain is currently
# registered with nameservers at serverfreak.biz, switched to Route53 after
# the first apply.
resource "aws_route53_zone" "ekowayhardware" {
  name = var.ekowayhardware_domain_name

  tags = { Name = "ekowayhardware-zone" }
}

resource "aws_route53_record" "ekowayhardware_apex" {
  zone_id = aws_route53_zone.ekowayhardware.zone_id
  name    = var.ekowayhardware_domain_name
  type    = "A"
  ttl     = 300
  records = [var.ekowayhardware_vps_ip]
}

resource "aws_route53_record" "ekowayhardware_www" {
  zone_id = aws_route53_zone.ekowayhardware.zone_id
  name    = "www.${var.ekowayhardware_domain_name}"
  type    = "A"
  ttl     = 300
  records = [var.ekowayhardware_vps_ip]
}

output "ekowayhardware_nameservers" {
  description = "Route53 nameservers for ekowayhardware.com — enter these four as custom nameservers at the registrar"
  value       = aws_route53_zone.ekowayhardware.name_servers
}
