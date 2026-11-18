# Refactor Progress

## Final Summary

### What Changed (Phases 0–6)

- **Phase 0 (Audit)**: Mapped the full stack, frontend routes, Axum `/api` endpoints, PostgreSQL schema/migrations, background jobs, RBAC roles/groups, severity-grouped findings, and the Phase 5 checklist in `docs/refactor/AUDIT.md`.
- **Phase 1 (Tooling & Standards)**: Added `.editorconfig`, `.prettierrc.json`, `.prettierignore`, `.bun-version` (`1.3.14`), `rust-toolchain.toml` (`stable` + `rustfmt`/`clippy`), Husky v9 + `.lintstagedrc.json` monorepo pre-commit hooks, and unified root `package.json` scripts (`lint`, `format`, `format:check`, `typecheck`, `test`, `build`); removed unused ESLint prettier packages; eliminated remaining `any` casts and unnecessary `eslint-disable` comments in `frontend/src/lib/webauthn.ts` and `frontend/src/pages/portal/TeamCalendar.tsx`.
- **Phase 2 (Structure & Naming)**: Moved all remaining inline SQL out of `backend/src/main.rs`, `services/auth_service.rs`, and `services/statutory_rules.rs` into `repositories/` (`refresh_tokens::purge_stale`, `user_groups::user_has_any_active_permission`, `users::list_two_factor_non_compliant_emails`, `statutory_rule_sets::pcb_status_and_rule_verified`); replaced magic retention literals in `main.rs` with named constants; regenerated the offline cache in `backend/.sqlx/`; and centralized SPA route constants in `frontend/src/lib/routes.ts` (`ROUTES`) wired into `frontend/src/App.tsx`.
- **Phase 3 (Code Quality & Modularity)**: Added characterization tests (`frontend/src/tests/PageModularization.test.tsx`); split four oversized (1,100–1,460 LOC) page modules — `Approvals.tsx` (`ApprovalShared.tsx`, `approvalRules.ts`, `LeaveCrudModal.tsx`, `ClaimCrudModal.tsx`, `OvertimeCrudModal.tsx`), `AttendancePage.tsx` (`attendanceHelpers.ts`, `KioskCredentialsModal.tsx`, `QrPanel.tsx`, `AttendanceSummaryTab.tsx`, `AttendanceRecordModals.tsx`), `EmployeeList.tsx` (`EmployeeProfileDrawer.tsx`, `EmployeeFormModal.tsx`), and `Reports.tsx` (`ReportShared.tsx`, `leaveReportHelpers.ts`, `LeaveEntitlementReport.tsx`, `ClaimsSummaryReport.tsx`, `EaFormTab.tsx`); and moved welcome-email company lookup and async dispatch out of `backend/src/handlers/employee.rs` into `backend/src/services/employee_service.rs`.
- **Phase 4 (Security, Privacy & Config)**: Hardened `AppConfig::validate_production_secrets` against `.env.example` placeholders, fixed multi-byte email redaction and redacted recipient emails from `email_service.rs` `tracing` logs, and documented `RUST_ENV`, `LOG_FORMAT`, and `VITE_TURNSTILE_SITE_KEY` in `.env.example`.
- **Phase 5 (UX & Accessibility Gaps)**: Added loading skeletons and error/retry states in `TeamCalendar.tsx`, added global `OfflineBanner` across all 4 locales (`en`, `ms`, `zh-CN`, `zh-TW`), added keyboard skip-to-content (`#main-content`) links and 44px (`min-h-11 min-w-11`) mobile header touch targets in `AppLayout` and `PortalLayout`, added `<meta name="description">`, `theme-color`, and `apple-touch-icon` in `frontend/index.html`, and expanded `frontend/src/tests/TeamCalendarAndOffline.test.tsx`.
- **Phase 6 (Final Verification & Docs)**: Updated `README.md`, `CLAUDE.md`, and `AGENTS.md` to match the root workspace scripts, runtime pins, and new helper modules; re-synced CodeGraph (`graphify-out/`); and verified clean install, formatting, linting, SQLx offline cache, typecheck, full test suites, and production builds on branch `refactor/standards`.

### Bug Fixes (`fix:` Commits & Pre-Refactor Fixes)

- `35a6c86`: Fixed `password_reset_requests` retention column (`requested_at` instead of non-existent `created_at`) and `background_jobs` retention terminal status (`'succeeded'` instead of `'completed'`) while moving cleanup SQL into `repositories/`.
- `dead8a7` (`fix(security)`): Rejected `"replace-with"` `.env.example` placeholder secrets in `AppConfig::validate_production_secrets` (+ `production_secrets_reject_env_example_placeholders` unit test).
- `23f1d76` (`fix(privacy)`): Fixed `core::redact::email` multi-byte single-character local-part check (`chars().nth(1).is_none()`, + `masks_multibyte_single_char_local` test) and redacted `recipient_email` in `services/email_service.rs` `tracing` logs.
- `6317c27` (`fix(portal)`): Added missing loading skeleton and error/retry state handling in `frontend/src/pages/portal/TeamCalendar.tsx` (+ 4-locale i18n keys and `TeamCalendarAndOffline.test.tsx`).

### Items Waiting on User Decision

