# Four-Locale i18n Implementation Plan

> **For agentic workers:** executed inline. Steps tracked via session todos.

**Goal:** Full en / ms / zh-CN / zh-TW support for the payroll frontend, with
automated regression gates, and a documented boundary for backend strings.

**Architecture:** `i18next` + `react-i18next` +
`i18next-browser-languagedetector` (the standard React stack — no hand-rolled
i18n). Locale resources are TypeScript modules under `src/i18n/locales/`
(`en.ts`, `ms.ts`, `zh-CN.ts`, `zh-TW.ts`); `en.ts` defines the `Messages`
type and the other three are annotated `Messages`, so `tsc -b` (already in CI)
fails on missing/extra keys for free. Single default `translation` namespace
with semantic nested keys (`common.save`, `payroll.runs.title`).

**Tech stack:** i18next, react-i18next, i18next-browser-languagedetector,
Intl.* formatters, vitest.

## Global constraints

- Locales: `en`, `ms`, `zh-CN`, `zh-TW`. Fallback `en`.
- Persistence: `localStorage` key `payroll.lang` via LanguageDetector
  (`order: ['localStorage', 'navigator']`), `convertDetectedLanguage`
  normalizes `zh-Hans/*`→`zh-CN`, `zh-Hant/HK/MO`→`zh-TW`, `ms*`→`ms`.
- Interpolation `{{var}}`; plurals via i18next `_one`/`_other` suffixes.
- No language-specific key suffixes (`foo.en` is forbidden).
- Currency stays MYR; only grouping/digits localize.
- Money Decimal/serde contracts untouched; backend schema untouched.
- Existing tests must keep passing (`bun run test`, `lint`, `typecheck`).

## Key taxonomy (top-level groups)

`common` (actions/states shared everywhere), `nav`, `a11y`, `auth`,
`validation`, `errors` (pages + API mapping), `status` (attendance/payroll/
leave/claims/ot/approval enums rendered to users), `roles`, `time`
(relative/relative ranges/months/weekdays), then one group per feature:
`company`, `employees`, `payroll`, `attendance`, `kiosk`, `approvals`,
`reports`, `documents`, `letters`, `settings`, `admin`, `audit`, `backup`,
`calendar`, `teams`, `portal` (sub-groups `portal.leave|claims|overtime|
payslips|profile|notifications|teamCalendar|myAttendance`), `serverErrors`
(backend message map).

## Work order

1. **Deps + skeleton** — `bun add i18next react-i18next
   i18next-browser-languagedetector`; `src/i18n/index.ts`; `locales/en.ts`
   seed; `main.tsx` imports `@/i18n`; `LanguageSwitcher` ui component;
   mount in `Sidebar` footer, `PortalLayout` header, `Login` footer.
2. **format.ts** — `formatDate`, `formatDateTime`, `formatTime`,
   `formatNumber`, `formatMYR`, `formatRelativeTime` reading
   `i18n.language`→Intl tag (`en`→`en-MY`, `ms`→`ms-MY`, `zh-CN`, `zh-TW`).
   Replace the 31 `toLocale*` call sites + `formatRelativeTime`.
3. **getErrorMessage** — accepts a `t`-produced fallback; add
   `localizeApiError(err, t)` that maps `serverErrors.<hash>` when the
   backend `error` string is a known literal, else raw server text.
4. **Convert files** (~60 .tsx + libs) module by module: core shell → auth →
   employees/payroll/attendance → portal → admin/settings/reports/etc.
   Each file: `useTranslation()`, replace literals with `t()`, grow `en.ts`.
5. **Translate** en→ms, zh-CN, zh-TW in the locale files directly.
6. **Backend audit** — script-extract `AppError::*("…")` literals →
   `serverErrors` catalog (slug → english); translate. Email/notification
   templates: document as follow-up (server-rendered, needs user-locale
   plumbing — out of scope for a frontend pass).
7. **Gates** — `src/tests/i18n.test.ts`: key parity, no empties,
   interpolation-var parity, zh script purity (deny-lists), en residue in
   ms. `scripts/i18n-scan.mjs`: hardcoded-string detector over src/
   (JSXText, placeholder/title/aria-label/alt literals, getErrorMessage
   fallbacks); `bun run i18n:check` in CI frontend-lint job.
8. **UI/UX** — `min-w-0`, `truncate`, `whitespace-nowrap` on buttons where
   translations lengthen text; sidebar/nav reviewed for ms/zh lengths.
9. **Docs** — `docs/i18n/README.md` (workflow), `docs/i18n/terminology.md`
   (4-locale dictionary), `docs/i18n/audit-2026-09-16.md` (report),
   AGENTS.md/CLAUDE.md i18n section.
10. **Verify** — lint, typecheck, test, build; spot-check language switching
    keeps route + form state (i18n changeLanguage remounts nothing).

## Audit findings (pre-implementation)

- **Existing i18n:** none. No library, no locale files, no `t()` calls.
- **Frontend surface:** 154 source files, ~24k LOC; ~640 JSX text nodes,
  ~180 user-facing attribute literals, plus alert/toast/validation strings
  in page logic and `src/lib/*`.
- **Backend surface:** ~593 `AppError` message literals reach the client via
  `response.data.error` (surfaced through `getErrorMessage`). Server-rendered
  emails/notifications exist in `repositories/email_templates.rs` +
  `services/` notification calls — flagged as out-of-scope follow-up (needs
  per-user locale stored server-side).
- **Hardcoded formats:** 31 `toLocale*`/`Intl.*` sites pinned to `en-MY`/
  `en-GB`/`en-CA`; `formatRelativeTime` hardcodes English words.
- **Non-user strings (correctly left alone):** role/permission keys, status
  enums on the wire, CSS classes, URLs, `console.*`, CSV headers for
  statutory exports (machine-consumed formats stay English).
