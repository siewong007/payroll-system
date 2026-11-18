# Refactor Progress

## Phases

- [x] Phase 0: Understand (Done)
- [x] Phase 1: Tooling and standards
- [ ] Phase 2: Structure and naming
- [ ] Phase 3: Code quality
- [ ] Phase 4: Security, privacy and performance
- [ ] Phase 5: Fill the gaps
- [ ] Phase 6: Final check

## Log

- 2026-11-18: Initialized project understanding and created `docs/refactor/AUDIT.md`.
- 2026-11-18: Completed Phase 1 (Tooling and standards): configured `.editorconfig`, `lint-staged`, `husky`, and updated `.gitignore`.
- 2026-11-18: (`chore`) Hardened Phase 1 tooling: untracked `.lintstagedrc.json` from `.gitignore`, updated `.dockerignore`, pinned `.bun-version` (`1.3.14`) and `rust-toolchain.toml`, added `.prettierrc.json`/`.prettierignore`, added unified root scripts (`lint`, `format`, `format:check`, `typecheck`, `test`, `build`), removed unused prettier eslint packages, and fixed `.husky/pre-commit` + `.lintstagedrc.json`.
- 2026-11-18: (`refactor`) Removed remaining `any` casts and `eslint-disable` suppressions in `frontend/src/lib/webauthn.ts` and `frontend/src/pages/portal/TeamCalendar.tsx`.
