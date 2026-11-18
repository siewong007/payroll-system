# Refactor Progress

## Phases

- [x] Phase 0: Understand (Done)
- [x] Phase 1: Tooling and standards
- [x] Phase 2: Structure and naming
- [x] Phase 3: Code quality
- [x] Phase 4: Security, privacy and performance
- [x] Phase 5: Fill the gaps
- [ ] Phase 6: Final check

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
