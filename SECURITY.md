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

## Known Prototype Limitations

The repository is not approved as production payroll software. The following
known limitations require design work or independent review before deployment
with real employee data:

- Tenant authorization is enforced by application permissions and
  company-scoped queries; PostgreSQL row-level security is not enabled.
- The attendance option labelled Face ID does not verify a fresh biometric or
  WebAuthn assertion at the check-in endpoint.
- Employee onboarding still uses a predictable initial-password fallback and
  permits the forced-password-change state to be skipped.
- Uploaded files are stored on local API disk and served through unguessable
  capability URLs without per-user download authorization. The provisioned S3
  bucket is not integrated.
- Backup file restoration and document cleanup require stronger canonical-path
  containment checks before accepting untrusted backup/document metadata.
- Calendar import from a remote ICS URL needs outbound destination, redirect,
  response-size, and private-network controls to prevent SSRF.
- A normal password change does not yet revoke all existing refresh sessions.
- Scheduled cleanup and absence jobs execute inside the API process; multiple
  replicas require a distributed lease or dedicated worker.
- Statutory rows shipped in `1001_data.sql` are unverified academic fixtures.
  Production calculations fail closed unless source-linked EPF/SOCSO/EIS rules
  are independently verified, and automatic PCB remains disabled until the
  calculator passes LHDN computerised-MTD conformance testing.

Fresh databases no longer contain demo organizations or known credentials. Use
the one-time `bootstrap_admin` command and remove its password environment
variable immediately after use.
