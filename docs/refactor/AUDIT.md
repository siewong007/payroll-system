# Audit Report: Payroll System

## Summary

The system is a full-stack payroll and HR workflow platform for Malaysian SMEs, built using Rust (Axum, SQLx, PostgreSQL) for the backend and React (Vite, TypeScript, Tailwind) for the frontend.

### Stack

- **Backend:** Rust 2024, Axum 0.8, SQLx 0.8, Tokio.
- **Frontend:** React 19, TypeScript 7, Vite 8, Tailwind CSS 4, TanStack Query 5, i18next, Axios.
- **Database:** PostgreSQL 19 Beta 2.
- **Infrastructure:** Docker Compose, GitHub Actions, local encrypted backups, Caddy on AIC VPS.

## Current Repository Status

- Currently checked out on `refactor/standards`.
- Working tree is **dirty**:
  - Modified: `backend/src/main.rs`, `backend/src/repositories/background_jobs.rs`, `backend/src/repositories/password_reset_requests.rs`, `backend/src/services/job_service.rs`, `backend/src/tests/attendance_tests.rs`, `backend/src/tests/auth_audit_tests.rs`, `backend/src/tests/security_hardening_tests.rs`, `backend/src/tests/totp_route_tests.rs`.
  - Untracked: `.sqlx/` query json files.

## Problems Found

- **Repo State**: Dirty working tree on existing `refactor/standards` branch.
- **Statutory Rules**: Production PCB is disabled; requires verification. Statutory data are academic fixtures.
- **Dependencies**: Need to ensure all dependencies are compatible with the planned refactoring.
- **Tests**: Need characterization tests for flows that are currently modified in the dirty state.

## Refactor Plan

1. **Prepare**: Clean working tree (commit or stash changes).
2. **Phase 1: Tooling and Standards**: Ensure linter, formatter, and CI are configured correctly and running.
3. **Phase 2: Structure and Naming**: Organize by domain, apply consistent naming conventions.
4. **Phase 3: Code Quality**: Split large files, service/repository separation, error handling.
5. **Phase 4: Security/Performance**: Review auth, rate limits, indexing, N+1 queries.
6. **Phase 5: Fill Gaps**: Implement missing infrastructure/UX components.
7. **Phase 6: Final check**.

## Phase 5 Checklist Status

- [ ] README covering setup, environment variables, deployment.
- [ ] .env.example, CHANGELOG, LICENSE.
- [ ] CI workflow.
- [ ] Unit/Integration/E2E tests.
- [ ] Database migrations/seed data.
- [ ] Error monitoring (e.g., Sentry).
- [ ] Documented environments.
- [ ] CLAUDE.md.
- [ ] ... and more.
