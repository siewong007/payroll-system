# Settings Redesign + Session Control Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild `/settings` as a grouped, sidebar-nav settings hub using the app's glass design system; surface session control with device icons, IP addresses, and relative timestamps; add a change-password card.

**Architecture:** Left in-page nav (`?section=` deep links) splits the page into Workspace sections (existing company-setting categories, same `SettingField` renderer + per-section sticky save bar) and Account sections (Security = password/2FA/passkeys/linked accounts; Sessions = rewritten `SessionManagement`). Backend plumbing adds the already-migrated-but-unwritten `user_sessions.ip_address` column, fed from `AuditRequestMeta` at session create and refreshed on token rotation.

**Tech Stack:** React 19 + TS + Tailwind v4 (CSS-var theme classes in `index.css`), react-query, framer-motion, lucide-react; Rust/Axum + sqlx (runtime queries in this module — no `.sqlx` regen needed).

## Global Constraints

- Money/dates/etc. untouched here; but follow repo layering: handlers thin, SQL only in `repositories/`, services return `AppResult<T>`.
- `user_sessions.rs` uses runtime `sqlx::query` — keep that style (no `query!` macros → no `.sqlx` cache regen).
- Frontend uses the shared classes in `src/index.css`: `.card`, `.btn-primary`, `.btn-secondary`, `.form-input`, `.form-label`, `.section-header`, `.section-title`, `.badge*`, `.spinner`, `.glow-dot`, `.animate-fade-up`. Do NOT introduce `bg-white rounded-2xl shadow` panels.
- All session-revoke/destructive confirms use `components/ui/Modal.tsx` — no `window.confirm`.
- Access token stays in-memory; API calls go through `@/api/client` (`api.get/put/delete`). Never a second axios instance.
- Backend: `cargo fmt` style, `clippy -D warnings` clean. Frontend: `bun run lint`, `bun run typecheck`, `bun run test` clean.
- Commits: conventional style like `feat(auth): …` / `feat(frontend): …` seen in `git log`. Commit per task.
- Run `./scripts/codegraph update .` after code changes (repo rule).

---

### Task 1: Backend — populate `user_sessions.ip_address`

The column already exists (migration `1003`, `ip_address inet`). Every login path funnels through `auth_service::issue_session`, which already receives `audit_meta: Option<&AuditRequestMeta>` (`ip_address: Option<String>`). Refresh rotation goes through `refresh_session` → `issue_rotated` → `user_sessions::touch`.

**Files:**
- Modify: `backend/src/models/session.rs` — `UserSession` + `UserSessionResponse` gain `ip_address`
- Modify: `backend/src/repositories/user_sessions.rs` — `insert`, `touch`, `list_active`
- Modify: `backend/src/services/session_service.rs` — `create_session`, `create_refresh_token`, `issue_rotated`
- Modify: `backend/src/services/auth_service.rs` — `issue_session`, `refresh_session`
- Modify: `backend/src/handlers/auth.rs` — `refresh_token` gains `AuditRequestMeta` extractor
- Modify: test call sites `backend/src/tests/{totp_route_tests,route_auth_tests,session_rotation_tests,auth_audit_tests}.rs`

**Interfaces:**
- `create_session(pool, user_id, user_agent, ip_address: Option<IpAddr>) -> AppResult<(Uuid, String)>`
- `refresh_session(pool, raw_token, jwt_secret, jwt_expiry, ip_address: Option<IpAddr>)`
- `UserSessionResponse.ip_address: Option<IpAddr>` (serde serializes `IpAddr` as a string)

- [ ] **Step 1: Update the model** — `backend/src/models/session.rs`:

```rust
use std::net::IpAddr;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_agent: Option<String>,
    pub ip_address: Option<IpAddr>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
```

Add `pub ip_address: Option<IpAddr>` to `UserSessionResponse` and populate it in `from_session` (`ip_address: session.ip_address`).

- [ ] **Step 2: Update the repository** — `backend/src/repositories/user_sessions.rs`:

```rust
use std::net::IpAddr;

pub async fn insert(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    user_id: Uuid,
    user_agent: Option<&str>,
    ip_address: Option<IpAddr>,
    expires_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO user_sessions (id, user_id, user_agent, ip_address, expires_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(user_id)
    .bind(user_agent)
    .bind(ip_address)
    .bind(expires_at)
    .execute(executor)
    .await?;
    Ok(())
}
```

`list_active`: add `ip_address` to the SELECT list.

`touch` — keep the last known address when the current request can't resolve one:

```rust
pub async fn touch(
    executor: impl Executor<'_, Database = Postgres>,
    session_id: Uuid,
    expires_at: DateTime<Utc>,
    ip_address: Option<IpAddr>,
) -> AppResult<()> {
    sqlx::query("UPDATE user_sessions SET last_seen_at = NOW(), expires_at = $2, ip_address = COALESCE($3, ip_address) WHERE id = $1")
        .bind(session_id)
        .bind(expires_at)
        .bind(ip_address)
        .execute(executor)
        .await?;
    Ok(())
}
```

- [ ] **Step 3: Update the service** — `session_service.rs`: add `ip_address: Option<IpAddr>` param to `create_session`, `create_refresh_token` (pass to `user_sessions::insert`), and `issue_rotated` (pass to `user_sessions::touch`). Add `use std::net::IpAddr;`.

- [ ] **Step 4: Update `auth_service.rs`** — in `issue_session`, before the `create_session` call:

```rust
let ip_address = audit_meta
    .and_then(|m| m.ip_address.as_deref())
    .and_then(|s| s.parse().ok());
let (session_id, refresh_token) =
    session_service::create_session(pool, user.id, user_agent, ip_address).await?;
```

`refresh_session` gains a trailing `ip_address: Option<IpAddr>` param, passed to `session_service::issue_rotated(&mut tx, user.id, token.session_id, ip_address)`.

- [ ] **Step 5: Update the refresh handler** — `handlers/auth.rs::refresh_token` adds `audit_meta: AuditRequestMeta` after `headers: HeaderMap` (the extractor is infallible), then:

```rust
let ip_address = audit_meta.ip_address.as_deref().and_then(|s| s.parse().ok());
let refreshed = auth_service::refresh_session(
    &state.pool,
    &refresh,
    &state.config.jwt_secret,
    state.config.jwt_expiry_hours,
    ip_address,
)
.await?;
```

- [ ] **Step 6: Fix call sites in tests** — every `session_service::create_session(&pool, user_id, None)` becomes `…, None, None)`; every `auth_service::refresh_session(&pool, &raw, JWT_SECRET, 1)` gets a trailing `None`. `cargo build --tests` surfaces each site mechanically.

- [ ] **Step 7: Verify** — `cd backend && cargo fmt && cargo clippy -- -D warnings && cargo build --tests`. If a migrated dev DB is up (`docker compose up -d`, `DATABASE_URL` set), `cargo test session` too.

- [ ] **Step 8: Commit**

```bash
git add backend/src && git commit -m "feat(auth): record client IP on user sessions"
```

---

### Task 2: Frontend — `userAgent` parser + `formatRelativeTime`

**Files:**
- Create: `frontend/src/lib/userAgent.ts`
- Modify: `frontend/src/lib/utils.ts` (append `formatRelativeTime`)
- Test: `frontend/src/tests/userAgent.test.ts`

**Interfaces:**
- `parseUserAgent(ua: string | null | undefined) -> { deviceType: 'desktop'|'mobile'|'tablet'|'unknown', os: string, browser: string }`
- `deviceLabel(p: ParsedUserAgent) -> string` (e.g. `"Chrome on macOS"`, `"Unknown device"`)
- `formatRelativeTime(date: string | Date) -> string`

- [ ] **Step 1: Write the failing tests** — `frontend/src/tests/userAgent.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { parseUserAgent, deviceLabel } from '@/lib/userAgent';
import { formatRelativeTime } from '@/lib/utils';

const CHROME_MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36';
const SAFARI_IPHONE = 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1';
const EDGE_WIN = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0';
const IPAD = 'Mozilla/5.0 (iPad; CPU OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/604.1';

describe('parseUserAgent', () => {
  it('parses Chrome on macOS as desktop', () => {
    expect(parseUserAgent(CHROME_MAC)).toEqual({ deviceType: 'desktop', os: 'macOS', browser: 'Chrome' });
  });
  it('parses Safari on iPhone as mobile', () => {
    expect(parseUserAgent(SAFARI_IPHONE)).toEqual({ deviceType: 'mobile', os: 'iOS', browser: 'Safari' });
  });
  it('detects Edge before Chrome (its UA contains Chrome)', () => {
    expect(parseUserAgent(EDGE_WIN).browser).toBe('Edge');
  });
  it('detects iPad as tablet', () => {
    expect(parseUserAgent(IPAD)).toEqual({ deviceType: 'tablet', os: 'iOS', browser: 'Safari' });
  });
  it('returns unknowns for missing/garbage UA', () => {
    expect(parseUserAgent(null).deviceType).toBe('unknown');
    expect(deviceLabel(parseUserAgent(null))).toBe('Unknown device');
    expect(deviceLabel(parseUserAgent('curl/8.0'))).toBe('Unknown device');
  });
  it('labels a parsed device as "Browser on OS"', () => {
    expect(deviceLabel(parseUserAgent(CHROME_MAC))).toBe('Chrome on macOS');
  });
});

describe('formatRelativeTime', () => {
  it('formats recency buckets', () => {
    const now = Date.now();
    expect(formatRelativeTime(new Date(now - 30_000))).toBe('just now');
    expect(formatRelativeTime(new Date(now - 5 * 60_000))).toBe('5 min ago');
    expect(formatRelativeTime(new Date(now - 3 * 3_600_000))).toBe('3 hr ago');
    expect(formatRelativeTime(new Date(now - 2 * 86_400_000))).toBe('2 days ago');
  });
});
```

- [ ] **Step 2: Run to verify failure** — `cd frontend && bun run test -- userAgent` → FAIL (module missing).

- [ ] **Step 3: Implement** — `frontend/src/lib/userAgent.ts`:

```ts
export type DeviceType = 'desktop' | 'mobile' | 'tablet' | 'unknown';

export interface ParsedUserAgent {
  deviceType: DeviceType;
  os: string;
  browser: string;
}

/** Order matters: Edge/Opera UAs also contain "Chrome", and Chrome's contains "Safari". */
export function parseUserAgent(ua: string | null | undefined): ParsedUserAgent {
  if (!ua) return { deviceType: 'unknown', os: 'Unknown OS', browser: 'Unknown browser' };

  const browser = /Edg(e|A|iOS)?\//.test(ua) ? 'Edge'
    : /OPR\/|Opera/.test(ua) ? 'Opera'
    : /Chrome\//.test(ua) ? 'Chrome'
    : /Firefox\//.test(ua) ? 'Firefox'
    : /Safari\//.test(ua) ? 'Safari'
    : 'Unknown browser';

  const os = /iPhone|iPad|iPod/.test(ua) ? 'iOS'
    : /Android/.test(ua) ? 'Android'
    : /Windows/.test(ua) ? 'Windows'
    : /Mac OS X|Macintosh/.test(ua) ? 'macOS'
    : /CrOS/.test(ua) ? 'ChromeOS'
    : /Linux/.test(ua) ? 'Linux'
    : 'Unknown OS';

  const deviceType: DeviceType =
    /iPad|Tablet/.test(ua) || (/Android/.test(ua) && !/Mobile/.test(ua)) ? 'tablet'
    : /iPhone|iPod|Mobile/.test(ua) ? 'mobile'
    : os === 'Unknown OS' && browser === 'Unknown browser' ? 'unknown'
    : 'desktop';

  return { deviceType, os, browser };
}

export function deviceLabel(p: ParsedUserAgent): string {
  if (p.deviceType === 'unknown' && p.browser === 'Unknown browser') return 'Unknown device';
  if (p.browser === 'Unknown browser') return `Browser on ${p.os}`;
  return `${p.browser} on ${p.os}`;
}
```

Wait — `deviceLabel` for `curl/8.0`: deviceType 'unknown' + browser 'Unknown browser' → 'Unknown device' ✓. For a Macintosh-with-no-browser UA → 'Browser on macOS' ✓.

Append to `frontend/src/lib/utils.ts`:

```ts
export function formatRelativeTime(date: string | Date): string {
  const seconds = Math.max(0, Math.floor((Date.now() - new Date(date).getTime()) / 1000));
  if (seconds < 60) return 'just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} hr ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days} day${days === 1 ? '' : 's'} ago`;
  return formatDate(date);
}
```

- [ ] **Step 4: Run tests** — `bun run test -- userAgent` → PASS.

- [ ] **Step 5: Commit** — `feat(frontend): add user-agent parser and relative-time helper`.

---

### Task 3: Frontend — session type + `SessionManagement` rewrite

**Files:**
- Modify: `frontend/src/api/sessions.ts` — `ip_address: string | null` on `UserSession`
- Modify: `frontend/src/components/SessionManagement.tsx` — full rewrite
- Test: `frontend/src/tests/SecurityComponents.test.tsx` — replace the `SessionManagement` describe block

**Interfaces:**
- Consumes: `parseUserAgent`, `deviceLabel`, `formatRelativeTime`, `formatDate` (utils), `Modal` (`@/components/ui/Modal`), `getSessions`/`revokeSession`/`revokeOtherSessions` (unchanged API).
- Produces: `<SessionManagement />` — same name/props (none), still safe to mount in portal `MyProfile`.

- [ ] **Step 1: Update the type** — in `api/sessions.ts` add `ip_address: string | null;` to `UserSession`.

- [ ] **Step 2: Update the tests first** — replace the whole `describe('SessionManagement')` block in `SecurityComponents.test.tsx`. New fixtures (realistic UAs + `ip_address`):

```ts
const sessions = [
  {
    id: 'sess-current',
    user_agent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36',
    ip_address: '203.0.113.10',
    created_at: '2026-07-20T02:00:00Z',
    last_seen_at: new Date().toISOString(),
    expires_at: '2026-08-27T02:00:00Z',
    current: true,
  },
  {
    id: 'sess-phone',
    user_agent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1',
    ip_address: null,
    created_at: '2026-07-21T02:00:00Z',
    last_seen_at: '2026-07-26T02:00:00Z',
    expires_at: '2026-08-26T02:00:00Z',
    current: false,
  },
];
```

Tests (same coverage, new assertions):
- `labels devices as "Browser on OS" and badges the current one` → `findByText('Chrome on macOS')`, `getByText('Safari on iOS')`, `getByText('This device')`.
- `shows IP and Active now for the current session` → `getByText(/203\.0\.113\.10/)`, `getByText('Active now')`.
- `falls back to "Unknown device" when UA is missing`.
- `offers no sign-out control for the current session` → exactly 1 `Sign out device` button.
- `revokes a single session and refreshes the list` (unchanged flow).
- `confirms in a modal before signing out every other device` → click `Sign out all other sessions` → `findByRole('dialog')` → click its `Sign out` button → `revokeOtherSessions` called; a cancel click does not call it.
- `hides the bulk control when this is the only session` → queryByRole button name `Sign out all other sessions` absent.
- `reports an empty list` → `findByText('No active sessions found.')`.

- [ ] **Step 3: Run to verify failure** — `bun run test -- SecurityComponents` → FAIL on new assertions.

- [ ] **Step 4: Rewrite the component** — `SessionManagement.tsx`:

```tsx
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Globe, Laptop, LogOut, Monitor, ShieldCheck, Smartphone, Tablet } from 'lucide-react';
import { getSessions, revokeOtherSessions, revokeSession, type UserSession } from '@/api/sessions';
import { parseUserAgent, deviceLabel } from '@/lib/userAgent';
import { formatDate, formatRelativeTime } from '@/lib/utils';
import { Modal } from '@/components/ui/Modal';

const DEVICE_ICONS = { desktop: Monitor, mobile: Smartphone, tablet: Tablet, unknown: Globe } as const;

