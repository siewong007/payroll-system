# Contributing to Payroll System

Thank you for your interest in improving this project. This repository is maintained as an academic and portfolio-oriented payroll/HR workflow system, so contributions should be clear, reviewable, and aligned with the existing architecture.

## Ways to Contribute

- Report reproducible bugs.
- Suggest focused feature improvements.
- Improve documentation, tests, and accessibility.
- Review payroll, attendance, approval, or security-related behavior.
- Refactor code only when it reduces complexity or follows an existing project direction.

## Development Setup

### Prerequisites

- Docker and Docker Compose
- Rust stable toolchain
- Bun 1.3.14

### Local Setup

```bash
git clone https://github.com/siewong007/payroll-system.git
cd payroll-system
cp .env.example .env
cp .env.example backend/.env
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

- Keep handlers thin. Handlers should extract request data, call a service, and return a response.
- Place business logic in `backend/src/services/`.
- Place SQL/data access in `backend/src/repositories/` or `backend/src/repositories/reads/`.
- Use `AppResult<T>` and existing `AppError` variants for fallible paths.
- Use `rust_decimal::Decimal` for money. Do not introduce floating-point payroll calculations.
- Add new schema changes as new migration files in `backend/migrations/schema/`.
- Do not edit existing migrations unless the repository owner explicitly asks for a history rewrite.

### Frontend

- Use the existing Axios instance from `frontend/src/api/client.ts`.
- Do not create a second HTTP client.
- Keep API types aligned with backend contract changes.
- Use existing layout, role guard, and React Query patterns.
- Keep UI changes accessible and responsive.

## SQLx Offline Cache

Some backend queries use SQLx compile-time macros. If you add or change a macro query, regenerate the SQLx cache against a migrated PostgreSQL database:

```bash
cd backend
DATABASE_URL=postgres://payroll:payroll_secret_change_me@localhost:5432/payroll_db cargo sqlx prepare
```

Commit the updated `backend/.sqlx/` files when present.

## Required Checks

Run the relevant checks before opening a pull request.

Backend:

```bash
cd backend
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Frontend:

```bash
cd frontend
bun run lint
bun run test
bun run build
```

## Commit Guidelines

Use short, descriptive commit messages in present tense.

Examples:

```text
Add attendance CSV export filters
Fix payroll approval status transition
Document backend environment variables
```

Keep the first line under 72 characters when possible.

## Pull Request Guidelines

- Keep each pull request focused on one feature, fix, or documentation change.
- Explain what changed, why it changed, and how it was tested.
- Include screenshots for UI changes.
- Include migration notes for database changes.
- Update README or related docs when behavior changes.
- Avoid unrelated formatting or refactoring in the same pull request.

## Reporting Issues

When reporting a bug, include:

- Steps to reproduce.
- Expected behavior.
- Actual behavior.
- Relevant logs, screenshots, or API responses.
- Browser/OS details for frontend issues.
- Database or migration context for backend issues.

Use the issue templates in `.github/ISSUE_TEMPLATE/` where possible.

## Security Issues

Please do not report security vulnerabilities in public issues. Follow [SECURITY.md](SECURITY.md).

## Code of Conduct

All contributors are expected to follow [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
