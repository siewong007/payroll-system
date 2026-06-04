# Repository Appearance Guide

This file lists presentation improvements for the GitHub repository profile and project demo materials.

## Suggested GitHub Description

Full-stack SME payroll and HR workflow system using Rust, Axum, React, PostgreSQL, attendance tracking, approvals, and Malaysian statutory payroll support.

## Suggested GitHub Topics

```text
payroll
hrms
sme
malaysia
rust
axum
react
typescript
vite
postgresql
sqlx
tailwindcss
webauthn
oauth2
docker
terraform
aws
final-year-project
```

## Logo and Banner Direction

Use the existing branding assets in `frontend/public/branding/`:

- `payrollmy-icon-primary.svg` for the repository avatar or favicon-style icon.
- `payrollmy-lockup-light.svg` for README branding.

Suggested GitHub social preview banner:

- Size: 1280 x 640.
- Left side: Payroll System logo and tagline.
- Right side: muted dashboard/payroll table preview.
- Text: "Payroll System - SME payroll and HR workflow management".
- Style: clean academic project presentation, with dark navy, white, and muted gold accents.

## Recommended Screenshots

Add screenshots under `docs/screenshots/`:

| Screenshot | Purpose |
| --- | --- |
| `login.png` | Authentication, passkey option, and first impression |
| `dashboard.png` | Admin dashboard overview |
| `employees.png` | Employee directory and HR records |
| `employee-import.png` | Bulk import workflow |
| `payroll-process.png` | Payroll processing form |
| `payroll-detail.png` | Payroll run summary and approval state |
| `attendance-kiosk.png` | QR/kiosk attendance flow |
| `portal-payslips.png` | Employee self-service payslip view |
| `portal-leave.png` | Leave request workflow |
| `reports.png` | Payroll/statutory reports |

## Demo GIF or Video Ideas

- 45 seconds: admin logs in, opens employees, and creates a payroll run.
- 45 seconds: employee submits leave, admin approves it, and the portal updates.
- 30 seconds: attendance kiosk generates a QR code and an employee checks in.
- 60 seconds: payroll run moves from draft to submitted, approved, and paid.
- 30 seconds: reports page exports statutory or attendance data.

## README Presentation Notes

- Keep claims factual and avoid stating that the system is production-ready.
- Place unimplemented ideas in roadmap or planned improvements.
- Use screenshots that show actual UI states, not only cropped decorative sections.
- Prefer a short demo video over many static screenshots when presenting to evaluators.