function lastActive(session: UserSession): string {
  if (Date.now() - new Date(session.last_seen_at).getTime() < 120_000) return 'Active now';
  return formatRelativeTime(session.last_seen_at);
}

export function SessionManagement() {
  const queryClient = useQueryClient();
  const [confirmingOthers, setConfirmingOthers] = useState(false);
  const { data: sessions = [], isLoading } = useQuery({ queryKey: ['sessions'], queryFn: getSessions });
  const invalidate = () => queryClient.invalidateQueries({ queryKey: ['sessions'] });
  const revoke = useMutation({ mutationFn: revokeSession, onSuccess: invalidate });
  const revokeOthers = useMutation({
    mutationFn: revokeOtherSessions,
    onSuccess: () => { setConfirmingOthers(false); invalidate(); },
  });
  const otherSessions = sessions.filter((s) => !s.current);

  return (
    <section className="card">
      <div className="section-header">
        <div className="flex items-center gap-2">
          <ShieldCheck className="w-4 h-4 text-gray-400" />
          <span className="section-title">Active sessions</span>
        </div>
        {otherSessions.length > 0 && (
          <button
            type="button"
            onClick={() => setConfirmingOthers(true)}
            className="ml-auto text-sm font-medium text-red-600 hover:text-red-700 whitespace-nowrap"
          >
            Sign out all other sessions
          </button>
        )}
      </div>

      <p className="text-sm text-gray-500 mb-4">
        Devices currently signed in to your account. Revoked devices will need to sign in again.
      </p>

      {isLoading ? (
        <div className="space-y-3" aria-hidden>
          {[0, 1].map((i) => (
            <div key={i} className="flex items-center gap-3 py-2">
              <div className="w-9 h-9 rounded-xl bg-gray-100" />
              <div className="flex-1 space-y-2">
                <div className="h-3 w-40 rounded bg-gray-100" />
                <div className="h-2.5 w-56 rounded bg-gray-50" />
              </div>
            </div>
          ))}
        </div>
      ) : sessions.length === 0 ? (
        <p className="py-3 text-sm text-gray-500">No active sessions found.</p>
      ) : (
        <div className="divide-y divide-gray-100">
          {sessions.map((session) => {
            const parsed = parseUserAgent(session.user_agent);
            const Icon = session.current ? Laptop : DEVICE_ICONS[parsed.deviceType];
            return (
              <div key={session.id} className="py-4 flex items-center gap-3">
                <div className="w-9 h-9 rounded-xl bg-[var(--accent-soft)] flex items-center justify-center shrink-0">
                  <Icon className="w-[18px] h-[18px] text-gray-600" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap gap-2 items-center">
                    <span className="text-sm font-medium text-gray-900">{deviceLabel(parsed)}</span>
                    {session.current && (
                      <span className="badge badge-approved inline-flex items-center gap-1.5">
                        <span className="glow-dot" /> This device
                      </span>
                    )}
                  </div>
                  <p className="text-xs text-gray-500 mt-1">
                    {session.ip_address ?? 'IP unknown'} · Signed in {formatDate(session.created_at)} · {lastActive(session)}
                  </p>
                </div>
                {!session.current && (
                  <button
                    type="button"
                    aria-label="Sign out device"
                    onClick={() => revoke.mutate(session.id)}
                    disabled={revoke.isPending}
                    className="p-2 text-gray-400 hover:text-red-600 hover:bg-red-50 rounded-lg transition-all-fast disabled:opacity-50"
                  >
                    <LogOut className="w-4 h-4" />
                  </button>
                )}
              </div>
            );
          })}
        </div>
      )}

      <Modal
        open={confirmingOthers}
        onClose={() => setConfirmingOthers(false)}
        title="Sign out other sessions?"
        maxWidth="max-w-md"
        footer={
          <div className="flex justify-end gap-2">
            <button type="button" className="btn-secondary" onClick={() => setConfirmingOthers(false)}>
              Cancel
            </button>
            <button
              type="button"
              onClick={() => revokeOthers.mutate()}
              disabled={revokeOthers.isPending}
              className="btn-primary !bg-none !bg-red-600 hover:!bg-red-700"
            >
              {revokeOthers.isPending ? 'Signing out…' : `Sign out ${otherSessions.length} session${otherSessions.length === 1 ? '' : 's'}`}
            </button>
          </div>
        }
      >
        <p className="text-sm text-gray-600">
          You will stay signed in on this device. {otherSessions.length} other
          session{otherSessions.length === 1 ? '' : 's'} will be signed out and must log in again.
        </p>
      </Modal>
    </section>
  );
}
```



- [ ] **Step 5: Run tests** — `bun run test -- SecurityComponents` → PASS. Then `bun run typecheck`.

- [ ] **Step 6: Commit** — `feat(frontend): richer session management with device, IP, and modal confirm`.

---

### Task 4: Frontend — `ChangePasswordCard`

**Files:**
- Create: `frontend/src/components/ChangePasswordCard.tsx`
- Test: `frontend/src/tests/ChangePasswordCard.test.tsx`

**Interfaces:**
- Consumes: `api.put('/auth/change-password', { current_password, new_password })`; `validatePassword`, `PASSWORD_POLICY_HINT` from `@/lib/password`; `useAuth().logout`; `useNavigate` from `react-router` (this repo's router import is `react-router`, matching `ChangePassword.tsx`).
- Backend behavior (verified): password change calls `user_sessions::revoke_all_for_user` + `refresh_tokens::revoke_all_for_user` — **all sessions die, including this one** — so success path = `await logout(); navigate('/login')`, same as `pages/auth/ChangePassword.tsx`.

- [ ] **Step 1: Write the failing test** — `frontend/src/tests/ChangePasswordCard.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ChangePasswordCard } from '@/components/ChangePasswordCard';

