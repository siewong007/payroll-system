# Cloudflare Turnstile — Design

Date: 2026-09-12
Status: Approved (brainstorm)

Add Cloudflare Turnstile (managed mode) to every unauthenticated,
bot-abusable API endpoint. Keys are operator-supplied: site key to the
frontend build, secret to the backend environment. When the backend secret
is unset, verification is skipped entirely — local dev and CI need no keys.

## Goals / non-goals

- Stop credential stuffing on `/auth/login`, email-enumeration on
  forgot/reset + `passkey/check`, and challenge-mint spam on the passkey
  `begin` endpoints and the OAuth2 authorize endpoint.
- Not a replacement for rate limiting — `tower_governor` stays as-is.
- No authenticated endpoint changes; attendance check-in/out, refresh, and
  all `AuthUser` routes are out of scope.

## Backend

### Config (`core/config.rs`)

- `turnstile_secret_key: Option<String>` from `TURNSTILE_SECRET_KEY`
  (`env::var(...).ok().filter(|v| !v.is_empty())`, same shape as the Google
  OAuth keys).
- `turnstile_verify_url: String` from `TURNSTILE_VERIFY_URL`, defaulting to
  `https://challenges.cloudflare.com/turnstile/v0/siteverify`. Exists so a
  test can point verification at an in-process stub server; operators never
  set it.

### Verifier (`core/turnstile.rs`, new)

```rust
pub async fn verify(
    config: &AppConfig,
    token: Option<&str>,
    remote_ip: Option<&str>,
) -> AppResult<()>
```

- Secret unset → `Ok(())` (skip; dev/CI path).
- Secret set, token absent/empty → `AppError::BadRequest("Human verification
  required")`.
- POST form `secret` / `response` / `remoteip` (remoteip omitted when
  unknown) via the existing `reqwest` dep, ~5 s timeout.
- `success == false` → `AppError::Forbidden("Human verification failed")`;
  log `error-codes` server-side, do not echo them to the client.
- Transport error or non-2xx from Cloudflare → `AppError::Internal`
  (fail closed — a captcha that passes when it cannot run protects nothing).

### Protected endpoints and DTOs

Add `#[serde(default)] pub turnstile_token: Option<String>` to:

| Endpoint | DTO (file) |
|---|---|
| `POST /auth/login` | `LoginRequest` (`models/user.rs`) |
| `POST /auth/forgot-password` | `ForgotPasswordRequest` (`models/session.rs`) |
| `POST /auth/reset-password` | `ResetPasswordRequest` (`models/session.rs`) |
| `POST /auth/passkey/check` | `CheckPasskeyRequest` (`models/passkey.rs`) |
| `POST /auth/passkey/authenticate/begin` | `AuthBeginRequest` (`models/passkey.rs`) |
| `POST /auth/passkey/discoverable/begin` | new `DiscoverableAuthBeginRequest` in `models/passkey.rs` (currently body-less — add a JSON body) |
| `GET /auth/oauth2/google/authorize` | `OAuth2AuthorizeQuery` (`models/oauth2.rs`) gains `turnstile_token: Option<String>` |

In each handler, first statement after extraction:

```rust
turnstile::verify(
    &state.config,
    req.turnstile_token.as_deref(),
    audit_meta.ip_address.as_deref(),
)
.await?;
```

`google_authorize` does not currently extract `AuditRequestMeta` — add it so
`remoteip` flows through the same `core::client_ip` path as the audit trail.

`serde(default)` keeps every existing test client compiling and lets
unconfigured deployments omit the field.

### Deliberately excluded (unauthenticated but captcha-proof or pointless)

- `GET /health`, `/health/ready` — LB/monitor probes cannot solve a widget.
- `POST /auth/refresh` — httpOnly-cookie-gated, fires unattended on load.
- `POST /auth/2fa/verify` — gated by `mfa_token`, minted only after a
  captcha-protected primary auth; already rate-limited.
