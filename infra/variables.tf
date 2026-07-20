variable "environment" {
  description = "Deployment environment (dev, staging, prod)"
  type        = string
  default     = "dev"

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
  description = "Root domain name (e.g., payroll.example.com). Leave empty to skip DNS/ACM."
  type        = string
  default     = ""
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
