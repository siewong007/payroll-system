# Cloudflare Turnstile Implementation Plan

> **For agentic workers:** Execute task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Require a Cloudflare Turnstile token on every unauthenticated, bot-abusable endpoint; skip verification entirely when the secret is unset.

**Architecture:** `core/turnstile.rs` posts the token to Cloudflare `siteverify`; each protected handler calls it first. Frontend renders a hand-rolled `TurnstileWidget` (managed mode) and sends `turnstile_token` in request bodies (query param for the OAuth authorize GET).

**Tech Stack:** Rust/Axum + reqwest (existing dep), React 19 + Vite env vars.

## Global Constraints

- Verification OFF when `TURNSTILE_SECRET_KEY` unset — dev/CI/tests need no keys.
- `turnstile_token` is `Option<String>` with `#[serde(default)]` everywhere — existing clients/tests keep working.
- The real secret lives ONLY in root `.env` (gitignored) and the deploy environment — never committed.
- Site key `0x4AAAAAAExKOU4xJzQjfmQR` is public by design; `frontend/.env.production` (tracked) + `frontend/.env.local` (gitignored, for dev).
- No new dependencies anywhere. No SQL changes → no `.sqlx` regen.
- CI gates: `cargo fmt --check`, `cargo clippy -- -D warnings`, `bun run lint`, `bun run typecheck`, `bun run test`.

---

### Task 1: Backend — config + `core::turnstile` verifier

**Files:**
- Modify: `backend/src/core/config.rs` (add 2 fields + env reads)
- Create: `backend/src/core/turnstile.rs`
- Modify: `backend/src/core/mod.rs` (add `pub mod turnstile;`)

**Interfaces:**
- Produces: `turnstile::verify(config: &AppConfig, token: Option<&str>, remote_ip: Option<&str>) -> AppResult<()>`; `config.turnstile_secret_key: Option<String>`; `config.turnstile_verify_url: String`.

- [ ] **Step 1: Write `core/turnstile.rs`**

```rust
//! Cloudflare Turnstile server-side verification for unauthenticated,
//! bot-abusable endpoints (login, password reset, passkey begins, OAuth
//! authorize).
//!
//! Verification is a no-op when `TURNSTILE_SECRET_KEY` is unset so local
//! dev and CI need no keys. When it IS set the outcome is fail-closed in
//! both directions: a missing token is a 400, a rejected token is a 403,
//! and Cloudflare being unreachable is a 502 — a captcha that passes when
//! it cannot run protects nothing.

use std::time::Duration;

use serde::Deserialize;

use crate::core::config::AppConfig;
use crate::core::error::{AppError, AppResult};

const VERIFY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Deserialize)]
struct SiteverifyResponse {
    success: bool,
    #[serde(default, rename = "error-codes")]
    error_codes: Vec<String>,
}

/// Check `token` with Cloudflare. `remote_ip` is the server-observed client
/// address (same `core::client_ip` resolution the audit trail uses); it is
/// optional evidence for Cloudflare, never load-bearing.
pub async fn verify(
    config: &AppConfig,
    token: Option<&str>,
    remote_ip: Option<&str>,
) -> AppResult<()> {
    let Some(secret) = config.turnstile_secret_key.as_deref() else {
        return Ok(());
    };
    let token = token
        .filter(|t| !t.is_empty())
        .ok_or_else(|| AppError::BadRequest("Human verification required".into()))?;

    let client = reqwest::Client::builder()
        .timeout(VERIFY_TIMEOUT)
        .build()
        .map_err(|e| {
            tracing::warn!("Turnstile: could not build HTTP client: {}", e);
            verify_unavailable()
        })?;

    let mut form = vec![("secret", secret), ("response", token)];
    if let Some(ip) = remote_ip {
        form.push(("remoteip", ip));
    }

    let outcome = client
        .post(&config.turnstile_verify_url)
        .form(&form)
        .send()
        .await
        .and_then(|r| r.error_for_status());

    let body = match outcome {
        Ok(resp) => resp.json::<SiteverifyResponse>().await.map_err(|e| {
            tracing::warn!("Turnstile: unreadable siteverify body: {}", e);
            verify_unavailable()
        })?,
        Err(e) => {
            tracing::warn!("Turnstile: siteverify request failed: {}", e);
            return Err(verify_unavailable());
        }
    };

    if body.success {
        Ok(())
    } else {
        tracing::info!("Turnstile rejected token: {:?}", body.error_codes);
        Err(AppError::Forbidden("Human verification failed".into()))
    }
}

/// Uniform 502 for every "could not reach/understand Cloudflare" outcome —
/// `BadGateway` exists so a third party's outage is not reported as our bug.
fn verify_unavailable() -> AppError {
    AppError::BadGateway("Human verification is temporarily unavailable".into())
}
```

