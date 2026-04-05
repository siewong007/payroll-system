<div align="center">

# PayrollMY

---

**A full-stack payroll management system built for Malaysian businesses with Rust, React, and Tailwind.**
**Handles statutory compliance (EPF, SOCSO, EIS, PCB), employee self-service, and multi-company support.**

[![Quick Start](https://img.shields.io/badge/Quick_Start-5_MIN-blue?style=for-the-badge)](#quick-start)
[![Features](https://img.shields.io/badge/Features-14+-gray?style=for-the-badge)](#features)
[![API](https://img.shields.io/badge/API-15_MODULES-green?style=for-the-badge)](#api-modules)
[![License](https://img.shields.io/badge/License-MIT-gold?style=for-the-badge)](LICENSE)

[![Rust](https://img.shields.io/badge/Rust-2024-orange?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Axum](https://img.shields.io/badge/Axum-0.8-gray?style=flat-square)](https://github.com/tokio-rs/axum)
[![React](https://img.shields.io/badge/React-19-61DAFB?style=flat-square&logo=react&logoColor=white)](https://react.dev/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.9-3178C6?style=flat-square&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Vite](https://img.shields.io/badge/Vite-8-646CFF?style=flat-square&logo=vite&logoColor=white)](https://vitejs.dev/)
[![Tailwind CSS](https://img.shields.io/badge/Tailwind_CSS-4-06B6D4?style=flat-square&logo=tailwindcss&logoColor=white)](https://tailwindcss.com/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-14+-4169E1?style=flat-square&logo=postgresql&logoColor=white)](https://www.postgresql.org/)

</div>

---

## Features

<table>
<tr>
<td width="50%">

### Payroll & Compliance
- Automated monthly payroll processing
- EPF, SOCSO, EIS, and PCB auto-computation
- Zakat, PTPTN, and Tabung Haji deductions
- Salary history and mid-year TP3 imports
- Payroll approval and lock workflows
- Configurable payroll groups, cutoff dates, and overtime multipliers

</td>
<td width="50%">

### Employee Management
- Full employee lifecycle — onboarding to offboarding
- Department, designation, and team assignment
- Banking details and statutory registration numbers
- Probation tracking and confirmation management
- Bulk employee import with CSV upload
- Employee directory with search and filters

</td>
</tr>
<tr>
<td width="50%">

### Employee Self-Service Portal
- View and download payslips
- Submit leave requests with attachments
- File expense claims with receipt uploads
- Apply for overtime with auto-calculation
- Team calendar and public holidays
- Personal profile and statutory details

</td>
<td width="50%">

### Approval Workflows
- Leave request approvals with manager review
- Expense claim review and processing
- Overtime application approvals
- Admin notification system for pending items

</td>
</tr>
<tr>
<td width="50%">

### HR Letters & Email
- Compose HR letters: Offer, Appointment, Warning, Termination, Promotion
- Template system with variable substitution
- Preview emails before sending
- Auto-send welcome email on employee creation
- Email delivery log with status tracking

</td>
<td width="50%">

### Documents & Calendar
- Company document management with categories and expiry tracking
- Holiday calendar with ICS import
- Configurable working days per company
- State-specific holiday support

</td>
</tr>
<tr>
<td width="50%">

### Authentication & Security
- Passwordless login with **Passkeys (WebAuthn)**
- Google OAuth2 single sign-on
- JWT with secure httpOnly refresh cookies
- Role-based access control (6 roles)
- Password reset with admin approval
- Per-endpoint rate limiting

</td>
<td width="50%">

### Multi-Company & Reporting
- Manage multiple companies from one dashboard
- Users assigned to multiple companies with switching
- Company-scoped data isolation
- Payroll summary and department reports
- Leave utilization, claims, and statutory reports

</td>
</tr>
</table>

## API Modules

| Module | Description |
|--------|-------------|
| `auth` | Login, passkeys, OAuth2, JWT refresh |
| `employees` | CRUD, bulk import, onboarding/offboarding |
| `payroll` | Processing, calculations, approvals |
| `leaves` | Requests, balances, approvals |
| `claims` | Expense claims, receipt uploads |
| `overtime` | Applications, auto-calculation |
| `companies` | Multi-company management |
| `departments` | Department and team structure |
| `designations` | Role and title management |
| `documents` | Upload, categorize, expiry tracking |
| `calendar` | Holidays, ICS import, working days |
| `letters` | HR letter templates and sending |
| `reports` | Payroll, leave, claims, statutory |
| `settings` | System and company configuration |
| `backup` | Database backup and restore |

## Quick Start

### Prerequisites

- **Docker & Docker Compose** — for PostgreSQL and Redis
- **Rust** (latest stable) — backend
- **Node.js 18+** — frontend

### Setup

```bash
# Clone the repository
git clone https://github.com/your-username/payroll-system.git
cd payroll-system

# Start PostgreSQL and Redis
docker compose up -d

# Backend
cd backend
cp .env.example .env    # Configure your environment
cargo run

# Frontend (in a new terminal)
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
| `GOOGLE_CLIENT_ID` | Google OAuth2 client ID *(optional)* |
| `GOOGLE_CLIENT_SECRET` | Google OAuth2 secret *(optional)* |

### Demo Accounts

| Role | Email | Password |
|------|-------|----------|
| Super Admin | admin@demo.com | admin123 |
| Executive | exec@demo.com | admin123 |
| Employee | sarah@demo.com | admin123 |

## Project Structure

```
payroll-system/
├── backend/                 # Rust API server (Axum)
│   ├── src/
│   │   ├── handlers/        # Route handlers
│   │   ├── services/        # Business logic
│   │   ├── models/          # Data models & queries
│   │   ├── core/            # Auth, config, error handling
│   │   └── routes/          # Route definitions
│   └── migrations/          # PostgreSQL migrations (30+)
├── frontend/                # React SPA
│   └── src/
│       ├── pages/           # Page components (15 modules)
│       ├── components/      # Reusable UI components
│       ├── api/             # API client modules
│       ├── context/         # Auth context & providers
│       └── types/           # TypeScript interfaces
├── infra/                   # Terraform (AWS deployment)
└── docker-compose.yml       # Local development services
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
