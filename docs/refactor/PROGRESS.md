# Refactor Progress

## Phases

- [x] Phase 0: Understand (Done)
- [x] Phase 1: Tooling and standards
- [x] Phase 2: Structure and naming
- [ ] Phase 3: Code quality
- [ ] Phase 4: Security, privacy and performance
- [ ] Phase 5: Fill the gaps
- [ ] Phase 6: Final check

## Log

- 2026-11-18: Initialized project understanding and created `docs/refactor/AUDIT.md`.
- 2026-11-18: Completed Phase 1 (Tooling and standards): configured `.editorconfig`, `lint-staged`, `husky`, and updated `.gitignore`.
- 2026-11-18: (`chore`) Hardened Phase 1 tooling: untracked `.lintstagedrc.json` from `.gitignore`, updated `.dockerignore`, pinned `.bun-version` (`1.3.14`) and `rust-toolchain.toml`, added `.prettierrc.json`/`.prettierignore`, added unified root scripts (`lint`, `format`, `format:check`, `typecheck`, `test`, `build`), removed unused prettier eslint packages, and fixed `.husky/pre-commit` + `.lintstagedrc.json`.
- 2026-11-18: (`refactor`) Removed remaining `any` casts and `eslint-disable` suppressions in `frontend/src/lib/webauthn.ts` and `frontend/src/pages/portal/TeamCalendar.tsx`.
- 2026-11-18: (`docs`) Completed full-repository audit, severity inventory, and Phase 5 checklist in `docs/refactor/AUDIT.md`.
- 2026-11-18: (`refactor`) Completed Phase 2 (Structure and naming): moved remaining SQL out of `main.rs`, `services/auth_service.rs`, and `services/statutory_rules.rs` into `repositories/` (`refresh_tokens::purge_stale`, `user_groups::user_has_any_active_permission`, `users::list_two_factor_non_compliant_emails`, `statutory_rule_sets::pcb_status_and_rule_verified`), named retention constants in `main.rs`, refreshed `backend/.sqlx/`, and centralized route constants in `frontend/src/lib/routes.ts`.
