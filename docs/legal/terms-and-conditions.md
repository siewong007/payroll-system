# Terms and Conditions of Use

**Effective Date:** 18 November 2026  
**Document Version:** 2.0 (2026 Regulatory Alignment)  
**Governing Law:** Laws of Malaysia

---

## 1. Purpose, Scope, and Platform Status

1.1 These Terms and Conditions of Use (**"Terms"**) govern access to and use of the **Payroll System** web application, employee self-service portal, attendance kiosks, APIs, background services, and exported documents/files (collectively, the **"Platform"**).

1.2 By signing in, scanning an attendance QR code, registering a passkey or two-factor authenticator, administering a tenant organization, or otherwise accessing the Platform, you (**"User"**, whether acting as a platform administrator, employer representative, HR/payroll/finance officer, executive, or employee) agree to comply with these Terms.

1.3 **Software License and Prototype Notice:** Source code in the open-source repository is distributed under the [MIT License](../../LICENSE). Unless an organization has independently audited, configured, and verified its deployment and statutory rule sets for live production operations, the Platform is provided **"AS IS"** without warranty of legal, tax, accounting, or statutory compliance. Nothing in the Platform constitutes legal, tax, or statutory advice.

---

## 2. Roles and Multi-Tenant Responsibilities

2.1 **Employer Tenant ("Data Controller"):** Each subscribing or self-hosting employer entity configured in the Platform (`companies`) is solely responsible for:

- Determining the lawful basis for collecting and processing employee records under the **Personal Data Protection Act 2010 (Act 709)** as amended by the **Personal Data Protection (Amendment) Act 2024 (Act A1727)** (**"PDPA"**);
- Ensuring accuracy of employee master data (full legal name per NRIC/passport, date of birth, residency status, marital/child declarations, bank account details, salary, allowances, and statutory registration numbers);
- Importing, verifying, and approving authoritative statutory schedules (`statutory_rule_sets`) before enabling live payroll runs;
- Reviewing payroll previews, payslip breakdowns, bank payment files, EA forms, and statutory export files prior to disbursement or submission to **KWSP (EPF)**, **PERKESO (SOCSO & EIS)**, **LHDN (PCB/MTD)**, **HRD Corp**, or financial institutions; and
- Paying wages within the statutory period mandated by **Section 19 of the Employment Act 1955 (Act 265)** (or the **Sabah Labour Ordinance [Cap. 67]** / **Sarawak Labour Ordinance [Cap. 76]**, as amended effective 1 May 2025) and remitting statutory contributions by the **15th day of the following calendar month**.

2.2 **Platform Operator / Host ("Data Processor"):** Where the Platform is hosted on behalf of an employer tenant, the operator acts as a **Data Processor** within the meaning of the PDPA and maintains technical and organizational security controls in accordance with **Section 9 of the PDPA**.

2.3 **Authorized Users:** Access rights are strictly role- and permission-scoped (`super_admin`, `admin`, `payroll_admin`, `hr_manager`, `finance`, `exec`, `employee`, plus assigned `user_groups`). Users must only access tenant data and perform actions within the scope of their employment authority.

---

## 3. Account Security, Multi-Factor Authentication, and Credentials

3.1 **Personal Credentials:** Login credentials, TOTP authenticator secrets, recovery codes, Google OAuth2 links, and WebAuthn passkeys are strictly personal and non-transferable. Sharing credentials or approving another person's two-factor prompt is prohibited.

3.2 **Mandatory Password Change & Session Hygiene:** Users provisioned with a temporary or reset credential must complete password rotation immediately when prompted (`must_change_password`). Changing a password revokes all active refresh tokens and user sessions across devices.

3.3 **Privileged Access:** Users holding administrative, payroll, HR, or finance permissions must maintain strong passwords, protect exported CSV/PDF/JSON files containing personal or banking data, and promptly report any suspected unauthorized access or credential compromise to their Employer Administrator and Data Protection Officer (**"DPO"**).

---

## 4. Attendance, Kiosk, Geofence, and Office-Network Rules

4.1 **Truthful Attendance Recording:** Employees must record their own check-in and check-out events accurately. Proxy check-ins (scanning a QR code, presenting a session, or transmitting attendance requests on behalf of another employee), falsifying attendance timestamps, or spoofing GPS coordinates or network paths may constitute misconduct under the employer's disciplinary policy.

4.2 **Kiosk Display Credentials:** Wall-mounted or tablet kiosk URLs (`/kiosk/:kioskKey`) authenticate using revocable kiosk secrets. Kiosk devices must be physically secured on employer premises and must not be shared publicly or embedded on external websites.

4.3 **WebAuthn ("Face ID") Check-In:** Where WebAuthn attendance verification is enabled, biometric matching occurs locally on the User's own device authenticator (e.g., Apple Face ID / Touch ID / Windows Hello / Android BiometricPrompt). Only cryptographic public-key assertions are sent to the Platform; raw biometric images or templates are never transmitted to or stored by the Platform.

4.4 **Geofence and Network Evidence:** Depending on employer policy (`none`, `learn`, `warn`, `enforce`), check-in requests may evaluate device geolocation against configured workplace radii or evaluate server-observed egress IP prefixes against approved office networks. Check-out is never blocked by geofence or network enforcement so employees are never stranded in an open shift, though off-site check-outs may be flagged for HR review.

