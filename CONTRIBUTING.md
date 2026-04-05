# Contributing to PayrollMY

Thank you for your interest in contributing! This guide will help you get started.

## Getting Started

1. **Fork** the repository
2. **Clone** your fork:
   ```bash
   git clone https://github.com/your-username/payroll-system.git
   cd payroll-system
   ```
3. **Create a branch** for your changes:
   ```bash
   git checkout -b feature/your-feature-name
   ```

## Development Setup

### Prerequisites

- Docker & Docker Compose
- Rust (latest stable)
- Node.js 18+

### Running Locally

```bash
# Start services
docker compose up -d

# Backend
cd backend
cp .env.example .env
cargo run

# Frontend (new terminal)
cd frontend
npm install
npm run dev
```

## Making Changes

### Code Style

**Backend (Rust)**
- Follow standard Rust conventions (`cargo fmt` and `cargo clippy`)
- Use meaningful variable and function names
- Keep handler functions thin — business logic belongs in `services/`

**Frontend (React/TypeScript)**
- Use functional components with hooks
- Follow existing patterns for API calls (`src/api/`)
- Use TypeScript types — avoid `any`
- Use Tailwind CSS for styling

### Commit Messages

Write clear, concise commit messages:

```
Add employee bulk import via CSV upload
Fix PCB calculation for mid-year joiners
Update leave balance calculation to handle carry-forward
```

- Use present tense ("Add feature" not "Added feature")
- Keep the first line under 72 characters
- Reference issue numbers when applicable (e.g., `Fix #42`)

## Submitting Changes

1. **Push** your branch to your fork:
   ```bash
   git push origin feature/your-feature-name
   ```

2. **Open a Pull Request** against the `main` branch

3. In your PR description, include:
   - What the change does
   - Why the change is needed
   - How to test it
   - Screenshots for UI changes

## Pull Request Guidelines

- Keep PRs focused — one feature or fix per PR
- Ensure the backend compiles without errors (`cargo build`)
- Ensure the frontend builds without errors (`npm run build`)
- Update types in `frontend/src/types/` if API contracts change
- Add or update migrations in `backend/migrations/` for schema changes

## Reporting Issues

When reporting a bug, please include:

- Steps to reproduce the issue
- Expected vs actual behavior
- Browser and OS information (for frontend issues)
- Relevant error messages or logs

## Areas for Contribution

Here are some areas where contributions are especially welcome:

- **Testing** — unit and integration tests for backend services
- **Documentation** — API documentation, inline code comments
- **Accessibility** — improving frontend accessibility (ARIA labels, keyboard navigation)
- **Localization** — adding Malay language support
- **Performance** — query optimization, frontend bundle size reduction

## Code of Conduct

- Be respectful and constructive in all interactions
- Welcome newcomers and help them get started
- Focus on the technical merits of contributions
- Disagree respectfully — critique code, not people

## Questions?

If you have questions about contributing, feel free to open an issue with the **question** label.

Thank you for helping improve PayrollMY!
