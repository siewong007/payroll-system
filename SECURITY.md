# Security Policy

## Supported Versions

This project is an academic and portfolio-oriented prototype. Security fixes are handled on the default branch unless a separate release branch is explicitly maintained.

| Version | Supported |
| --- | --- |
| `main` | Yes |
| Other branches | No, unless stated by the maintainer |

## Reporting a Vulnerability

Please do not open a public GitHub issue for a suspected security vulnerability.

Report security concerns privately to the repository maintainer with:

- A clear description of the issue.
- Steps to reproduce, if available.
- Affected endpoint, file, or workflow.
- Potential impact.
- Suggested mitigation, if known.

The maintainer will review the report and may request more information before preparing a fix.

## Security Scope

Examples of issues worth reporting:

- Authentication or authorization bypass.
- Exposure of payroll, employee, or company data.
- Insecure token, cookie, or session behavior.
- SQL injection, command injection, or path traversal.
- Unsafe file upload or download behavior.
- Secrets committed to the repository.

## Out of Scope

- Reports based only on missing production hardening for a local academic prototype.
- Vulnerabilities requiring access to a developer's local machine.
- Denial-of-service findings without a practical exploit path.
- Dependency warnings without a reachable security impact.

## Local Development Secrets

- Do not commit real `.env` files.
- Rotate any secret that is accidentally committed.
- Use strong values for `JWT_SECRET`, database passwords, OAuth2 credentials, and SMTP credentials.
- Treat payroll and employee data as sensitive, even in test scenarios.
