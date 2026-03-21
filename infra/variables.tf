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

variable "db_instance_class" {
  description = "RDS instance class"
  type        = string
  default     = "db.t4g.micro"
}

variable "db_allocated_storage" {
  description = "RDS allocated storage in GB"
  type        = number
  default     = 20
}

variable "enable_multi_az" {
  description = "Enable Multi-AZ for RDS"
  type        = bool
  default     = false
}

variable "ec2_instance_type" {
  description = "EC2 instance type for backend server"
  type        = string
  default     = "t4g.micro"
}

variable "google_client_id" {
  description = "Google OAuth2 client ID (optional)"
  type        = string
  default     = ""
  sensitive   = true
}

variable "google_client_secret" {
  description = "Google OAuth2 client secret (optional)"
  type        = string
  default     = ""
  sensitive   = true
}
