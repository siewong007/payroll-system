# Full UI i18n Coverage — Design Spec

Date: 2026-09-15
Status: Approved (verbal design review)

## Goal

Every user-visible label, message, and informational string in the frontend
(`frontend/src`, ~96 TSX files, ~28k lines) renders through a central i18n
system with complete `en`, `ms`, and `zh-CN` translations. No hardcoded
user-facing strings remain; missing keys and regressions are caught by tests.

There is no existing i18n implementation — this is a from-scratch build, not a
completion task.

## Locked decisions

| Decision | Choice |
|---|---|
| Locales | `en` (fallback), `ms` (Bahasa Melayu), `zh-CN` (简体中文) |
| Library | `react-i18next` + `i18next` + `i18next-browser-languagedetector` |
| Backend messages | Frontend message→key map at the `getErrorMessage` funnel; unmapped → generic translated message (raw logged to console, never shown in non-English UI) |
| Locale selection | `localStorage` persistence via language detector (`localStorage → navigator`), manual `LanguageSwitcher` in `AppLayout` header, `PortalLayout`, and the public `Login` page |
| Money/dates | `RM` prefix and `en-GB` DD/MM/YYYY date format stay (MY convention, locale-independent). Only *labels* translate, not formats. |

## Architecture

### File layout

```
frontend/src/i18n/
  index.ts               # i18next singleton init, detector, resources, lang/title side effects
  resources.ts           # explicit imports of all locale JSON; exports `resources` + `Locales` type
  locales/
    en/<ns>.json         # source of truth for key shape
    ms/<ns>.json
    zh-CN/<ns>.json
frontend/src/components/LanguageSwitcher.tsx
frontend/src/lib/apiErrorMessages.ts   # backend error string → errors:api.* key map
```

Locale JSON is imported eagerly and bundled (hundreds of keys — no lazy
loading, no suspense gap). Explicit imports keep `resources` statically typed.

### Type safety

`index.ts` augments `i18next.CustomTypeOptions` with `resources` derived from
the `en` locale objects. `t('bad.key')` and missing `en` entries become
`tsc -b` errors. Cross-locale parity is a runtime property and is enforced by a
test (below), not the type system.

### Namespaces

One JSON file per namespace per locale. Namespace = feature area; `common`
holds globally reused concepts only.

| Namespace | Covers |
|---|---|
| `common` | Global actions (save/cancel/delete/edit/close/search/back/submit/confirm/add/remove/view/download/upload/print/export/import/refresh/retry), states (loading, saving, active/inactive, enabled/disabled, required/optional, yes/no, all/none, unknown, N/A), pagination ("Showing X–Y of Z", prev/next), table chrome (actions column, select row/all, "No data found", "Record Details"), relative time units, role display names, day/month names if any are hand-built, "Error N" prefix |
| `nav` | Sidebar items, section labels (Workspace/Me/Administration), portal nav, sign out, company switcher strings, layout chrome |
| `auth` | Login, code login, forgot/reset/change password, TOTP setup/verify, passkeys, OAuth link/callback, session mgmt, security section |
| `company` | Company profile page |
| `employees` | List/create/detail/import + EmployeePicker + employee field labels |
| `payroll` | Overview/runs/process/detail/preview/drawer, payslip labels, statutory exports, payroll status labels |
| `attendance` | Admin attendance page, work schedule, geofence, network cards, QR, kiosk public + scan pages |
| `approvals` | Approvals inbox |
| `calendar` | Calendar page, team calendar shared strings |
| `reports` | Reports page, chart/metric labels, export UI |
| `documents` | Document list/upload |
| `letters` | Email/letters log |
| `settings` | Settings nav/page, company categories |
| `admin` | Users/roles/groups/companies management, attendance settings, audit trail, backup |
| `portal` | Self-service pages: profile, payslips, leave, claims, overtime, team calendar, notifications, my attendance |
| `errors` | 403/404 pages, ErrorBoundary fallback, `errors.api.*` backend-message map, generic error |
| `validation` | Shared form validation messages (required field, invalid email, min/max length, password rules, date/amount validation) |

A new file/component maps to the namespace of its feature area. Shared
concepts go to `common`/`validation`/`errors`, never duplicated per feature.

### Key conventions

- `t('ns:key.subkey')`; keys are `camelCase` paths nested by meaning:
  `payroll:runs.table.status`, `auth:login.submit`, `common:actions.save`.
- Interpolation: `t('common:pagination.showing', { from, to, total })`.
- Plurals via i18next `_one`/`_other` (relative time: `common:time.minAgo`).
- Identical wording + identical meaning ⇒ reuse `common:*`. Feature-specific
  terms stay in their namespace even if the English happens to match.