---

## 5. Payroll, Statutory Calculations, and Electronic Records

5.1 **Fail-Closed Statutory Safety Gate:** Automatic payroll calculation refuses to execute in production unless verified, source-linked rule sets (`status = 'verified'` with SHA-256 provenance) cover the payroll period's effective date. Automatic computerised PCB calculation remains disabled unless and until the deployment has passed **LHDN Computerised Calculation Method (Spesifikasi Kaedah Pengiraan Berkomputer PCB)** conformance testing and an authorized operator enables `pcb_calculator_status`.

5.2 **Separation of Duties:** Payroll preparation (`manage_payroll_draft`, `submit_payroll`) and financial approval/locking (`approve_payroll`, `mark_payroll_paid`) are separated by permission gates, and four-eyes controls prevent the preparer of a run from approving their own submission.

5.3 **Electronic Payslips and Statements:** Subject to **Section 61 of the Employment Act 1955** and **Subsection 83(1A) of the Income Tax Act 1967**, employees consent to receiving itemized monthly payslips and annual **Form EA (Borang C.P.8A)** statements electronically through the Employee Self-Service Portal and/or registered email. Employees should review payslips promptly and notify HR/Payroll of any discrepancy.

5.4 **Leave, Claims, and Overtime Submissions:** Supporting documents uploaded for medical leave, hospitalization leave, expense claims, or HR records must be genuine, unaltered, and business-related. Fraudulent claims or forged medical certificates may result in rejection, recovery of overpayments in accordance with **Section 24 of the Employment Act 1955**, and disciplinary or legal action.

---

## 6. Acceptable Use Restrictions

Users must **not**:

- Attempt to bypass tenant isolation (`company_id` scoping), role/permission guards, Cloudflare Turnstile bot protection, or rate limits;
- Upload malware, executable scripts, disguised file extensions, or files exceeding permitted size and magic-byte content checks;
- Submit internal, loopback, link-local, or private-network URLs to calendar import endpoints (SSRF probing);
- Scrape, bulk-exfiltrate, or disclose confidential payroll, NRIC/passport, banking, tax, or health records outside authorized business workflows; or
- Modify deployed database migrations in place or tamper with audit trail records (`audit_logs`).

---

## 7. Data Protection and Record Retention

7.1 Processing of personal data and sensitive personal data on the Platform is governed by the [Privacy Policy & PDPA Personal Data Protection Notice](privacy-and-pdpa-notice.md), which forms an integral part of these Terms.

7.2 Statutory employment and tax records (including payroll runs, payslips, EA data, and audit trails) are retained in accordance with Malaysian statutory minimums—specifically **6 years** under **Section 61 of the Employment Act 1955** and **7 years (2,555 days)** under **Section 82 of the Income Tax Act 1967**—while transient operational telemetry (such as network learning observations, expired tokens, and session logs) is automatically purged on shorter schedules.

---

## 8. Service Availability, Backups, and Modifications

8.1 Scheduled maintenance, database upgrades (including PostgreSQL major/beta migrations), or security patches may temporarily suspend access. Background jobs interrupted by process restarts are automatically marked failed on startup so operators can safely re-submit them without duplicate payroll runs.

8.2 Whole-company JSON/archive backup exports contain confidential personal and financial data. Administrators exporting or restoring tenant backups are responsible for encrypting, storing, and transmitting archive files securely.

8.3 These Terms may be updated to reflect changes in Malaysian legislation, statutory agency specifications, or Platform capabilities. Material updates will be reflected by the Effective Date and Document Version above.

---

## 9. Disclaimer of Warranties and Limitation of Liability

9.1 TO THE MAXIMUM EXTENT PERMITTED BY APPLICABLE MALAYSIAN LAW, THE PLATFORM AND ITS OPEN-SOURCE SOFTWARE COMPONENTS ARE PROVIDED **"AS IS"** AND **"AS AVAILABLE"**, WITHOUT WARRANTIES OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE, NON-INFRINGEMENT, OR STATUTORY CALCULATION ACCURACY.

9.2 IN NO EVENT SHALL THE AUTHORS, CONTRIBUTORS, OR OPEN-SOURCE MAINTAINERS BE LIABLE FOR ANY INDIRECT, INCIDENTAL, CONSEQUENTIAL, SPECIAL, EXEMPLARY, OR STATUTORY PENALTY DAMAGES (INCLUDING ANY LATE PAYMENT SURCHARGE, DIVIDEND, FINE, OR ASSESSMENT IMPOSED BY KWSP, PERKESO, LHDN, JTKSM, HRD CORP, OR THE PDP COMMISSIONER) ARISING OUT OF OR IN CONNECTION WITH USE OF THE PLATFORM.

---

## 10. Governing Law and Jurisdiction

10.1 These Terms shall be governed by and construed in accordance with the **laws of Malaysia**.

10.2 Any dispute, controversy, or claim arising out of or relating to these Terms or the use of the Platform shall be subject to the exclusive jurisdiction of the courts of Malaysia, without prejudice to statutory remedies available before the **Director General of Labour (Jabatan Tenaga Kerja)**, the **Industrial Court of Malaysia**, or the **Personal Data Protection Commissioner**.
