# PayrollMY

> A modern, full-featured payroll management system built for Malaysian businesses. Handles everything from employee onboarding to statutory compliance (EPF, SOCSO, EIS, PCB), with a self-service employee portal and multi-company support.

## Features

### Payroll & Compliance
- Automated monthly payroll processing with Malaysian statutory calculations
- EPF, SOCSO, EIS, and PCB (income tax) auto-computation
- Zakat deductions and PTPTN/Tabung Haji support
- Salary history tracking and mid-year TP3 balance imports
- Payroll approval and lock workflows
- Configurable payroll groups, cutoff dates, and overtime multipliers

### Employee Management
- Complete employee lifecycle — onboarding to offboarding
- Department, designation, and team assignment
- Banking details and statutory registration numbers
- Probation tracking and confirmation management
- Bulk employee directory with search and filters

### Employee Self-Service Portal
- View and download payslips
- Submit leave requests with attachment support
- File expense claims with receipt uploads
- Apply for overtime with auto-calculation
- View team calendar and public holidays
- Personal profile and statutory details

### Approval Workflows
- Leave request approvals with manager review
- Expense claim review and processing
- Overtime application approvals
- Admin notification system for pending items

### HR Letters & Email
- Compose and send HR letters: Offer, Appointment, Warning, Termination, Promotion
- Template system with variable substitution (employee name, company, designation, etc.)
- Preview emails before sending with full variable resolution
- Auto-send welcome email on employee creation
- Complete email delivery log with status tracking

### Documents & Calendar
- Company document management with categories and expiry tracking
- Holiday calendar with ICS import support
- Configurable working days per company
- State-specific holiday support

### Authentication & Security
- Passwordless login with **Passkeys (WebAuthn)** — fingerprint, Face ID, device PIN
- Google OAuth2 single sign-on
- JWT authentication with secure httpOnly refresh token cookies
- Role-based access control (Super Admin, Admin, Executive, HR, Finance, Employee)
- Password reset with admin approval workflow
- Per-endpoint rate limiting

### Multi-Company Platform
- Super admin manages multiple companies from a single dashboard
- Users can be assigned to multiple companies with seamless switching
- Company-scoped data isolation
- Per-company settings and configurations

### Reporting
- Payroll summary and department breakdown reports
- Leave utilization reports
- Claims and expense reports
- Statutory contribution reports

## Getting Started

### Prerequisites
- Docker & Docker Compose
- Rust (latest stable)
- Node.js 18+

### Setup

```bash
# Start PostgreSQL and Redis
docker compose up -d

# Backend
cd backend
cp .env.example .env    # Configure your environment
cargo run

# Frontend
cd frontend
npm install
npm run dev
```

The app will be available at `http://localhost:5173`.

### Environment Variables

| Variable | Description |
|----------|-------------|
| `DATABASE_URL` | PostgreSQL connection string |
| `JWT_SECRET` | Secret key for JWT signing |
| `FRONTEND_URL` | Frontend URL for CORS and email links |
| `SMTP_HOST` | SMTP server for sending emails |
| `SMTP_FROM_EMAIL` | Sender email address |
| `WEBAUTHN_RP_ID` | WebAuthn relying party ID (your domain) |
| `WEBAUTHN_RP_ORIGIN` | WebAuthn origin URL |
| `GOOGLE_CLIENT_ID` | Google OAuth2 client ID (optional) |
| `GOOGLE_CLIENT_SECRET` | Google OAuth2 secret (optional) |

### Demo Accounts

| Role | Email | Password |
|------|-------|----------|
| Super Admin | admin@demo.com | admin123 |
| Executive | exec@demo.com | admin123 |
| Employee | sarah@demo.com | admin123 |

## Project Structure

```
payroll-system/
├── backend/              # Rust API server
│   ├── src/
│   │   ├── handlers/     # Route handlers
│   │   ├── services/     # Business logic
│   │   ├── models/       # Data models
│   │   ├── core/         # Auth, config, error handling
│   │   └── routes/       # Route definitions
│   └── migrations/       # PostgreSQL migrations (30+)
├── frontend/             # React SPA
│   └── src/
│       ├── pages/        # Page components
│       ├── components/   # Reusable UI components
│       ├── api/          # API client modules
│       ├── context/      # Auth context
│       └── types/        # TypeScript interfaces
└── infra/                # Terraform (AWS deployment)
```

## License

MIT
