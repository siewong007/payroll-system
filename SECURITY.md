# Security Policy

## Supported Versions

This project is an academic and portfolio-oriented prototype. Security fixes are handled on the active integration branches (`main` / `master`) unless a separate release branch is explicitly maintained.

| Version           | Supported                           |
| ----------------- | ----------------------------------- |
| `main` / `master` | Yes                                 |
| Other branches    | No, unless stated by the maintainer |

## Reporting a Vulnerability

Please do not open a public GitHub issue for a suspected security vulnerability.

Report security concerns privately to the repository maintainer with:

- A clear description of the issue.
- Steps to reproduce, if available.
- Affected endpoint, file, or workflow.
- Potential impact and suggested mitigation, if known.

## Security Scope

Examples of issues worth reporting:

- Authentication or authorization bypass.
- Exposure of payroll, employee, or company data.
- Insecure token, cookie, or session behavior.
- SQL injection, command injection, or path traversal.
- Unsafe file upload, download, or Web Push endpoint behavior.
- Secrets committed to the repository.

## Out of Scope

- Reports based only on missing production hardening for a local academic prototype.
- Vulnerabilities requiring access to a developer's local machine.
- Denial-of-service findings without a practical exploit path.
- Dependency warnings without a reachable security impact.

## Local Development Secrets

- Do not commit real `.env` files and rotate any secret that is accidentally committed.
- Use strong, distinct values for `JWT_SECRET`, `TOTP_ENCRYPTION_KEY` (never reuse `JWT_SECRET`), `VAPID_PRIVATE_KEY`, database passwords, OAuth2 credentials, Turnstile keys, and SMTP credentials.
- `AppConfig::validate_production_secrets` fails closed on short (<32 chars) or placeholder (e.g. `.env.example`) secrets when `RUST_ENV=production` or `FRONTEND_URL` uses HTTPS.
- Treat payroll, banking, NRIC/passport, and employee health/claim data as sensitive personal data under the Malaysian **Personal Data Protection Act 2010 / (Amendment) Act 2024**, even in test scenarios.

## Implemented Security Controls

- **Session & MFA Hardening:** Access tokens stay in memory only; refresh tokens rotate atomically with reuse detection via `httpOnly` cookies and SHA-256 hashes in PostgreSQL (`1033` `revoked_at` tracking). OAuth2 logins exchange via cookie-backed `/auth/refresh` and bind `/authorize` state with an `httpOnly` cookie. Forced initial password rotations (`must_change_password`) are enforced server-side on API extraction. Password changes revoke all active `user_sessions` and `refresh_tokens` in-transaction. TOTP 2FA secrets are encrypted under `TOTP_ENCRYPTION_KEY` with atomic step anti-replay (`last_used_step`), brute-force lockout, O(1) keyed SHA-256 backup-code lookup (`1032`), `spawn_blocking` bcrypt verification, and passkey challenge user-binding (`1030`).
- **RBAC, Delegation & Maker-Checker Separation:** Platform-only privileges (`manage_users`, `manage_companies`, `manage_backups`, `manage_platform_settings`) are blocked from company-scoped `user_groups`; non-super-admins can delegate only permissions they hold; conflicting `submit_payroll` + `approve_payroll` capabilities cannot coexist in a group or user's effective grants; `manager` accounts cannot view `admin`/`super_admin` directory entries (`user_service::hides_privileged_users`); payroll and leave/claim/overtime approvals enforce maker-checker separation.
- **Bot, Rate & Payload Controls:** Cloudflare Turnstile guards unauthenticated credential-free endpoints when `TURNSTILE_SECRET_KEY` is set (fail-closed: `400` missing, `403` rejected, `502` upstream unreachable), paired with IP-keyed (`ClientIpKeyExtractor` gated by `TRUST_PROXY_HEADERS`) and session-keyed (`SessionOrIpKeyExtractor`) `tower_governor` limiters with periodic pruning, plus explicit `DefaultBodyLimit` bounds on upload and import endpoints.
- **Authorized File Storage, Import Bounds & Path Containment:** Uploaded attachments and receipts are validated by extension (SVG excluded), 10 MB size cap, magic-byte inspection, and single-component path containment (`core::upload_path`), tracked per tenant in `uploaded_files`, and authorized per-user via reverse-reference ownership (`upload_service::read_authorized`, returning uniform `404` on denial with `Content-Disposition: attachment` for non-images). Excel imports inspect ZIP archives with decompressed size caps before parsing.
- **SSRF & Web Push Protection:** Calendar ICS URL imports (`core::http_client`) and Web Push endpoint deliveries (`push_service`) block loopback, private, link-local, ULA, CGNAT, multicast, and IPv4-mapped/compatible IPv6 ranges; ICS fetches disable redirects and cap responses at 2 MB / 500 events; Web Push payloads are RFC 8291 AES-128-GCM encrypted and RFC 8292 VAPID signed, auto-revoking `404`/`410` subscriptions.
- **PDPA Minimization & Retention:** Initial passwords are randomly generated (never derived from NRIC or defaults) and redacted from `email_logs` and structured logs (`core::redact::email`); attendance network learning never exposes `employee_id → IP` mappings; automated daily jobs enforce 7-year (`2,555`-day) audit retention alongside shorter purges for sessions, tokens, reset/challenge states, finished jobs, unreferenced uploads, email logs, push subscriptions/outbox, and network observations. See [docs/legal.md](docs/legal.md#part-ii-privacy-policy-and-pdpa-notice) for the **72-hour mandatory Data Breach Notification (Section 12B PDPA)** procedure.

## Known Prototype & Architectural Considerations

The repository requires independent organizational review before deployment with live employee data:

- **Application-Enforced Tenancy:** Tenant isolation is enforced by capability permissions, composite `(parent_id, company_id)` foreign keys, immutable `company_id` triggers, and company-scoped SQL predicates (verified by `route_inventory_tests` and `route_auth_tests`); PostgreSQL row-level security (`RLS`) is not enabled.
- **Kiosk Display URLs:** The public kiosk display route (`/kiosk#<secret>`, with legacy `/kiosk/:kioskKey` support) passes its revocable credential in the URL fragment, loads it into memory, and scrubs it immediately via `history.replaceState` so wall-mounted displays survive setup without leaking secrets in HTTP `Referer` headers or server logs; kiosk devices should still be physically secured and rotated if decommissioned.
- **In-Process Background Workers:** Scheduled tasks coordinate across replicas via PostgreSQL advisory locks (`pg_try_advisory_lock`) and atomic `background_jobs` / `FOR UPDATE SKIP LOCKED` claiming, though execution occurs inside the API process rather than a separate worker tier.
- **Statutory Fail-Closed Gate:** Statutory rows shipped in `1001_data.sql` are unverified academic fixtures. Production calculations fail closed unless source-linked EPF/SOCSO/EIS schedules are independently verified (`status = 'verified'`), and automatic PCB remains disabled until the calculator passes LHDN computerised-MTD conformance testing.

Fresh databases contain no demo organizations or known credentials. Create the initial administrator with `cargo run --bin bootstrap_admin` and unset its password environment variable immediately after use.
