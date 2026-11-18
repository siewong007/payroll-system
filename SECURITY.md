# Security Policy

## Supported Versions

This project is an academic and portfolio-oriented prototype. Security fixes are handled on the default branch unless a separate release branch is explicitly maintained.

| Version        | Supported                           |
| -------------- | ----------------------------------- |
| `main`         | Yes                                 |
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
- Use strong, distinct values for `JWT_SECRET`, `TOTP_ENCRYPTION_KEY` (never reuse `JWT_SECRET`), database passwords, OAuth2 credentials, Turnstile keys, and SMTP credentials.
- Treat payroll, banking, NRIC/passport, and employee health/claim data as sensitive personal data under the Malaysian **Personal Data Protection Act 2010 / (Amendment) Act 2024**, even in test scenarios.

## Implemented Security Controls

- **Session & MFA Hardening:** Access tokens stay in memory only; refresh tokens rotate via `httpOnly` cookies and are hashed in PostgreSQL. OAuth2 logins exchange via cookie-backed `/auth/refresh` rather than URL fragments. Forced initial password rotations (`must_change_password`) are enforced server-side on API extraction. Password changes revoke all active `user_sessions` and `refresh_tokens` in-transaction. TOTP 2FA secrets are encrypted at rest under a dedicated `TOTP_ENCRYPTION_KEY` with atomic step anti-replay (`last_used_step`), brute-force lockout, and passkey challenge user-binding (`1030`).
- **RBAC, Delegation & Maker-Checker Separation:** Platform-only privileges (`manage_users`, `manage_companies`, `manage_backups`, `manage_platform_settings`) are blocked from company-scoped `user_groups`; non-super-admins can delegate only permissions they hold; conflicting `submit_payroll` + `approve_payroll` capabilities cannot coexist in a group or user's effective grants; payroll approvals and leave/claim/overtime approvals enforce maker-checker separation.
- **Bot & Rate Protection:** Cloudflare Turnstile guards unauthenticated credential-free endpoints in production (fail-closed: `400` missing, `403` rejected, `502` upstream unreachable), paired with verified-JWT session-keyed and IP-keyed `tower_governor` limiters (with periodic limiter state pruning) and explicit `TRUST_PROXY_HEADERS` gating.
- **Authorized File Storage, Import Bounds & Path Containment:** Uploaded attachments and receipts are validated by extension (SVG excluded), 10 MB size cap, magic-byte inspection, and platform-independent single-component path containment (`core::upload_path`), tracked per tenant in `uploaded_files`, and authorized per-user via reverse-reference ownership (`upload_service::read_authorized`, returning uniform `404` on denial with `Content-Disposition: attachment` for non-image files). Excel imports inspect ZIP archives with decompressed size caps before parsing to prevent zip-bomb DoS.
- **SSRF Protection:** Calendar ICS URL imports pass through `core::http_client`, blocking loopback, private, link-local (`169.254.0.0/16`, `fe80::/10`), ULA (`fc00::/7`), CGNAT, multicast, documentation/protocol-assignment/6to4/NAT64 IPv6 blocks, and IPv4-mapped/compatible IPv6 addresses, disabling redirects, capping responses at 2 MB and 500 events, and collapsing DNS/transport errors so the endpoint cannot be used as an internal port scanner.
- **PDPA Minimization & Retention:** Initial passwords are randomly generated (never derived from NRIC or defaults) and redacted from `email_logs`; attendance network learning never exposes `employee_id → IP` mappings; automated daily jobs enforce statutory 7-year (`2,555`-day) audit retention alongside shorter purges for sessions, tokens, reset/challenge states, finished jobs, email logs, and network observations. See [docs/legal/privacy-and-pdpa-notice.md](docs/legal/privacy-and-pdpa-notice.md) for the **72-hour mandatory Data Breach Notification (Section 12B PDPA)** procedure.

## Known Prototype & Architectural Considerations

The repository requires independent organizational review before deployment with live employee data:

- **Application-Enforced Tenancy:** Tenant isolation is enforced by capability permissions, composite `(parent_id, company_id)` foreign keys, immutable `company_id` triggers, and company-scoped SQL predicates (verified by `route_inventory_tests` and `route_auth_tests`); PostgreSQL row-level security (`RLS`) is not enabled.
- **Kiosk Display URLs:** The public kiosk display route (`/kiosk#<secret>`, with legacy `/kiosk/:kioskKey` support) passes its revocable credential in the URL fragment, loads it into memory, and scrubs it immediately via `history.replaceState` so wall-mounted displays survive browser setup without leaking secrets in HTTP `Referer` headers or server logs; kiosk devices should still be physically secured and rotated if decommissioned.
- **In-Process Background Workers:** Scheduled retention and auto-absent tasks coordinate across API replicas via PostgreSQL advisory locks (`pg_try_advisory_lock`), and `background_jobs` claim rows atomically (`status = 'pending'`), though execution still occurs inside the API worker process rather than a separate worker tier.
- **Statutory Fail-Closed Gate:** Statutory rows shipped in `1001_data.sql` are unverified academic fixtures. Production calculations fail closed unless source-linked EPF/SOCSO/EIS schedules are independently verified (`status = 'verified'`), and automatic PCB remains disabled until the calculator passes LHDN computerised-MTD conformance testing.

Fresh databases contain no demo organizations or known credentials. Use the one-time `bootstrap_admin` command and unset its password environment variable immediately after use.