- External error-monitoring sink (e.g., Sentry DSN or OTLP collector endpoint) — structured JSON logging (`LOG_FORMAT=json`) and PII redaction are already in place.
- Importing independently verified official 2026 KWSP/PERKESO statutory schedules and enabling automatic LHDN computerised-MTD PCB calculation after official vector conformance.
- Adding a browser E2E suite (e.g., Playwright) alongside the existing Rust integration and frontend Vitest suites.
- Any database schema/migration additions, authentication/permission policy modifications, or major-version dependency upgrades (including upstream `vite`/`vitest` once `picomatch > 4.0.7` ships for advisory `GHSA-fc5q-39xc-vjpf`).

### Recommended Next Steps

1. Review the commit series on `refactor/standards` (`git log main..refactor/standards`).
2. Decide on the optional product/infrastructure items above.
3. Merge or open a PR from `refactor/standards` into `main` when ready.

## Phases

- [x] Phase 0: Understand (Done)
- [x] Phase 1: Tooling and standards
- [x] Phase 2: Structure and naming
- [x] Phase 3: Code quality
- [x] Phase 4: Security, privacy and performance
- [x] Phase 5: Fill the gaps
- [x] Phase 6: Final check

## Log

- 2026-11-18: Initialized project understanding and created `docs/refactor/AUDIT.md`.
- 2026-11-18: Completed Phase 1 (Tooling and standards): configured `.editorconfig`, `lint-staged`, `husky`, and updated `.gitignore`.
- 2026-11-18: (`chore`) Hardened Phase 1 tooling: untracked `.lintstagedrc.json` from `.gitignore`, updated `.dockerignore`, pinned `.bun-version` (`1.3.14`) and `rust-toolchain.toml`, added `.prettierrc.json`/`.prettierignore`, added unified root scripts (`lint`, `format`, `format:check`, `typecheck`, `test`, `build`), removed unused prettier eslint packages, and fixed `.husky/pre-commit` + `.lintstagedrc.json`.
- 2026-11-18: (`refactor`) Removed remaining `any` casts and `eslint-disable` suppressions in `frontend/src/lib/webauthn.ts` and `frontend/src/pages/portal/TeamCalendar.tsx`.
- 2026-11-18: (`docs`) Completed full-repository audit, severity inventory, and Phase 5 checklist in `docs/refactor/AUDIT.md`.
- 2026-11-18: (`refactor`) Completed Phase 2 (Structure and naming): moved remaining SQL out of `main.rs`, `services/auth_service.rs`, and `services/statutory_rules.rs` into `repositories/` (`refresh_tokens::purge_stale`, `user_groups::user_has_any_active_permission`, `users::list_two_factor_non_compliant_emails`, `statutory_rule_sets::pcb_status_and_rule_verified`), named retention constants in `main.rs`, refreshed `backend/.sqlx/`, and centralized route constants in `frontend/src/lib/routes.ts`.
- 2026-11-18: (`test`) Added characterization tests in `frontend/src/tests/PageModularization.test.tsx` for `Approvals`, `Reports`, and `EmployeeList` before splitting oversized page modules.
- 2026-11-18: (`refactor`) Completed Phase 3 (Code quality): split `Approvals.tsx` (`ApprovalShared.tsx`, `approvalRules.ts`, `LeaveCrudModal.tsx`, `ClaimCrudModal.tsx`, `OvertimeCrudModal.tsx`), `AttendancePage.tsx` (`attendanceHelpers.ts`, `KioskCredentialsModal.tsx`, `QrPanel.tsx`, `AttendanceSummaryTab.tsx`, `AttendanceRecordModals.tsx`), `EmployeeList.tsx` (`EmployeeProfileDrawer.tsx`, `EmployeeFormModal.tsx`), and `Reports.tsx` (`ReportShared.tsx`, `leaveReportHelpers.ts`, `LeaveEntitlementReport.tsx`, `ClaimsSummaryReport.tsx`, `EaFormTab.tsx`); extracted welcome-email dispatch from `backend/src/handlers/employee.rs` into `backend/src/services/employee_service.rs`.
- 2026-11-18: (`fix`) Fixed `AppConfig::validate_production_secrets` in `backend/src/core/config.rs` to reject `"replace-with"` `.env.example` placeholder secrets and added `production_secrets_reject_env_example_placeholders` unit test.
- 2026-11-18: (`fix`) Fixed `core::redact::email` multi-byte single-char masking (`chars().nth(1).is_none()`) and redacted `recipient_email` in `services/email_service.rs` `tracing` logs.
- 2026-11-18: (`chore`) Completed Phase 4 (Security, privacy and performance): documented `RUST_ENV`, `LOG_FORMAT`, and `VITE_TURNSTILE_SITE_KEY` in `.env.example` with placeholders and descriptions.
- 2026-11-18: (`fix`) Fixed missing loading and error/retry states in `frontend/src/pages/portal/TeamCalendar.tsx` (with 4-locale i18n keys and unit tests in `frontend/src/tests/TeamCalendarAndOffline.test.tsx`).
- 2026-11-18: (`feat`) Completed Phase 5 (Fill the gaps): added `OfflineBanner` component and mounted in `App.tsx`, added keyboard skip-to-content links (`#main-content`) and `min-h-11 min-w-11` mobile header touch targets in `AppLayout`/`PortalLayout`, added HTML `<meta name="description">`, `theme-color`, and `apple-touch-icon` in `frontend/index.html`, expanded tests, and updated `docs/refactor/AUDIT.md` checklist statuses.
- 2026-11-18: (`docs`) Completed Phase 6 (Final check): synced CodeGraph (`graphify-out/`), updated `README.md`, `CLAUDE.md`, and `AGENTS.md`, added final summary to `docs/refactor/PROGRESS.md`, and verified clean install, formatting, lint, SQLx offline cache, typecheck, full test suites, and production builds.