const putMock = vi.fn();
const logoutMock = vi.fn();
const navigateMock = vi.fn();

vi.mock('@/api/client', () => ({ default: { put: (...a: unknown[]) => putMock(...a) } }));
vi.mock('@/context/AuthContext', () => ({ useAuth: () => ({ logout: logoutMock }) }));
vi.mock('react-router', () => ({ useNavigate: () => navigateMock }));

describe('ChangePasswordCard', () => {
  beforeEach(() => { putMock.mockReset(); logoutMock.mockReset(); navigateMock.mockReset(); });

  it('rejects mismatched confirmation without calling the API', async () => {
    const typer = userEvent.setup();
    render(<ChangePasswordCard />);
    await typer.type(screen.getByLabelText('Current password'), 'old-password-1');
    await typer.type(screen.getByLabelText('New password'), 'Newpass12345');
    await typer.type(screen.getByLabelText('Confirm new password'), 'Different123');
    await typer.click(screen.getByRole('button', { name: 'Change password' }));
    expect(await screen.findByText('New passwords do not match')).toBeInTheDocument();
    expect(putMock).not.toHaveBeenCalled();
  });

  it('rejects a weak new password client-side', async () => {
    const typer = userEvent.setup();
    render(<ChangePasswordCard />);
    await typer.type(screen.getByLabelText('Current password'), 'old-password-1');
    await typer.type(screen.getByLabelText('New password'), 'short');
    await typer.type(screen.getByLabelText('Confirm new password'), 'short');
    await typer.click(screen.getByRole('button', { name: 'Change password' }));
    expect(await screen.findByText(/at least 10 characters/)).toBeInTheDocument();
    expect(putMock).not.toHaveBeenCalled();
  });

  it('submits, then logs out and navigates to /login', async () => {
    const typer = userEvent.setup();
    putMock.mockResolvedValue({ data: {} });
    render(<ChangePasswordCard />);
    await typer.type(screen.getByLabelText('Current password'), 'old-password-1');
    await typer.type(screen.getByLabelText('New password'), 'Newpass12345');
    await typer.type(screen.getByLabelText('Confirm new password'), 'Newpass12345');
    await typer.click(screen.getByRole('button', { name: 'Change password' }));
    await waitFor(() => expect(putMock).toHaveBeenCalledWith('/auth/change-password', {
      current_password: 'old-password-1',
      new_password: 'Newpass12345',
    }));
    await waitFor(() => expect(logoutMock).toHaveBeenCalled());
    expect(navigateMock).toHaveBeenCalledWith('/login');
  });

  it('surfaces the server error', async () => {
    const typer = userEvent.setup();
    putMock.mockRejectedValue({ response: { data: { error: 'Current password is incorrect' } } });
    render(<ChangePasswordCard />);
    await typer.type(screen.getByLabelText('Current password'), 'wrong');
    await typer.type(screen.getByLabelText('New password'), 'Newpass12345');
    await typer.type(screen.getByLabelText('Confirm new password'), 'Newpass12345');
    await typer.click(screen.getByRole('button', { name: 'Change password' }));
    expect(await screen.findByText('Current password is incorrect')).toBeInTheDocument();
    expect(logoutMock).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run to verify failure** — `bun run test -- ChangePasswordCard` → FAIL.

- [ ] **Step 3: Implement** — `frontend/src/components/ChangePasswordCard.tsx`:

```tsx
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { KeyRound } from 'lucide-react';
import api from '@/api/client';
import { useAuth } from '@/context/AuthContext';
import { validatePassword, PASSWORD_POLICY_HINT } from '@/lib/password';
import { getErrorMessage } from '@/lib/utils';

export function ChangePasswordCard() {
  const { logout } = useAuth();
  const navigate = useNavigate();
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (newPassword !== confirmPassword) {
      setError('New passwords do not match');
      return;
    }
    const policyError = validatePassword(newPassword);
    if (policyError) {
      setError(policyError);
      return;
    }

    setLoading(true);
    try {
      await api.put('/auth/change-password', {
        current_password: currentPassword,
        new_password: newPassword,
      });
      // The backend revokes every session on password change — including this
      // one — so there is nothing to stay signed in with.
      await logout();
      navigate('/login');
    } catch (err) {
      setError(getErrorMessage(err, 'Failed to change password'));
      setLoading(false);
    }
  };

  return (
    <section className="card">
      <div className="section-header">
        <div className="flex items-center gap-2">
          <KeyRound className="w-4 h-4 text-gray-400" />
          <span className="section-title">Password</span>
        </div>
      </div>

      <p className="text-sm text-gray-500 mb-4">
        Change the password you use to sign in. You will be signed out on all devices.
      </p>

      {error && (
        <div className="bg-red-50 text-red-600 text-sm px-4 py-3 rounded-xl mb-4">{error}</div>
      )}

      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label className="form-label" htmlFor="cp-current">Current password</label>
          <input
            id="cp-current"
            type="password"
            autoComplete="current-password"
            className="form-input"
            value={currentPassword}
            onChange={(e) => setCurrentPassword(e.target.value)}
            required
          />
        </div>
        <div>
          <label className="form-label" htmlFor="cp-new">New password</label>
          <input
            id="cp-new"
            type="password"
            autoComplete="new-password"
            className="form-input"
            value={newPassword}
            onChange={(e) => setNewPassword(e.target.value)}
            required
          />
          <p className="text-xs text-gray-400 mt-1">{PASSWORD_POLICY_HINT}</p>
        </div>
        <div>
          <label className="form-label" htmlFor="cp-confirm">Confirm new password</label>
          <input
            id="cp-confirm"
            type="password"
            autoComplete="new-password"
            className="form-input"
            value={confirmPassword}
            onChange={(e) => setConfirmPassword(e.target.value)}
            required
          />
        </div>
        <div className="flex justify-end">
          <button type="submit" disabled={loading} className="btn-primary">
            {loading ? 'Changing…' : 'Change password'}
          </button>
        </div>
      </form>
    </section>
  );
}
```

Labels above use `htmlFor`+`id` so `getByLabelText` works.

- [ ] **Step 4: Run tests** — `bun run test -- ChangePasswordCard` → PASS.

- [ ] **Step 5: Commit** — `feat(frontend): add change-password card`.

---

### Task 5: Frontend — restyle `LinkedAccounts`, `PasskeyManagement`, `TwoFactorSetup`

`PasskeyManagement` and `TwoFactorSetup` already use `.card`; `LinkedAccounts` and the old `SessionManagement` used `bg-white rounded-2xl shadow`. Bring all four to the same visual language and replace remaining `window.confirm`/`confirm()` calls with `Modal`.

**Files:**
- Modify: `frontend/src/components/LinkedAccounts.tsx`
- Modify: `frontend/src/components/PasskeyManagement.tsx`
- Modify: `frontend/src/components/TwoFactorSetup.tsx`

**Interfaces:** same exports, same component names, no prop changes — portal `MyProfile` keeps working untouched.

- [ ] **Step 1: `LinkedAccounts`** — replace the outer `<section className="bg-white rounded-2xl shadow p-6">` with `<section className="card">`; replace the hand-rolled header with `.section-header` + `Link2` icon + `.section-title` "Linked accounts" + a trailing `<span className="badge badge-cancelled ml-auto">{accounts.length} linked</span>` only when `accounts.length > 0` (otherwise `Not linked`-style gray badge can be omitted). Replace the `window.confirm` unlink with the same `Modal` pattern as `SessionManagement` (state `confirmingUnlink: boolean`; confirm button calls `unlink.mutate('google')`, destructive red styling).

- [ ] **Step 2: `PasskeyManagement`** — in the `.section-header`, add `<span className="badge badge-cancelled ml-auto">{passkeys?.length ?? 0}</span>` when loaded and non-empty. Replace `confirm('Delete this passkey?…')` with a `Modal` (state `deletingId: string | null`; confirm → `deleteMutation.mutate(deletingId)`).

- [ ] **Step 3: `TwoFactorSetup`** — in its `.section-header`, add a status badge driven by the existing `totpStatus` query: `status?.enabled ? <span className="badge badge-approved ml-auto">Enabled</span> : <span className="badge badge-cancelled ml-auto">Off</span>`.

- [ ] **Step 4: Verify** — `bun run test` (existing `SecurityComponents` + `Components` suites must still pass — `TwoFactorSetup`/`PasskeyManagement`/`LinkedAccounts` tests may assert on `confirm()`; update any that do to drive the Modal instead: click the destructive action → `findByRole('dialog')` → click confirm). `bun run lint`.

- [ ] **Step 5: Commit** — `feat(frontend): unify security cards on glass design system`.

---

### Task 6: Frontend — `SettingsPage` shell rewrite

**Files:**
- Rewrite: `frontend/src/pages/settings/SettingsPage.tsx`
- Create: `frontend/src/pages/settings/SettingsNav.tsx`
- Create: `frontend/src/pages/settings/CompanyCategorySection.tsx`
- Create: `frontend/src/pages/settings/SecuritySection.tsx`
- Test: `frontend/src/tests/SettingsPage.test.tsx` (new)

**Interfaces:**
- `SettingsNav({ sections, active, onSelect })` — `sections: { id: string; label: string; icon: LucideIcon; group: 'Workspace' | 'Account' }[]`
- `CompanyCategorySection({ category, settings, edits, onEdit, onSave, onDiscard, saving, saved })`
- `SecuritySection()` — stacks `ChangePasswordCard`, `TwoFactorSetup`, `PasskeyManagement`, `LinkedAccounts`
- `SessionsSection` is NOT a new file — the page renders `SessionManagement` directly inside the section shell.
- Active section in `?section=` (`useSearchParams` from `react-router`).

- [ ] **Step 1: `SettingsNav.tsx`**

```tsx
import { motion } from 'framer-motion';
import type { LucideIcon } from 'lucide-react';

export interface SettingsNavItem {
  id: string;
  label: string;
  icon: LucideIcon;
  group: 'Workspace' | 'Account';
}

const GROUPS = ['Workspace', 'Account'] as const;

export function SettingsNav({
  sections,
  active,
  onSelect,
}: {
  sections: SettingsNavItem[];
  active: string;
  onSelect: (id: string) => void;
}) {
  return (
    <nav aria-label="Settings sections" className="flex gap-1 overflow-x-auto lg:flex-col lg:gap-0.5 lg:sticky lg:top-6">
      {GROUPS.map((group) => {
        const items = sections.filter((s) => s.group === group);
        if (items.length === 0) return null;
        return (
          <div key={group} className="contents lg:block">
            <p className="hidden lg:block px-3 pt-4 pb-1.5 first:pt-0 text-[10px] font-semibold uppercase tracking-widest text-gray-400">
              {group}
            </p>
            {items.map((item) => {
              const isActive = item.id === active;
              return (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => onSelect(item.id)}
                  aria-current={isActive ? 'true' : undefined}
                  className={`relative flex items-center gap-2.5 px-3 py-2 rounded-xl text-sm whitespace-nowrap transition-all-fast ${
                    isActive ? 'text-gray-900 font-medium' : 'text-gray-500 hover:text-gray-800 hover:bg-black/[0.03]'
                  }`}
                >
                  {isActive && (
                    <motion.span
                      layoutId="settings-nav-active"
                      className="absolute inset-0 rounded-xl bg-[var(--accent-soft)] ring-1 ring-[var(--ring)]/10"
                      transition={{ type: 'spring', stiffness: 420, damping: 34 }}
                    />
                  )}
                  <item.icon className={`relative w-4 h-4 ${isActive ? 'text-[var(--accent-1)]' : 'text-gray-400'}`} />
                  <span className="relative">{item.label}</span>
                </button>
              );
            })}
          </div>
        );
      })}
    </nav>
  );
}
```

- [ ] **Step 2: `CompanyCategorySection.tsx`** — move `SettingField` verbatim from the current `SettingsPage.tsx` (all special-key branches preserved), wrapped as:

```tsx
export function CompanyCategorySection({
  title, description, settings, edits, onEdit, onSave, onDiscard, saving,
}: {
  title: string;
  description: string;
  settings: CompanySetting[];
  edits: Record<string, unknown>;
  onEdit: (setting: CompanySetting, value: unknown) => void;
  onSave: () => void;
  onDiscard: () => void;
  saving: boolean;
}) {
  const dirtyCount = settings.filter((s) => `${s.category}/${s.key}` in edits).length;
  return (
    <div className="card !p-0 overflow-hidden">
      <div className="p-6 pb-0">
        <h2 className="section-title">{title}</h2>
        <p className="text-sm text-gray-500 mt-1 mb-4">{description}</p>
      </div>
      <div className="px-6 divide-y divide-gray-100 [&>*]:py-4">
        {settings.map((s) => (
          <SettingField
            key={s.key}
            setting={s}
            value={`${s.category}/${s.key}` in edits ? edits[`${s.category}/${s.key}`] : s.value}
            onChange={(v) => onEdit(s, v)}
          />
        ))}
      </div>
      {dirtyCount > 0 && (
        <div className="sticky bottom-0 flex items-center justify-between gap-3 px-6 py-3.5 border-t border-gray-200/80 bg-white/85 backdrop-blur">
          <span className="text-sm text-gray-500">
            {dirtyCount} unsaved change{dirtyCount === 1 ? '' : 's'}
          </span>
          <div className="flex gap-2">
            <button type="button" className="btn-secondary !min-h-0 !py-2" onClick={onDiscard}>
              Discard
            </button>
            <button type="button" className="btn-primary !min-h-0 !py-2" onClick={onSave} disabled={saving}>
              <Save className="w-4 h-4" /> {saving ? 'Saving…' : 'Save changes'}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 3: `SecuritySection.tsx`**

```tsx
import { ChangePasswordCard } from '@/components/ChangePasswordCard';
import { TwoFactorSetup } from '@/components/TwoFactorSetup';
import { PasskeyManagement } from '@/components/PasskeyManagement';
import { LinkedAccounts } from '@/components/LinkedAccounts';

export function SecuritySection() {
  return (
    <div className="space-y-6">
      <ChangePasswordCard />
      <TwoFactorSetup />
      <PasskeyManagement />
      <LinkedAccounts />
    </div>
  );
}
```

- [ ] **Step 4: Rewrite `SettingsPage.tsx`** — keeps `getSettings`/`bulkUpdateSettings` query+mutation, `edits` keyed `category/key`, save scoped to the active category. New behavior: `useSearchParams` `section` param; section list = present categories (ordered `system, payroll, statutory, notifications`, labels General/Payroll/Statutory/Notifications, icons `SlidersHorizontal, Calculator, Landmark, Bell`, group Workspace) + `security` (`ShieldCheck`) + `sessions` (`MonitorSmartphone`) in group Account. Default `active` = first nav item; if `?section=` names a hidden/absent id, fall back to it. `onSelect` sets `setSearchParams({ section: id })` — normal push so the back button walks sections. Drop `canAccessPayrollData`/`useAuth` — the route guard already gates the page.

```tsx
import { useMemo, useState } from 'react';
import { useSearchParams } from 'react-router';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Bell, Calculator, Check, Landmark, MonitorSmartphone, ShieldCheck, SlidersHorizontal,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { getSettings, bulkUpdateSettings } from '@/api/settings';
import type { CompanySetting, SettingUpdate } from '@/types';
import { SettingsNav, type SettingsNavItem } from './SettingsNav';
import { CompanyCategorySection } from './CompanyCategorySection';
import { SecuritySection } from './SecuritySection';
import { SessionManagement } from '@/components/SessionManagement';

const SECTION_META: Record<string, { label: string; icon: LucideIcon; description: string }> = {
  system:        { label: 'General',       icon: SlidersHorizontal, description: 'Core workspace preferences' },
  payroll:       { label: 'Payroll',       icon: Calculator,        description: 'Payroll calculation and payslip defaults' },
  statutory:     { label: 'Statutory',     icon: Landmark,          description: 'Statutory contribution behavior' },
  notifications: { label: 'Notifications', icon: Bell,              description: 'Workspace notification defaults' },
  security:      { label: 'Security',      icon: ShieldCheck,       description: 'Your sign-in credentials and account protection' },
  sessions:      { label: 'Sessions',      icon: MonitorSmartphone, description: 'Devices signed in to your account' },
};
const WORKSPACE_ORDER = ['system', 'payroll', 'statutory', 'notifications'];
const ACCOUNT_SECTIONS = ['security', 'sessions'];

export function SettingsPage() {
  const queryClient = useQueryClient();
  const [searchParams, setSearchParams] = useSearchParams();
  const [edits, setEdits] = useState<Record<string, unknown>>({});
  const [saved, setSaved] = useState(false);

  const { data: settings, isLoading } = useQuery({ queryKey: ['settings'], queryFn: getSettings });

  const grouped = useMemo(() => {
    const map: Record<string, CompanySetting[]> = {};
    settings?.forEach((s) => { (map[s.category] ??= []).push(s); });
    return map;
  }, [settings]);

  const sections = useMemo<SettingsNavItem[]>(() => [
    ...WORKSPACE_ORDER.filter((c) => grouped[c]?.length).map((c) => ({
      id: c, label: SECTION_META[c].label, icon: SECTION_META[c].icon, group: 'Workspace' as const,
    })),
    ...ACCOUNT_SECTIONS.map((id) => ({
      id, label: SECTION_META[id].label, icon: SECTION_META[id].icon, group: 'Account' as const,
    })),
  ], [grouped]);

  const requested = searchParams.get('section');
  const active = sections.some((s) => s.id === requested) ? requested! : (sections[0]?.id ?? 'security');

  const mutation = useMutation({
    mutationFn: (updates: SettingUpdate[]) => bulkUpdateSettings(updates),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['settings'] });
      setEdits({});
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    },
  });

  const activeEdits = () =>
    Object.entries(edits)
      .filter(([key]) => key.startsWith(active + '/'))
      .map(([key, value]) => {
        const [category, settingKey] = key.split('/', 2);
        return { category, key: settingKey, value };
      });

  const handleSave = () => {
    const updates = activeEdits();
    if (updates.length > 0) mutation.mutate(updates);
  };

  const handleDiscard = () =>
    setEdits((prev) => Object.fromEntries(Object.entries(prev).filter(([k]) => !k.startsWith(active + '/'))));

  const isWorkspace = WORKSPACE_ORDER.includes(active);

  if (isLoading) {
    return <div className="flex items-center justify-center h-64"><div className="spinner" /></div>;
  }

  return (
    <div>
      <div className="page-header flex items-center justify-between mb-6">
        <div>
          <h1 className="page-title">Settings</h1>
          <p className="page-subtitle">Manage workspace and account preferences</p>
        </div>
        {saved && (
          <span className="flex items-center gap-1 text-sm text-green-600 font-medium">
            <Check className="w-4 h-4" /> Saved
          </span>
        )}
      </div>

      <div className="lg:grid lg:grid-cols-[220px_1fr] lg:gap-6 lg:items-start">
        <SettingsNav
          sections={sections}
          active={active}
          onSelect={(id) => setSearchParams({ section: id })}
        />

        <div key={active} className="mt-4 lg:mt-0 animate-fade-up">
          {isWorkspace ? (
            <CompanyCategorySection
              title={SECTION_META[active].label}
              description={SECTION_META[active].description}
              settings={grouped[active] ?? []}
              edits={edits}
              onEdit={(s, v) => setEdits((prev) => ({ ...prev, [`${s.category}/${s.key}`]: v }))}
              onSave={handleSave}
              onDiscard={handleDiscard}
              saving={mutation.isPending}
            />
          ) : (
            <>
              <h2 className="section-title">{SECTION_META[active].label}</h2>
              <p className="text-sm text-gray-500 mt-1 mb-4">{SECTION_META[active].description}</p>
              {active === 'security' ? <SecuritySection /> : <SessionManagement />}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
```

`SettingField` moves unchanged into `CompanyCategorySection.tsx` (Task 6 Step 2 shows the wrapper; paste the existing function below it).

- [ ] **Step 5: `SettingsPage.test.tsx`** — mock `@/api/settings` + the four card components as stubs; assert: nav renders General/Security/Sessions; clicking "Sessions" sets `?section=sessions` and shows the session stub; company section hides save bar until an edit; save calls `bulkUpdateSettings` with the active category's edits only.

- [ ] **Step 6: Run** — `bun run test`, `bun run lint`, `bun run typecheck`, `bun run build`.

- [ ] **Step 7: Commit** — `feat(frontend): rebuild settings page as grouped nav hub`.

---

### Task 7: Final verification + codegraph

- [ ] `cd backend && cargo fmt --check && cargo clippy -- -D warnings && cargo build --tests`
- [ ] `cd frontend && bun run lint && bun run typecheck && bun run test && bun run build`
- [ ] If dev DB available: `cargo test` session/auth subset.
- [ ] `./scripts/codegraph update .` (repo rule after code changes).
- [ ] Manual smoke (if DB + servers available): `bun run dev` + `cargo run`; open `/settings?section=sessions`; sign in from a second browser → see both sessions, IP shown; revoke one → other browser loses refresh.