- [ ] **Step 2: Wire config + module**

`core/config.rs` — add fields to `AppConfig` (after `trust_proxy_headers`):

```rust
    /// Cloudflare Turnstile secret. Unset = verification skipped on every
    /// protected endpoint (local dev, CI). Set in production only.
    pub turnstile_secret_key: Option<String>,
    /// Operator-trusted verify endpoint; overridden only by tests pointing
    /// at an in-process stub. Never tenant-supplied, so no SSRF policy applies.
    pub turnstile_verify_url: String,
```

and in `from_env()` (end of the struct literal):

```rust
            turnstile_secret_key: env::var("TURNSTILE_SECRET_KEY")
                .ok()
                .filter(|v| !v.is_empty()),
            turnstile_verify_url: env::var("TURNSTILE_VERIFY_URL").unwrap_or_else(|_| {
                "https://challenges.cloudflare.com/turnstile/v0/siteverify".to_string()
            }),
```

`core/mod.rs` — add `pub mod turnstile;` (alphabetical, after `timezone`).

- [ ] **Step 3: Write tests (inline `#[cfg(test)] mod tests` in turnstile.rs)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Literal fixture — `AppConfig::from_env()` panics without env vars, so
    // tests must not call it.
    fn config(secret: Option<&str>, url: &str) -> AppConfig {
        AppConfig {
            database_url: String::new(),
            jwt_secret: String::new(),
            totp_encryption_key: String::new(),
            jwt_expiry_hours: 1,
            server_host: String::new(),
            server_port: 0,
            frontend_url: String::new(),
            api_public_url: String::new(),
            google_client_id: None,
            google_client_secret: None,
            webauthn_rp_id: String::new(),
            webauthn_rp_origin: String::new(),
            smtp_host: None,
            smtp_port: None,
            smtp_username: None,
            smtp_password: None,
            smtp_from_email: None,
            smtp_from_name: None,
            trust_proxy_headers: false,
            turnstile_secret_key: secret.map(str::to_string),
            turnstile_verify_url: url.to_string(),
        }
    }

    #[tokio::test]
    async fn skips_when_secret_unset() {
        let c = config(None, "http://127.0.0.1:1/unused");
        assert!(verify(&c, None, None).await.is_ok());
    }

    #[tokio::test]
    async fn rejects_missing_token_when_configured() {
        let c = config(Some("secret"), "http://127.0.0.1:1/unused");
        let err = verify(&c, None, None).await.unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    // Tiny in-process stub: no new dev-deps.
    async fn stub(success: bool) -> String {
        let app = axum::Router::new().route(
            "/siteverify",
            axum::routing::post(move || async move {
                axum::Json(serde_json::json!({"success": success}))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/siteverify")
    }

    #[tokio::test]
    async fn passes_on_cloudflare_success() {
        let url = stub(true).await;
        let c = config(Some("secret"), &url);
        assert!(verify(&c, Some("tok"), Some("1.2.3.4")).await.is_ok());
    }

    #[tokio::test]
    async fn forbids_on_cloudflare_failure() {
        let url = stub(false).await;
        let c = config(Some("secret"), &url);
        let err = verify(&c, Some("tok"), None).await.unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn bad_gateway_when_unreachable() {
        let c = config(Some("secret"), "http://127.0.0.1:1/siteverify");
        let err = verify(&c, Some("tok"), None).await.unwrap_err();
        assert!(matches!(err, AppError::BadGateway(_)));
    }
}
```

- [ ] **Step 4: `cargo test turnstile` — all pass; `cargo fmt`, commit**

```bash
git add backend/src/core/{config.rs,mod.rs,turnstile.rs}
git commit -m "feat(backend): add Turnstile siteverify helper (skip when unconfigured)"
```

---

### Task 2: Backend — DTO fields + handler wiring (7 endpoints)

**Files:**
- Modify: `backend/src/models/user.rs` (`LoginRequest`), `models/session.rs` (`ForgotPasswordRequest`, `ResetPasswordRequest`), `models/passkey.rs` (`CheckPasskeyRequest`, `AuthBeginRequest`, + new `DiscoverableAuthBeginRequest`), `models/oauth2.rs` (`OAuth2AuthorizeQuery`)
- Modify: `backend/src/handlers/auth.rs`, `handlers/passkey.rs`, `handlers/oauth2.rs`

**Interfaces:**
- Consumes: `turnstile::verify` from Task 1.
- Produces: every protected request accepts optional `turnstile_token`.

- [ ] **Step 1: Add the field to each DTO**

```rust
// models/user.rs — inside LoginRequest
    #[serde(default)]
    pub turnstile_token: Option<String>,
// models/session.rs — inside ForgotPasswordRequest AND ResetPasswordRequest (same line each)
// models/passkey.rs — inside CheckPasskeyRequest AND AuthBeginRequest
// models/passkey.rs — new struct:
#[derive(Deserialize)]
pub struct DiscoverableAuthBeginRequest {
    #[serde(default)]
    pub turnstile_token: Option<String>,
}
// models/oauth2.rs — inside OAuth2AuthorizeQuery
    pub turnstile_token: Option<String>,
```

- [ ] **Step 2: Verify first inside each handler**

`handlers/auth.rs` — `login`, `forgot_password`, `reset_password`: insert as first statement after extraction:

```rust
    crate::core::turnstile::verify(
        &state.config,
        req.turnstile_token.as_deref(),
        audit_meta.ip_address.as_deref(),
    )
    .await?;
```

`handlers/passkey.rs` — same verify insert in `check_passkey` and `authentication_begin`. Neither currently extracts `audit_meta`, so add `audit_meta: AuditRequestMeta` to both signatures (`AuditRequestMeta` is already imported in this file — `authentication_complete` uses it). `discoverable_auth_begin` gains a body:

```rust
pub async fn discoverable_auth_begin(
    State(state): State<AppState>,
    audit_meta: AuditRequestMeta,
    Json(req): Json<DiscoverableAuthBeginRequest>,
) -> AppResult<Json<DiscoverableAuthBeginResponse>> {
    crate::core::turnstile::verify(
        &state.config,
        req.turnstile_token.as_deref(),
        audit_meta.ip_address.as_deref(),
    )
    .await?;
    // ...existing body unchanged
```

`handlers/oauth2.rs` — `google_authorize` gains `audit_meta: AuditRequestMeta` and:

```rust
    crate::core::turnstile::verify(
        &state.config,
        query.turnstile_token.as_deref(),
        audit_meta.ip_address.as_deref(),
    )
    .await?;
```

- [ ] **Step 3: `cargo clippy -- -D warnings` + `cargo test` (suite unchanged — secret unset) + commit**

```bash
git commit -m "feat(backend): require Turnstile token on unauthenticated endpoints"
```

---

### Task 3: Env plumbing

**Files:**
- Modify: `.env.example` (document `TURNSTILE_SECRET_KEY` + test-only `TURNSTILE_VERIFY_URL`)
- Modify: `deploy/docker-compose.prod.yml` — `TURNSTILE_SECRET_KEY: "${TURNSTILE_SECRET_KEY:-}"` beside `GOOGLE_CLIENT_*`
- Modify (untracked): root `.env` — `TURNSTILE_SECRET_KEY=0x4AAAAAAExKOUIp448uo-mzWZAsPfvZvgY`

- [ ] Commit the two tracked files: `git commit -m "chore: plumb TURNSTILE_SECRET_KEY through env + prod compose"`. Never `git add .env`.

---

### Task 4: Frontend — `TurnstileWidget` + site key env

**Files:**
- Create: `frontend/src/components/TurnstileWidget.tsx`
- Modify: `frontend/.env.production` — add `VITE_TURNSTILE_SITE_KEY=0x4AAAAAAExKOU4xJzQjfmQR`
- Create (gitignored): `frontend/.env.local` — same line (dev needs it once the backend secret is set)

**Interfaces:**
- Produces: `<TurnstileWidget ref={ref} onVerify={(t) => ...} onExpire={() => ...} onError={() => ...} />`; ref type `TurnstileWidgetRef { reset(): void }`. Renders nothing when `import.meta.env.VITE_TURNSTILE_SITE_KEY` unset.

- [ ] **Step 1: Component** — inject `api.js?render=explicit` once; `turnstile.render` with `sitekey`, wire `callback`/`expired-callback`/`error-callback`; cleanup `turnstile.remove(id)`; `useImperativeHandle` → `reset()`. Ambient `Window['turnstile']` type declared in-file (no @types dep).

- [ ] **Step 2: Vitest** (`tests/Turnstile.test.tsx`) — renders nothing without the env key; with key + mocked `window.turnstile`, calls `render` and fires `onVerify`.

- [ ] **Step 3: `bun run typecheck` + `bun run test` + commit**

```bash
git commit -m "feat(frontend): add hand-rolled TurnstileWidget component"
```

---

### Task 5: Frontend — wire pages + API modules

**Files:**
- Modify: `api/admin.ts` (`forgotPassword`, `resetPassword` accept token), `api/passkey.ts` (all 3 public calls send `turnstile_token`), `api/oauth2.ts` (`getGoogleAuthorizeUrl(flow?, turnstileToken?)` → query param)
- Modify: `context/AuthContext.ts` (`login(email, password, turnstileToken?)`), `context/AuthProvider.tsx` (send field)
- Modify: `pages/auth/Login.tsx`, `ForgotPassword.tsx`, `ResetPassword.tsx`, `components/LinkedAccounts.tsx`
- Modify: `tests/Auth.test.tsx`, `tests/WebAuthn.test.ts` expectations (new request bodies)

- [ ] **Step 1:** API modules — add optional `turnstile_token` to payloads / params.
- [ ] **Step 2:** `Login.tsx` — one shared widget; `tokenRef` pattern: `onVerify` stores token, each protected call reads it and calls `ref.reset()` immediately after the request resolves/rejects (single-use). `checkPasskey` sends token only if present (skip silently otherwise — discoverable flow is the fallback). Submit buttons need NOT block on the token — server enforces.
- [ ] **Step 3:** `ForgotPassword.tsx`, `ResetPassword.tsx` — widget above submit; pass token; reset on error.
- [ ] **Step 4:** `LinkedAccounts.tsx` — widget (renders only when key set); `startLink` passes token to `getGoogleAuthorizeUrl('link', token)`; reset on error.
- [ ] **Step 5:** Update existing test expectations (`api.post` called-with args now include `turnstile_token` key or `undefined` field — match implementation).
- [ ] **Step 6:** `bun run lint` + `bun run typecheck` + `bun run test` + `bun run build`; commit `feat(frontend): send Turnstile token from public auth flows`.

---

### Task 6: End-to-end verification

- [ ] Backend: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
- [ ] Frontend: `bun run lint`, `bun run typecheck`, `bun run test`, `bun run build`.
- [ ] Manual smoke (keys in `.env` + `frontend/.env.local`): `cargo run` + `bun run dev`; login page shows widget; wrong/absent token → 400/403; real login succeeds.
- [ ] Update `CLAUDE.md`/`AGENTS.md` auth paragraph + `frontend/src/types/index.ts` request shapes per CONTRIBUTING.
