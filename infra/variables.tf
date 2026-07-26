# There are no workspaces and exactly one state key (see main.tf's backend
# block), so these two variables are not preferences — they select which live
# resources this configuration claims to describe. Both are deliberately
# defaultless: an operator who omits one gets an error, where before they got a
# plan. Copy terraform.tfvars.example to terraform.tfvars.

variable "environment" {
  description = "Deployment environment (dev, staging, prod). REQUIRED: it is half of local.name_prefix, so a wrong value renames every prefixed resource in the one shared state."
  type        = string

  validation {
    condition     = contains(["dev", "staging", "prod"], var.environment)
    error_message = "Environment must be dev, staging, or prod."
  }
}

variable "aws_region" {
  description = "AWS region for resources"
  type        = string
  default     = "ap-southeast-1"
}

variable "project_name" {
  description = "Project name used for resource naming"
  type        = string
  default     = "payroll"
}

variable "github_repository" {
  description = "GitHub repository (owner/name) allowed to assume the CI/CD deploy role via OIDC"
  type        = string
  default     = "siewong007/payroll-system"
}

variable "domain_name" {
  description = "Root domain name (e.g. payrollmy.com). REQUIRED: local.has_domain gates the ACM certificate and both Route53 A records, so an empty value does not 'skip DNS/ACM' against the live state — it plans their DESTRUCTION, including api.payrollmy.com, which is how the backend VPS is reached by name."
  type        = string

  validation {
    condition     = length(trimspace(var.domain_name)) > 0
    error_message = "domain_name must be set — copy terraform.tfvars.example to terraform.tfvars."
  }
}

variable "api_subdomain" {
  description = "Subdomain for the API (e.g., api)"
  type        = string
  default     = "api"
}

variable "backend_vps_ip" {
  description = "Public IPv4 of the Lightsail VPS that serves the payroll backend (and ekowayhardware.com, saliminn.my)"
  type        = string
  default     = "13.251.162.88"
}
