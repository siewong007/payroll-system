# Settings Page Redesign + Session Control — Design

Date: 2026-09-12
Scope: admin `/settings` page only. Approved approach: **nav shell + shared components restyled onto the glass design system**.

## Problem

`/settings` is one long scroll: company-setting category tabs (payroll / statutory / system / notifications) on top, then four unrelated personal-security cards (`TwoFactorSetup`, `PasskeyManagement`, `LinkedAccounts`, `SessionManagement`) appended below. It does not use the app's `.card` / `.btn-primary` design system (`index.css`), and session management — which already exists end-to-end — is buried at the bottom with a minimal UI (generic laptop icon, crude UA sniffing, `window.confirm`, absolute timestamps only).

The `user_sessions.ip_address inet` column (migration `1003`) exists but is never written or read.

## Information architecture

In-page grouped navigation (desktop: sticky left column; mobile: horizontal pill strip). Active section syncs to `?section=` via `useSearchParams` — deep-linkable, back-button works, no route changes (the page stays gated by `PermissionGuard requires="manage_company_settings"`).

```
Settings
├── Workspace
│   ├── General        (category "system", relabeled)
│   ├── Payroll        (category "payroll")
│   ├── Statutory      (category "statutory")
│   └── Notifications  (category "notifications")
└── Account
    ├── Security       → Password · 2FA · Passkeys · Linked accounts
    └── Sessions       → device list, revoke one / all others
```

- Default section: first workspace category that has settings (unchanged source of truth: `CATEGORY_ORDER` filtered by what the API returns).
- Workspace categories render only when the API returns rows for them (current behavior preserved). `exec`/payroll-blind users already never reach this page; no new gating is introduced.
- Nav item: lucide icon + label. Active item gets an accent-soft pill with a gradient indicator bar (same grammar as `Sidebar.tsx`, framer-motion `layoutId` for the indicator). Group labels in uppercase micro type, matching the sidebar's `Workspace` / `Me` / `Administration` pattern.

## Layout

- Header uses existing `.page-header` / `.page-title` / `.page-subtitle` conventions: "Settings" + "Manage workspace and account preferences".
- Desktop: `lg:grid lg:grid-cols-[240px_1fr]`; nav sticky under the app top bar.
- Mobile (< lg): nav collapses to a horizontal scrollable pill strip above the content (same pattern the current tab strip already uses).
- Content panel: `.card` glass surface, `animate-fade-up` on section switch. Each section = title + one-line description + body.
- Company-setting sections reuse the existing `SettingField` renderer, rows separated by `divide-y`. The always-visible footer is replaced by a sticky save bar *inside* the panel that appears only when the visible section has pending edits: "N unsaved changes" · Discard · "Save changes" (`.btn-primary`). Save behavior unchanged: `bulkUpdateSettings` with edits scoped to the active category; success shows the inline "Saved" confirmation.

## Sessions (headline feature)

The backend already implements `GET /auth/sessions`, `DELETE /auth/sessions/{id}` (refuses the current session), and `DELETE /auth/sessions/others`. This spec upgrades data richness and UX.

### Backend — populate `ip_address`

- `issue_session` (`services/auth_service.rs`) already receives `audit_meta: Option<&AuditRequestMeta>` whose `ip_address: Option<String>` is resolved by `core::client_ip` — no signature change needed there. Extract `audit_meta.and_then(|m| m.ip_address.as_deref())` and pass it down.
- `session_service::create_session` / `create_refresh_token` gain `ip_address: Option<&str>`; `user_sessions::insert` binds it (`$n::inet` cast in SQL or a parsed `IpAddr` bind — implementation detail).
- `user_sessions::touch` (called on refresh rotation) also updates `ip_address` so it reflects the most recent ingress address.
- `user_sessions::list_active` selects `ip_address`; `UserSession` (FromRow) gets `ip_address: Option<IpAddr>`; `UserSessionResponse` serializes it as a string.
- Keep the module's runtime `sqlx::query` style — it has not been macro-migrated, so no `.sqlx` cache regen is needed.
- All `create_session` call sites updated: `issue_session` + the test helpers in `totp_route_tests.rs`, `route_auth_tests.rs`, `session_rotation_tests.rs`, `auth_audit_tests.rs` (pass `None`).

### Frontend — `SessionManagement` rewrite