- `POST /auth/validate-reset-token` — the high-entropy token is itself the
  credential; a bot gains nothing.
- `POST /attendance/kiosk/qr` — kiosk-secret-gated; an always-on kiosk
  refreshes unattended and a widget would wedge it.
- `POST /auth/passkey/{authenticate,discoverable}/complete` — useless
  without a WebAuthn assertion over a challenge from protected `begin`.
- `GET /auth/oauth2/providers`, `GET /auth/oauth2/google/callback` — one
  only mirrors what the login page shows; the other is a Google redirect
  that cannot carry a token (state + PKCE binder still apply).

## Frontend

### Site key

`VITE_TURNSTILE_SITE_KEY` at build time (`frontend/.env.production`, plus
`.env.example` documentation). Unset → no widget renders, no token sent.

### `TurnstileWidget` component (`components/TurnstileWidget.tsx`, new)

Hand-rolled, no new dependency:

- `useEffect` injects
  `https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit`
  once (guard on an existing script tag / `window.turnstile`), `onload`
  then `turnstile.render(div, { sitekey, callback, 'error-callback',
  'expired-callback' })`; cleanup removes the widget via
  `turnstile.remove(widgetId)`.
- Props: `onVerify(token: string)`, `onStateChange?(state)`; exposes a
  `reset()` via forwarded ref (tokens are single-use — parent must reset
  after each failed submit).
- Renders nothing when `VITE_TURNSTILE_SITE_KEY` is unset.
- `declare global` / ambient `Window['turnstile']` typing locally; no
  `@types` package needed.

### Pages and API modules

- `pages/auth/Login.tsx` — widget under the form; its token feeds password
  login, `passkey/check` + `authenticate/begin`, `discoverable/begin`, and
  the Google authorize call. Submit disabled until token present (when key
  configured). Reset widget on error.
- `pages/auth/ForgotPassword.tsx`, `pages/auth/ResetPassword.tsx` — same
  pattern.
- `api/auth.ts` / `admin.ts` / `passkey.ts` / `oauth2.ts` — accept and send
  `turnstile_token` (body field; query param for authorize).
- `frontend/src/types/` — extend request payload types per CONTRIBUTING.
- Error path: a 400/403 verification failure shows a message and resets the
  widget; `onExpire` clears stored token state.

### Env/deploy wiring

- `.env.example`: `TURNSTILE_SECRET_KEY=` (commented, with a note that
  unset = verification off) and `TURNSTILE_VERIFY_URL` left undocumented or
  commented as test-only.
- `frontend/.env.production`: `VITE_TURNSTILE_SITE_KEY=` placeholder line.
- `deploy/docker-compose.prod.yml`: `TURNSTILE_SECRET_KEY:
  "${TURNSTILE_SECRET_KEY:-}"` alongside the `GOOGLE_CLIENT_*` optional vars
  (empty default = verification off, matching the skip-when-unset design).

## Testing

- `core::turnstile::verify` unit tests: no-secret → Ok (existing handler
  tests cover the rest unchanged). HTTP path: spin a tiny `axum::serve`
  stub in the test returning `{"success":false}` / `{"success":true}`,
  point `turnstile_verify_url` at it, assert 403 / pass. No new dev-deps.
- Frontend Vitest: Login renders no widget without the env key; POST body
  contains `turnstile_token` when a token is set (mock the widget/`window
  .turnstile`).
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `bun run lint`,
  `bun run typecheck`, `bun run test` must stay green.
- No `.sqlx` regeneration needed — no query macros change.

## Risks / notes

- **Token single-use**: every protected submit must come from a fresh or
  reset widget; Login page shares one widget across password/passkey/
  Google paths, so reset on any failure.
- **Fail-closed on Cloudflare outage**: login page becomes unusable if
  siteverify is unreachable — accepted trade-off; removing the secret
  restores access instantly.
- **Kiosk fleet**: kiosks are IP-shared behind one egress; Turnstile is
  deliberately NOT on `kiosk/qr`, so no change there.
