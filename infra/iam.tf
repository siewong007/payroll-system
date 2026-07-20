# --- CI/CD Deployment Role ---

resource "aws_iam_openid_connect_provider" "github" {
  url             = "https://token.actions.githubusercontent.com"
  client_id_list  = ["sts.amazonaws.com"]
  thumbprint_list = ["6938fd4d98bab03faadb97b34396831e3780aea1", "1c58a3a8518e8759bf075b76b750d4f2df264fcd"]
}

resource "aws_iam_role" "cicd" {
  name = "${local.name_prefix}-cicd"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRoleWithWebIdentity"
        Effect = "Allow"
        Principal = {
          Federated = aws_iam_openid_connect_provider.github.arn
        }
        Condition = {
          # Audience must be the AWS STS audience configured on the OIDC provider.
          StringEquals = {
            "token.actions.githubusercontent.com:aud" = "sts.amazonaws.com"
          }
          # Pin to THIS repository's main branch. Previously `repo:*` allowed any
          # GitHub repository on github.com to assume this deploy role and thereby
          # write to the frontend S3 bucket and invalidate CloudFront.
          StringLike = {
            "token.actions.githubusercontent.com:sub" = "repo:${var.github_repository}:ref:refs/heads/main"
          }
        }
      }
    ]
  })

  tags = { Name = "${local.name_prefix}-cicd" }
}

resource "aws_iam_role_policy" "cicd_deploy" {
  name = "${local.name_prefix}-deploy"
  role = aws_iam_role.cicd.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid    = "S3Frontend"
        Effect = "Allow"
        Action = [
          "s3:PutObject",
          "s3:DeleteObject",
          "s3:ListBucket",
          "s3:GetObject",
        ]
        Resource = [
          aws_s3_bucket.frontend.arn,
          "${aws_s3_bucket.frontend.arn}/*",
        ]
      },
      {
        Sid    = "CloudFront"
        Effect = "Allow"
        Action = [
          "cloudfront:CreateInvalidation",
        ]
        Resource = aws_cloudfront_distribution.frontend.arn
      },
    ]
  })
}
