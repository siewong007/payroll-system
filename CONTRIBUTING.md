# Contributing to Payroll System

Thank you for your interest in improving this project. This repository is maintained as an academic and portfolio-oriented payroll/HR workflow system, so contributions should be clear, reviewable, and aligned with the existing architecture. Read [AGENTS.md](AGENTS.md) for detailed subsystem invariants.

## Ways to Contribute

- Report reproducible bugs.
- Suggest focused feature improvements.
- Improve documentation, tests, and accessibility.
- Review payroll, attendance, approval, or security-related behavior.
- Refactor code only when it reduces complexity or follows an existing project direction.

## Development Setup

### Prerequisites

- Docker and Docker Compose
- Rust stable toolchain (pinned in `rust-toolchain.toml` with `rustfmt` and `clippy`)
- Bun 1.3.14 (pinned in `.bun-version`)

### Local Setup

```bash
git clone https://github.com/siewong007/payroll-system.git
cd payroll-system
cp .env.example .env
# Set strong, distinct values for POSTGRES_PASSWORD, JWT_SECRET, and TOTP_ENCRYPTION_KEY in .env
bun install                 # installs Husky + lint-staged pre-commit hooks
docker compose up -d
```

Backend:

```bash
cd backend
cargo run
```

Frontend:

```bash
cd frontend
bun install
bun run dev
```

## Project Architecture Rules

### Backend

- Keep handlers thin. Extract auth context, validate inputs, call a service, and return a response.
- Place business logic and transaction boundaries in `backend/src/services/`.
- Place SQL/data access in `backend/src/repositories/` or `backend/src/repositories/reads/`.
- Use `AppResult<T>` and existing `AppError` variants for fallible paths.
- Use integer `sen` (`i64`) and `rust_decimal::Decimal` for money. Never introduce `f64` payroll math.
- The consolidated schema baseline is `backend/migrations/1000_schema.sql` plus `1001_data.sql`, followed by immutable forward migrations (`1002`–`1034` and onward).
- Treat every deployed migration as strictly immutable (CI enforces forward-only immutability). Later changes must use the next numbered migration file; squashing history requires an explicit rebaseline and fresh/upgrade verification.
- Keep statutory rules effective-dated and table-driven (`statutory_rule_sets`); see [docs/legal.md](docs/legal.md#part-iii-malaysian-statutory-compliance-guide-2026) and [docs/payroll.md](docs/payroll.md).

### Frontend

- Use the existing Axios instance from `frontend/src/api/client.ts`. Do not create a second HTTP client.
- Keep route paths centralized in `frontend/src/lib/routes.ts` and types aligned in `frontend/src/types/`.
- Use existing layout, `PermissionGuard`, and React Query patterns.
- Route all user-facing strings through i18next (`en`, `ms`, `zh-CN`, `zh-TW`) with full key/interpolation parity and script purity ([docs/i18n.md](docs/i18n.md)).
- Keep UI changes accessible (`>=44px` mobile targets, labeled controls) and responsive.

## SQLx Offline Cache

Backend queries use SQLx compile-time macros (`query!`, `query_as!`, `query_scalar!`). After adding or changing a macro query, regenerate the offline cache against a migrated PostgreSQL 19 database:

```bash
cd backend
DATABASE_URL=postgres://payroll:payroll_secret_change_me@localhost:5434/payroll_db cargo sqlx prepare
SQLX_OFFLINE=true cargo check --all-targets
```

Commit the updated `backend/.sqlx/` files with your change.

## Required Checks

Run the unified root gates before opening a pull request:

```bash
bun run format:check
bun run lint                # cargo clippy -- -D warnings, eslint, and i18n-scan --ci
bun run typecheck           # frontend tsc -b --noEmit
bun run test                # backend cargo test + frontend Vitest suite
bun run build               # backend cargo build + frontend production build
```

Or run per-package checks individually:

```bash
cd backend
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

```bash
cd frontend
bun run lint
bun run typecheck
bun run test
bun run build
cd ..
node scripts/i18n-scan.mjs --ci
```

## Commit and Pull Request Guidelines

- Use short, descriptive commit messages in present tense (first line under 72 characters).
- Keep each pull request focused on one feature, fix, or documentation change.
- Explain what changed, why it changed, and how it was tested; include screenshots for UI changes and migration notes for schema changes.
- Update documentation when behavior changes.
- Use the templates in `.github/ISSUE_TEMPLATE/` and `.github/pull_request_template.md`.

## Security and Conduct

- Please do not report security vulnerabilities in public issues. Follow [SECURITY.md](SECURITY.md).
- All contributors are expected to follow [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