- New `frontend/src/lib/userAgent.ts`: hand-rolled parser (no dependency) returning `{ deviceType: 'desktop'|'mobile'|'tablet'|'unknown', os: string, browser: string }`. Covers Chrome/Edge/Firefox/Safari and macOS/Windows/iOS/Android/Linux; falls back to "Browser session"/"Unknown device". Unit-tested.
- Row layout: device-type icon (`Monitor` / `Smartphone` / `Tablet` / `Globe`), title "{Browser} on {OS}", "This device" badge (accent-soft pill + `glow-dot`), meta line `IP · Signed in {date} · {relative last active}` ("Active now" under ~2 min).
- New `formatRelativeTime` helper in `lib/utils.ts`.
- Per-row revoke icon button (non-current sessions only, unchanged rule). "Sign out all other sessions" button shows the count and confirms through `components/ui/Modal.tsx` — `window.confirm` is removed.
- Skeleton rows while loading; friendly empty state ("No other active sessions").

## Security section

Four `.card` blocks, each with an icon header and a status badge where meaningful:

1. **Password** — new `frontend/src/components/ChangePasswordCard.tsx`: current / new / confirm fields, min-10-chars rule (matches `ChangePassword.tsx`), submits to the existing `PUT /auth/change-password`. Copy warns that changing the password signs out all devices (`revoke_all_for_user` on credential change is existing backend behavior). On success the card shows a confirmation; the in-memory token stays valid until expiry — no forced reload (verify actual endpoint behavior during implementation; if it clears the session, redirect to `/login` instead).
2. **Two-factor authentication** — `TwoFactorSetup`, restyled to `.card` + status badge (Enabled / Off).
3. **Passkeys** — `PasskeyManagement`, restyled, badge shows registered count.
4. **Linked accounts** — `LinkedAccounts`, restyled, badge shows linked providers.

The four shared components stay in `frontend/src/components/` and keep their APIs — the portal `MyProfile` reuses them, so the restyle improves it for free. Portal page structure itself is out of scope.

## Files

```
frontend/src/pages/settings/
  SettingsPage.tsx                        — shell: nav + section router + save logic (rewrite)
  SettingsNav.tsx                         — grouped in-page nav (new)
  sections/CompanyCategorySection.tsx     — SettingField logic moved here (new)
  sections/SecuritySection.tsx            — new
  sections/SessionsSection.tsx            — new (wraps rewritten SessionManagement)
frontend/src/lib/userAgent.ts             — new
frontend/src/lib/utils.ts                 — + formatRelativeTime
frontend/src/components/ChangePasswordCard.tsx — new
frontend/src/components/{TwoFactorSetup,PasskeyManagement,LinkedAccounts,SessionManagement}.tsx — restyle
frontend/src/types/ (sessions type)       — + ip_address
backend/src/services/session_service.rs   — ip_address params
backend/src/services/auth_service.rs      — pass audit_meta ip into create_session
backend/src/repositories/user_sessions.rs — insert/touch/list_active + ip_address
backend/src/models/session.rs             — UserSession/UserSessionResponse + ip_address
```

## Testing & verification

- Frontend: update `SecurityComponents.test.tsx` for the new session markup; unit-test `userAgent` parser and `formatRelativeTime`; run `bun run lint`, `bun run test`, `bun run typecheck`, `bun run build`.
- Backend: update the four test files' `create_session` call sites; `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo build`. DB-backed `cargo test` only if a migrated database is available locally.
- Types contract: `frontend/src/types` updated to match the `ip_address` response field (per CONTRIBUTING).

## Out of scope

- Employee portal page restructuring (shared component restyle still benefits it).
- GeoIP location display (no GeoIP database; IP shown raw).
- New features below — proposals only.

## Future feature proposals (not in this round)

- **Security activity feed** — per-user read of the existing audit rows (logins, password/2FA changes). Needs a company-scoped `GET /auth/security-activity` endpoint + card in Security.
- **New-device login alerts** — email via `notification_service` when `issue_session` creates a session for an unrecognized user-agent/IP.
- **Session nicknames** — let users label a device ("Office Mac"); `user_sessions.label` + edit UI.
- **Per-user notification preferences** — email/push toggles for payslips, approvals, leave updates.
- **Appearance preference** — light/dark/system (theme vars already exist per shell).