- Never compose sentences by concatenating translated fragments — use one key
  with interpolation.
- Enum-like backend values (statuses, roles, leave types, claim statuses)
  translate through `<ns>:status.<value>` / `common:roles.<value>` maps;
  lookup falls back to the raw value so new enum members never crash.

### Integration points

- `main.tsx`: `import './i18n'` before `createRoot` — init is synchronous
  (bundled resources), so no suspense for translations themselves.
- `index.ts` side effects: `document.documentElement.lang = lng` and
  `document.title = t('common:app.title')` on init and `languageChanged`.
- `LanguageSwitcher`: compact dropdown (EN | Melayu | 中文) using
  `i18n.changeLanguage`; mounted in `AppLayout` header, `PortalLayout`,
  and `Login`.
- `getErrorMessage(err, fallback?)` in `lib/utils.ts`: routes the extracted
  backend message through `apiErrorMessages.ts` → `errors:api.*` key via the
  i18next singleton (`i18n.t`, safe outside React). Mapped ⇒ translated;
  unmapped ⇒ `errors:api.unexpected` + `console.warn(raw)`. Call sites that
  pass English `fallback` strings are converted to pass keys instead.
- `describeRateLimit` in `api/client.ts`: uses `i18n.t` with interpolation
  for `retryAfter` seconds (pluralized).
- `formatRelativeTime` in `lib/utils.ts`: pluralized `common:time.*` keys.
- Label maps (`statusMeta.ts`, role names via `roleList(...)`, any
  `STATUS_LABELS`-style records): values become translation keys resolved at
  render (`t(map[value] ?? value)`).
- Turnstile widget: pass `i18n.language` to its `hl` param if supported;
  its internals are an intentional exception.

### Reactivity rule

Components rendering text use `useTranslation(ns)` (re-renders on language
change). Non-render code (catch blocks, interceptors, pure helpers) may call
the `i18n` singleton's `t()` directly — the result is computed at call time.
Helpers called *during* render (`formatRelativeTime`, label maps) are invoked
inside components that already hold `useTranslation`, so they update on switch.

## Testing / enforcement

1. **`tests/i18nParity.test.ts`** — walks the `en` resource tree; asserts every
   key exists and is a non-empty string in `ms` and `zh-CN`, and that neither
   has extra keys. Fails the build on drift.
2. **`tests/i18nHardcoded.test.ts`** — scans every `*.tsx` under `src/`
   (excluding `tests/`, `i18n/`) for: literal JSX text nodes
   (`>letters/digits…<`), and string-literal `placeholder`/`title`/
   `aria-label`/`alt` attributes. Findings must be empty or match an explicit
   `allowlist` in the test file (`RM`, `…`, `N/A`, `ERROR`, single chars,
   `{`-expressions). This is the "no hardcoded strings" guardrail.
3. **`tests/setup.ts`** — imports `@/i18n` and forces `i18n.changeLanguage('en')`
   so existing tests asserting English text keep passing.
4. i18next `missingKeyHandler` → `console.warn` in dev builds.
5. Per-task gate: `bun run typecheck && bun run lint && bun run test` in
   `frontend/` before each commit.

## UX requirements

- Translations reviewed for consistent terminology (e.g. "payslip" =
  "slip gaji" / "工资单" everywhere), consistent capitalization, and MY HR
  vocabulary.
- `ms` strings run ~20–30% longer than `en`: audit sidebar width, table
  headers, modal footers, buttons, and pagination after implementation; fix
  layout (wrap/truncate-with-tooltip/wider columns) rather than abbreviating
  translations.
- Language switch updates the entire visible tree without reload.

## Intentional exceptions (audited, allowlisted)

- DB/user content: employee names, company names, letter subjects/bodies,
  notes, uploaded file names, audit-log payloads.
- `RM` currency prefix, `…`, `N/A` where used as a data glyph, page `ERROR N`
  code line, brand name "PayrollMY".
- Third-party internals: Turnstile widget (own i18n via `hl`), `html5-qrcode`
  scanner chrome, WebAuthn browser dialogs.
- `console.*`, dev logging, `document.title` format string internals.
- `<html lang>` in `index.html` stays `en` statically; runtime syncs it.

## Deliverables (final audit)

`docs/i18n-inventory.md` generated at completion: full key inventory, key →
files map, locale coverage matrix (auto-verified by parity test), consolidated
duplicates list, exceptions list, and UI issues found/fixed during the
expansion review.

## Out of scope

- Translating `backend/` Rust error catalog or emails server-side (frontend
  map covers what reaches the UI).
- Per-user locale preference stored in DB (localStorage only).
- Statutory export file content (EPF/SOCSO/EIS/PCB file formats are
  regulator-defined, produced by the backend).
