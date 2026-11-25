# Legal, Privacy (PDPA), and Malaysian Statutory Compliance

**Effective / Last Reviewed Date:** 25 November 2026  
**Governing Law & Scope:** Laws of Malaysia (Peninsular Malaysia, Federal Territories, Sabah & Sarawak)

This consolidated legal and regulatory reference covers:

1. [Part I: Terms and Conditions of Use](#part-i-terms-and-conditions-of-use) (in-app route `/terms`)
2. [Part II: Privacy Policy and PDPA Notice](#part-ii-privacy-policy-and-pdpa-notice) (bilingual English & Bahasa Malaysia Section 7 PDPA Notice; in-app route `/privacy`)
3. [Part III: Malaysian Statutory Compliance Guide (2026)](#part-iii-malaysian-statutory-compliance-guide-2026) (in-app route `/compliance`)

---

## Part I: Terms and Conditions of Use

**Document Version:** 2.1 (2026 Regulatory Alignment)

### 1. Purpose, Scope, and Platform Status

1.1 These Terms and Conditions of Use (**"Terms"**) govern access to and use of the **Payroll System** web application, installable Progressive Web App (PWA), employee self-service portal, attendance kiosks, push/email notifications, APIs, background services, and exported documents/files (collectively, the **"Platform"**).

1.2 By signing in, installing the PWA, enabling Web Push notifications, scanning an attendance QR code, registering a passkey or two-factor authenticator, administering a tenant organization, or otherwise accessing the Platform, you (**"User"**, whether acting as a platform administrator, employer representative, HR/payroll/finance/manager officer, executive, or employee) agree to comply with these Terms.

1.3 **Software License and Prototype Notice:** Source code in the open-source repository is distributed under the [MIT License](../LICENSE). Unless an organization has independently audited, configured, and verified its deployment and statutory rule sets for live production operations, the Platform is provided **"AS IS"** without warranty of legal, tax, accounting, or statutory compliance. Nothing in the Platform constitutes legal, tax, or statutory advice.

### 2. Roles and Multi-Tenant Responsibilities

2.1 **Employer Tenant ("Data Controller"):** Each subscribing or self-hosting employer entity configured in the Platform (`companies`) is solely responsible for:

- Determining the lawful basis for collecting and processing employee records under the **Personal Data Protection Act 2010 (Act 709)** as amended by the **Personal Data Protection (Amendment) Act 2024 (Act A1727)** (**"PDPA"**);
- Ensuring accuracy of employee master data (full legal name per NRIC/passport, date of birth, residency status, marital/child declarations, bank account details, salary, allowances, and statutory registration numbers);
- Importing, verifying, and approving authoritative statutory schedules (`statutory_rule_sets`) before enabling live payroll runs;
- Reviewing payroll previews, payslip breakdowns, bank payment files, EA forms, and statutory export files prior to disbursement or submission to **KWSP (EPF)**, **PERKESO (SOCSO & EIS)**, **LHDN (PCB/MTD)**, **HRD Corp**, or financial institutions; and
- Paying wages within the statutory period mandated by **Section 19 of the Employment Act 1955 (Act 265)** (or the **Sabah Labour Ordinance [Cap. 67]** / **Sarawak Labour Ordinance [Cap. 76]**, as amended effective 1 May 2025) and remitting statutory contributions by the **15th day of the following calendar month**.

2.2 **Platform Operator / Host ("Data Processor"):** Where the Platform is hosted on behalf of an employer tenant, the operator acts as a **Data Processor** within the meaning of the PDPA and maintains technical and organizational security controls in accordance with **Section 9 of the PDPA**.

2.3 **Authorized Users:** Access rights are strictly role- and permission-scoped across `super_admin`, `admin`, `manager`, `payroll_admin`, `hr_manager`, `finance`, `exec`, and `employee`, plus assigned company-scoped `user_groups`. Users must only access tenant data and perform actions within the scope of their employment authority.

### 3. Account Security, Multi-Factor Authentication, and Credentials

3.1 **Personal Credentials:** Login credentials, TOTP authenticator secrets, recovery codes, Google OAuth2 links, and WebAuthn passkeys are strictly personal and non-transferable. Sharing credentials or approving another person's two-factor prompt is prohibited.

3.2 **Mandatory Password Change & Session Hygiene:** Users provisioned with a temporary or reset credential must complete password rotation immediately when prompted (`must_change_password`). Changing a password revokes all active refresh tokens and user sessions across devices.

3.3 **Privileged Access:** Users holding administrative, manager, payroll, HR, or finance permissions must maintain strong passwords, protect exported CSV/PDF/JSON files containing personal or banking data, and promptly report any suspected unauthorized access or credential compromise to their Employer Administrator and Data Protection Officer (**"DPO"**).

### 4. Attendance, Kiosk, Geofence, and Office-Network Rules

4.1 **Truthful Attendance Recording:** Employees must record their own check-in and check-out events accurately. Proxy check-ins (scanning a QR code, presenting a session, or transmitting attendance requests on behalf of another employee), falsifying attendance timestamps, or spoofing GPS coordinates or network paths may constitute misconduct under the employer's disciplinary policy.

4.2 **Kiosk Display Credentials:** Wall-mounted or tablet kiosk URLs (`/kiosk#<secret>`, with legacy `/kiosk/:kioskKey` compatibility) authenticate using revocable kiosk secrets scrubbed from the browser address bar immediately on load. Kiosk devices must be physically secured on employer premises and rotated if compromised or decommissioned.

4.3 **WebAuthn ("Face ID") Check-In:** Where WebAuthn attendance verification is enabled, biometric matching occurs locally on the User's own device authenticator (Apple Face ID / Touch ID / Windows Hello / Android BiometricPrompt). Only cryptographic public-key assertions are sent to the Platform; raw biometric images or templates are never transmitted to or stored by the Platform.

4.4 **Geofence and Network Evidence:** Depending on employer policy (`none`, `learn`, `warn`, `enforce`), check-in requests may evaluate device geolocation against configured workplace radii or evaluate server-observed egress IP prefixes against approved office networks. Check-out is never blocked by geofence or network enforcement so employees are never stranded in an open shift, though off-site check-outs may be flagged for HR review.

### 5. Payroll, Statutory Calculations, and Electronic Records

5.1 **Fail-Closed Statutory Safety Gate:** Automatic payroll calculation refuses to execute in production unless verified, source-linked rule sets (`status = 'verified'` with SHA-256 provenance) cover the payroll period's effective date. Automatic computerised PCB calculation remains disabled unless and until the deployment has passed **LHDN Computerised Calculation Method (_Spesifikasi Kaedah Pengiraan Berkomputer PCB_)** conformance testing and an authorized operator enables `pcb_calculator_status`.

5.2 **Separation of Duties:** Payroll preparation (`manage_payroll_draft`, `submit_payroll`) and financial approval/locking (`approve_payroll`, `mark_payroll_paid`) are separated by permission gates, and four-eyes controls prevent the preparer or submitter of a run from approving their own submission.

5.3 **Electronic Payslips and Statements:** Subject to **Section 61 of the Employment Act 1955** and **Subsection 83(1A) of the Income Tax Act 1967**, employees consent to receiving itemized monthly payslips and annual **Form EA (Borang C.P.8A)** statements electronically through the Employee Self-Service Portal and/or registered email. Employees should review payslips promptly and notify HR/Payroll of any discrepancy.

5.4 **Leave, Claims, and Overtime Submissions:** Supporting documents uploaded for medical leave, hospitalization leave, expense claims, or HR records must be genuine, unaltered, and business-related. Fraudulent claims or forged medical certificates may result in rejection, recovery of overpayments in accordance with **Section 24 of the Employment Act 1955**, and disciplinary or legal action.

5.5 **PWA & Web Push Notifications:** Where enabled by the deployment (`VAPID_*`) and explicitly granted by the User's browser permission prompt, the Platform delivers operational alerts (such as approval decisions, payslip readiness, or user-initiated test notifications) via standard W3C Web Push to subscribed devices. Users may unsubscribe or revoke browser notification permissions at any time.

### 6. Acceptable Use Restrictions

Users must **not**:

- Attempt to bypass tenant isolation (`company_id` scoping), role/permission guards, Cloudflare Turnstile bot protection, or rate limits;
- Upload malware, executable scripts, disguised file extensions, or files exceeding permitted size and magic-byte content checks;
- Submit internal, loopback, link-local, or private-network URLs to calendar import or Web Push subscription endpoints (SSRF probing);
- Scrape, bulk-exfiltrate, or disclose confidential payroll, NRIC/passport, banking, tax, or health records outside authorized business workflows; or
- Modify deployed database migrations in place or tamper with audit trail records (`audit_logs`).

### 7. Data Protection, Availability, Disclaimer, and Governing Law

7.1 Processing of personal data and sensitive personal data on the Platform is governed by [Part II: Privacy Policy and PDPA Notice](#part-ii-privacy-policy-and-pdpa-notice). Statutory employment and tax records (payroll runs, payslips, EA data, and audit trails) are retained for **7 years (2,555 days)** under **Section 82 of the Income Tax Act 1967** and **Section 61 of the Employment Act 1955**, while transient operational telemetry is purged automatically on shorter schedules.

7.2 Scheduled maintenance, database upgrades (including PostgreSQL major/beta migrations), or security patches may temporarily suspend access. Whole-company JSON backup exports contain confidential personal and financial data; administrators must encrypt, store, and transmit archives securely.

7.3 **Disclaimer & Liability:** TO THE MAXIMUM EXTENT PERMITTED BY APPLICABLE MALAYSIAN LAW, THE PLATFORM AND ITS OPEN-SOURCE SOFTWARE COMPONENTS ARE PROVIDED **"AS IS"** AND **"AS AVAILABLE"**, WITHOUT WARRANTIES OF ANY KIND, EXPRESS OR IMPLIED. IN NO EVENT SHALL THE AUTHORS, CONTRIBUTORS, OR OPEN-SOURCE MAINTAINERS BE LIABLE FOR ANY INDIRECT, INCIDENTAL, CONSEQUENTIAL, SPECIAL, EXEMPLARY, OR STATUTORY PENALTY DAMAGES (INCLUDING ANY LATE PAYMENT SURCHARGE, DIVIDEND, FINE, OR ASSESSMENT IMPOSED BY KWSP, PERKESO, LHDN, JTKSM, HRD CORP, OR THE PDP COMMISSIONER).

7.4 **Governing Law:** These Terms are governed by the **laws of Malaysia** and subject to the exclusive jurisdiction of the courts of Malaysia, without prejudice to statutory remedies before the **Director General of Labour (Jabatan Tenaga Kerja)**, the **Industrial Court of Malaysia**, or the **Personal Data Protection Commissioner**.

---

## Part II: Privacy Policy and PDPA Notice

# _Dasar Privasi & Notis Perlindungan Data Peribadi (PDPA)_

**Document Version / _Versi Dokumen_:** 2.1 (Aligned with _Personal Data Protection Act 2010 [Act 709]_ & _Personal Data Protection (Amendment) Act 2024 [Act A1727]_)

> **Statutory Bilingual Notice:** Pursuant to **Section 7(3) of the Malaysian Personal Data Protection Act 2010 (Act 709)**, this Personal Data Protection Notice is issued in both **English** (Section A) and the **National Language / Bahasa Malaysia** (Section B).

### Section A — Privacy Policy & Personal Data Protection Notice (English)

#### 1. Introduction and Roles (Data Controller vs. Data Processor)

This Notice explains how personal data and sensitive personal data are collected, used, disclosed, retained, and protected within the **Payroll System** (**"Platform"**) in compliance with the **Personal Data Protection Act 2010 (Act 709)** as amended by the **Personal Data Protection (Amendment) Act 2024 (Act A1727)** (fully in force across its January, April, and 1 June 2025 commencement phases, collectively the **"PDPA"**).

- **Data Controller (_Pengawal Data_):** Your employing organization (`company`) is the **Data Controller** that determines the purposes and means of processing your employment, attendance, leave, claims, and payroll data.
- **Data Processor (_Pemproses Data_):** Where the Platform infrastructure is operated on behalf of the Data Controller, the hosting operator acts as a **Data Processor** and is directly bound by the **Security Principle under Section 9 of the PDPA**.

#### 2. Categories of Personal Data Processed

Depending on your role and enabled workflows, the Platform processes:

1. **Identity & Demographic Data:** Full name, employee number, NRIC number or passport/work-pass number, date of birth, gender, nationality, race (optional statistical reporting), residency status (`citizen`, `permanent_resident`, `foreigner`), marital status, spouse working status, and number of eligible children.
2. **Contact Data:** Work/personal email address, telephone number, and residential address.
3. **Employment & Organizational Data:** Department, designation, branch, cost centre, employment type, join/probation/confirmation/resignation dates, work schedule, team membership, and HR letters/documents.
4. **Compensation, Banking & Statutory Data:** Basic salary, salary change history, allowances, overtime records, expense claims, bank name, bank account number and type, Income Tax TIN, EPF (KWSP) number and Third Schedule category, SOCSO (PERKESO) number, EIS number, Form TP3 prior-employment figures, Zakat, PTPTN, and Tabung Haji deduction instructions.
5. **Attendance, Location & Network Signals:** Check-in/check-out timestamps, QR scan records, GPS coordinates when geofence verification (`warn` or `enforce`) is enabled, and server-observed egress IP prefixes when office-network binding (`learn`, `warn`, or `enforce`) is active.
6. **Authentication, Security & Notification Data:** Bcrypt password hashes, AES-GCM encrypted TOTP 2FA secrets (under a dedicated `TOTP_ENCRYPTION_KEY`), keyed SHA-256 backup recovery code hashes, WebAuthn public-key credentials, Google OAuth2 subject links, session device/IP metadata, Cloudflare Turnstile verification tokens, immutable `audit_logs`, and (if subscribed) Web Push subscription endpoints, P-256 ECDH public keys, auth secrets, and user-agent strings.
7. **Sensitive Personal Data (Section 4 PDPA):**
   - _Health / Medical Data:_ Outpatient sick leave and hospitalization details or medical certificates uploaded to support leave or medical claim approvals.
   - _Religious Indicators:_ Muslim/Zakat eligibility flags where required to process voluntary or state statutory Zakat salary deductions and tax rebates.
   - _Biometric Boundary Clarification:_ Under the PDPA (Amendment) Act 2024 (effective 1 April 2025), **biometric data** constitutes Sensitive Personal Data. When you use Passkeys or **"Face ID" Attendance** on the Platform, biometric unlocking occurs strictly inside your own device's hardware enclave via W3C WebAuthn; **the Platform never collects, transmits, or stores facial images, fingerprints, or biometric templates**.

#### 3. Purposes and Lawful Basis of Processing

Your personal data is processed strictly for:

- Administering your contract of service, calculating monthly wages, overtime, proration (**EA 1955 s.18B**), and reimbursements;
- Complying with Malaysian statutory obligations under the **Employment Act 1955**, **Minimum Wages Order 2024**, **EPF Act 1991** (including Part F foreign-worker contributions), **Employees' Social Security Act 1969**, **EIS Act 2017**, **Income Tax Act 1967** (PCB/MTD & Form EA), and **PSMB Act 2001 (HRD Corp)**;
- Verifying workplace attendance, administering leave entitlements and approvals, delivering operational/push notifications, and maintaining occupational/audit records;
- Enforcing authentication, fraud/bot prevention, tenant isolation, and information security; and
- Generating statutory exports, bank payment files, management reports, and audit evidence.

Explicit consent is obtained where sensitive personal data is processed outside direct statutory obligations (Section 40 PDPA). Failure to supply mandatory identity, banking, date-of-birth, or statutory registration data may prevent your employer from onboarding you, rating age-based SOCSO/EIS/EPF contributions, disbursing salary by bank transfer, or fulfilling statutory filings.

#### 4. Disclosure, Sub-Processors, and Cross-Border Transfers (Section 129 PDPA)

Personal data is never sold. Within your employer's tenant scope and strictly as necessary, data may be disclosed to:

- **Malaysian Statutory Authorities:** KWSP (EPF), PERKESO (SOCSO & EIS), LHDN (e-PCB Plus / MyTax / CP39 / Form EA), Jabatan Tenaga Kerja (JTK), and HRD Corp;
- **Financial Institutions:** Licensed banks processing approved payroll disbursement files and statutory Zakat/PTPTN/Tabung Haji bodies;
- **Authorized Infrastructure & Delivery Sub-Processors:**
  - _Cloudflare, Inc._ — edge reverse proxy and Turnstile bot protection on unauthenticated sign-in/recovery endpoints;
  - _Google LLC_ — solely if your organization enables optional Google OAuth2 sign-in/account linking;
  - _Configured SMTP Provider_ — for password reset links, welcome notices (with credentials redacted from stored logs), HR letters, and payslip notifications; and
  - _Browser Push Services (e.g., Apple APNs, Google FCM, Mozilla Autopush)_ — solely when a User explicitly subscribes to Web Push notifications (`push_outbox` payloads are RFC 8291 end-to-end encrypted to the browser's `p256dh`/`auth` keys).

Under the **1 April 2025** amendment to **Section 129 of the PDPA**, personal data may only be transferred outside Malaysia if the receiving jurisdiction provides substantially similar / adequate protection, or where a statutory condition (such as contractual necessity, explicit consent, or binding safeguards) applies.

#### 5. Security Controls & Privacy-by-Design Safeguards (Section 9 PDPA)

- TLS transport encryption, `httpOnly` rotated refresh cookies with reuse detection, in-memory-only JWT access tokens, and AES-GCM encrypted TOTP secrets;
- Strict multi-tenant `company_id` predicates, composite `(parent_id, company_id)` foreign keys, immutable `company_id` triggers, and capability-based permission guards (`exec` denied all payroll figures; `manager` excluded from payroll & privileged-user visibility; four-eyes payroll approval);
- Per-user reverse-reference authorization on uploaded receipts and medical attachments (`GET /api/uploads/{filename}` returns uniform `404` on denial), magic-byte validation, and single-component path containment;
- Privacy-preserving office-network learning (`attendance_network_observations` exposes no `employee_id → IP address` mapping); and
- Redaction of initial/temporary passwords from stored `email_logs` and structured application logs (`core::redact::email`).

#### 6. Data Retention Schedule (Section 10 PDPA)

The Platform enforces automated daily retention purges in `backend/src/main.rs` alongside statutory archival horizons:

| Record Category                                               | Retention Period                                | Statutory / Operational Basis                                               |
| :------------------------------------------------------------ | :---------------------------------------------- | :-------------------------------------------------------------------------- |
| Payroll runs, payslips, EA records, statutory exports         | **7 years** minimum                             | _Income Tax Act 1967 s.82_ (7 years) & _Employment Act 1955 s.61_ (6 years) |
| Security & workflow `audit_logs`                              | **2,555 days (~7 years)**                       | Automated daily purge; `user_id` set to `NULL` on account deletion          |
| Attendance network observations                               | **`OBSERVATION_RETENTION_DAYS`**                | Short rolling window purged daily; PDPA minimization for egress IP prefixes |
| Email delivery logs (`email_logs`)                            | **180 days**                                    | Operational delivery verification; sensitive body secrets redacted          |
| User sessions (`user_sessions`)                               | **30 days** (revoked) / **90 days** (inactive)  | Automated daily purge                                                       |
| Expired/revoked `refresh_tokens` & `password_reset_requests`  | **30 days**                                     | Automated daily purge                                                       |
| Unreferenced expired QR tokens                                | **7 days**                                      | Automated daily purge; tokens linked to check-ins retained with attendance  |
| Portal notifications                                          | **90 days** (read) / **365 days** (unread)      | Automated daily purge                                                       |
| Web Push subscriptions (`push_subscriptions`)                 | **30 days** (revoked) / **180 days** (inactive) | Automated daily purge; `404`/`410` endpoints auto-revoked                   |
| Web Push delivery queue (`push_outbox`)                       | **30 days**                                     | Automated daily purge of aged delivery records                              |
| Unreferenced uploads (`uploaded_files`)                       | **24 hours**                                    | Automated daily purge of orphaned pre-attachment uploads                    |
| Finished `background_jobs`                                    | **30 days**                                     | Automated daily purge                                                       |
| `bulk_import_sessions`, `passkey_challenges`, `oauth2_states` | **Upon expiry**                                 | Automated daily purge                                                       |

#### 7. Governance (DPO, 72-Hour DBN) and Data Subject Rights

- **Data Protection Officer (Section 12A):** Employers meeting the prescribed threshold (>20,000 data subjects, >10,000 sensitive data subjects, or regular/systematic monitoring) must appoint a qualified Malaysia-resident DPO registered with the PDP Commissioner and publish business contact details.
- **Mandatory Data Breach Notification (Section 12B):** Where a personal data breach causes or is likely to cause **significant harm**, the Data Controller must notify the **PDP Commissioner (`dbnpdp@pdp.gov.my` / official portal) within 72 hours**, and notify affected individuals **without unnecessary delay** (within 7 days of notifying the Commissioner).
- **Your Rights (Sections 30–43A):** Access (`s.30`), Correction (`s.34`), Withdrawal of Consent (`s.38`), Prevention of Processing Causing Damage/Distress (`s.42`), and **Data Portability (`s.43A`)**. Direct requests to your employing organization's **HR Department or DPO** (responses processed within **21 days**).

---

### Section B — Notis Perlindungan Data Peribadi (Bahasa Malaysia)

#### 1. Pengenalan dan Peranan (_Pengawal Data_ & _Pemproses Data_)

Notis ini dikeluarkan selaras dengan **Akta Perlindungan Data Peribadi 2010 [Akta 709]** sebagaimana yang dipinda oleh **Akta Perlindungan Data Peribadi (Pindaan) 2024 [Akta A1727]** (**"PDPA"**) untuk menerangkan bagaimana data peribadi dan data peribadi sensitif anda dikumpulkan, diproses, dizahirkan, disimpan, dan dilindungi dalam **Payroll System** (**"Platform"**).

- **Pengawal Data (_Data Controller_):** Majikan anda (`company`) ialah **Pengawal Data** yang menentukan tujuan dan cara pemprosesan data pekerjaan, kehadiran, cuti, tuntutan, dan penggajian anda.
- **Pemproses Data (_Data Processor_):** Pengendali infrastruktur Platform bertindak sebagai **Pemproses Data** dan tertakluk secara langsung kepada **Prinsip Keselamatan di bawah Seksyen 9 PDPA**.

#### 2. Kategori Data Peribadi Yang Diproses

1. **Data Pengenalan & Demografi:** Nama penuh, nombor pekerja, nombor MyKad (NRIC) atau pasport/permit kerja, tarikh lahir, jantina, kewarganegaraan, bangsa (pilihan), status pemastautin (`citizen`, `permanent_resident`, `foreigner`), status perkahwinan, status pekerjaan pasangan, dan bilangan anak layak.
2. **Data Perhubungan:** Alamat e-mel, nombor telefon, dan alamat kediaman.
3. **Data Pekerjaan & Organisasi:** Jabatan, jawatan, cawangan, pusat kos, jenis pekerjaan, tarikh mula kerja/percubaan/pengesahan/peletakan jawatan, jadual kerja, dan dokumen HR.
4. **Data Gaji, Perbankan & Berkanun:** Gaji pokok, sejarah pelarasan gaji, elaun, kerja lebih masa (OT), tuntutan perbelanjaan, nama bank dan nombor akaun, TIN cukai pendapatan, nombor dan kategori Jadual Ketiga KWSP (EPF), nombor PERKESO (SOCSO), nombor SIP (EIS), rekod Borang TP3, Zakat, PTPTN, dan Tabung Haji.
5. **Data Kehadiran, Lokasi, Rangkaian & Pemberitahuan:** Masa daftar masuk/keluar, imbasan QR, koordinat GPS (apabila mod geofence `warn`/`enforce` diaktifkan), awalan IP rangkaian pejabat, serta titik akhir langganan Web Push (jika pengguna memilih untuk mengaktifkannya).
6. **Data Peribadi Sensitif (Seksyen 4 PDPA):** Sijil cuti sakit/penghospitalan atau resit perubatan yang dimuat naik bagi tujuan kelulusan cuti/tuntutan, serta penanda kelayakan Zakat. **Data biometrik** (seperti imbasan wajah/cap jari pada peranti anda melalui WebAuthn / "Face ID") diproses sepenuhnya di dalam peranti anda sendiri dan **tidak pernah dihantar atau disimpan di dalam pelayan Platform**.

#### 3. Tujuan Pemprosesan Data Peribadi

Data peribadi anda diproses bagi tujuan:

- Pentadbiran kontrak perkhidmatan, pengiraan gaji bulanan, kerja lebih masa, prorata gaji (**Seksyen 18B Akta Kerja 1955**), dan pembayaran balik tuntutan;
- Pematuhan kewajipan undang-undang di bawah **Akta Kerja 1955** (serta Ordinan Buruh Sabah & Sarawak Pindaan 2025), **Perintah Gaji Minimum 2024**, **Akta KWSP 1991** (termasuk caruman mandatori pekerja asing Bahagian F), **Akta Keselamatan Sosial Pekerja 1969**, **Akta SIP 2017**, **Akta Cukai Pendapatan 1967** (PCB/MTD & Borang EA), dan **Akta PSMB 2001 (HRD Corp)**;
- Pengesahan kehadiran, pengurusan cuti dan kelulusan, penyampaian pemberitahuan aplikasi/tolak Web Push, kawalan keselamatan pengesahan dua faktor (2FA/Passkey), serta jejak audit pematuhan.

Kegagalan membekalkan data mandatori (seperti tarikh lahir, nombor pengenalan, maklumat akaun bank, atau nombor berkanun) boleh menjejaskan pengiraan caruman berkanun berasaskan umur, pembayaran gaji melalui pindahan bank, atau pemfailan berkanun majikan.

#### 4. Penzahiran Data Peribadi & Pemindahan Rentas Sempadan

Data peribadi anda tidak akan dijual dan hanya dizahirkan mengikut keperluan undang-undang serta operasi kepada **Agensi Berkanun Malaysia** (KWSP, PERKESO, LHDN e-PCB Plus / MyTax, JTK, HRD Corp), **Institusi Kewangan** (bank berlesen, Zakat, PTPTN, Tabung Haji), dan **Sub-Pemproses Infrastruktur** (Cloudflare, Google OAuth2 jika diaktifkan, penyedia SMTP majikan, serta perkhidmatan Web Push pelayar seperti APNs/FCM apabila dilanggan oleh pengguna dengan muatan disulitkan hujung ke hujung). Sebarang pemindahan data ke luar Malaysia mematuhi **Seksyen 129 PDPA**.

#### 5. Keselamatan, Penyimpanan Data & Pemberitahuan Pelanggaran Data (72 Jam)

- **Keselamatan & Penyimpanan (Seksyen 9 & 10):** Data gaji, cukai, dan log audit disimpan selama **7 tahun (2,555 hari)** selaras dengan **Seksyen 82 Akta Cukai Pendapatan 1967** dan **Seksyen 61 Akta Kerja 1955**, manakala data operasi sementara (pemerhatian rangkaian kehadiran, sesi, token, log e-mel, fail tidak dirujuk, langganan serta baris gilir Web Push) dipadamkan secara automatik mengikut jadual pengekalan harian.
- **Pegawai Perlindungan Data (DPO — Seksyen 12A) & Pemberitahuan Pelanggaran Data (DBN — Seksyen 12B):** Majikan yang memenuhi nilai ambang ditetapkan hendaklah melantik **DPO** dan memaklumkan **Pesuruhjaya Perlindungan Data Peribadi dalam tempoh 72 jam** sekiranya berlaku pelanggaran data peribadi yang boleh mendatangkan kemudaratan ketara.

#### 6. Hak Subjek Data & Cara Menghubungi Kami

Di bawah **Seksyen 30 hingga 43A PDPA**, anda berhak untuk mengakses dan menyemak data (`Seksyen 30`), memohon pembetulan (`Seksyen 34`), menarik balik persetujuan pilihan (`Seksyen 38`), menghalang pemprosesan yang menyebabkan kemudaratan (`Seksyen 42`), serta memohon **kemudahalihan data (_data portability_)** (`Seksyen 43A`). Kemukakan permohonan kepada **Jabatan Sumber Manusia (HR) atau DPO** majikan anda.

---

## Part III: Malaysian Statutory Compliance Guide (2026)

This guide maps Malaysian statutes, subsidiary orders, agency specifications, contribution ceilings, and compliance rules applicable in **2025–2026** across Peninsular Malaysia, Federal Territories, Sabah, and Sarawak to how this repository implements, enforces, or fail-closes them.

> [!IMPORTANT]
> The repository's seeded `legacy-prototype-*` rows in `1001_data.sql` are academic fixtures (`status = 'prototype'`). Production payroll **fails closed** until an organization imports complete, official schedules from KWSP, PERKESO, and LHDN, verifies their SHA-256 provenance in `statutory_rule_sets`, and passes LHDN computerised-MTD conformance testing before enabling `pcb_calculator_status`.

### 1. Employment Legislation & Labour Standards

- **Peninsular Malaysia & Labuan:** _Employment Act 1955 (Act 265)_, incorporating the _Employment (Amendment) Act 2022 (Act A1651)_.
- **Sabah:** _Labour Ordinance of Sabah (Cap. 67)_, as amended by the _Labour Ordinance of Sabah (Amendment) Act 2025_ (**in force 1 May 2025**).
- **Sarawak:** _Labour Ordinance of Sarawak (Cap. 76)_, as amended by the _Labour Ordinance of Sarawak (Amendment) Act 2025_ (**in force 1 May 2025**).

Effective **1 May 2025**, Sabah and Sarawak removed their legacy RM2,500 coverage ceiling and aligned core labour protections—45-hour maximum work week, 98 days paid maternity leave, 7 days paid paternity leave, separate 60-day hospitalization leave, flexible working arrangement procedures, and anti-discrimination protections—with the _Employment Act 1955_.

| Statutory Provision                                           | Requirement (2025–2026)                                                                                                                                                                                             | Implementation in Payroll System                                                                                                                                          |
| :------------------------------------------------------------ | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **First Schedule Coverage**                                   | Universal coverage under contract of service regardless of wage; statutory OT & termination-benefit entitlements remain mandatory up to **RM4,000/month** wages                                                     | Supported for all employees; OT applications and attendance OT configurable per company                                                                                   |
| **Working Hours (s.60A(1))**                                  | Maximum **45 working hours per week** (exclusive of unpaid meal/rest breaks)                                                                                                                                        | Default settings: `effective_hours_per_day = 8`, `rest_time_minutes = 60`, `unpaid_break_minutes = 60` (`1024_unpaid_break_minutes.sql`)                                  |
| **ORP & OT Multipliers (s.60I, s.60A(3), s.60(3), s.60D(3))** | Monthly ORP = Monthly Wages ÷ 26; HRP = ORP ÷ 8. Multipliers: **1.5×** normal, **2.0×** rest day, **3.0×** public holiday                                                                                           | `OvertimeSettings::rate_overtime` uses `working_days_per_month` (`26`), `effective_hours_per_day` (`8`), and `1.5`/`2.0`/`3.0` multipliers (`1022`) using `Decimal`       |
| **Incomplete Month Proration (s.18A / s.18B)**                | Prorated wage = (Monthly Wages ÷ Calendar days in wage period) × Eligible calendar days                                                                                                                             | `payroll_engine::compute_payslip` prorates both `basic_salary` and overlapping recurring allowances by exact calendar days                                                |
| **Payment of Wages (s.19 & s.25A)**                           | Paid **not later than the 7th day** after wage period end, via bank account                                                                                                                                         | Default `payment_day = 28` (cutoff `25`); `missing_bank_account` preview warning; bank CSV export in `bank_export_service`                                                |
| **Statutory Paid Leave Minimums**                             | **Annual (s.60E):** 8 / 12 / 16 days<br>**Sick (s.60F):** 14 / 18 / 22 days + **60** hospitalisation<br>**Maternity (s.37):** **98** days<br>**Paternity (s.60FA):** **7** days<br>**Public Holidays (s.60D):** ≥11 | Provisioned in `provision_company_defaults` (`Maternity = 98`, `Paternity = 7`, `Hospitalisation = 60`, `Sick = 14`, `Annual = 14`); balance reservation in `leave_rules` |
| **Register & Payslip Retention (s.61)**                       | Itemized wage statements and retention for **at least 6 years**                                                                                                                                                     | Itemized `payroll_item_details`; PDF payslips in `payslip_pdf_service.rs`; 7-year (`2,555`-day) retention floor in `main.rs`                                              |

### 2. National Minimum Wage — _Minimum Wages Order 2024_

- **Governing Law:** _National Wages Consultative Council Act 2011 (Act 732)_ & _Minimum Wages Order 2024 [P.U. (A) 376/2024]_.
- **Rates:** **RM1,700.00/month** (effective 1 Feb 2025 for ≥5 employees / MASCO employers; universal across all employers from 1 Aug 2025). Equivalent statutory rates: **RM8.72/hour**, **RM78.46/day** (5-day), **RM65.38/day** (6-day), **RM98.08/day** (4-day). Statutory exclusions: domestic employees and statutory apprenticeship contracts.
- **Codebase Enforcement:** `payroll_engine::preview_payroll` evaluates `statutory_minimum_wage_sen(period.effective_date)` (`170_000` sen from `2025-02-01`) and raises `below_minimum_wage` for full-time non-intern employees below the floor; `company_settings` seeds `statutory/minimum_wage_monthly = "1700"` (`1029_epf_part_f_and_statutory_defaults.sql`).

### 3. EPF / KWSP — _Employees Provident Fund Act 1991 (Act 452)_

- _EPF Act 1991 (Act 452)_ **s.2** (wages), **s.43(1)** (age-75 cessation), **Third Schedule (Parts A–F)**, and _EPF (Amendment) Act 2025_ (effective **1 October 2025**, introducing mandatory **Part F** for non-citizen employees with valid work passes; Part F remains active statutory law unless amended by gazette).
- Under-55 member contributions allocate across **Akaun Persaraan (75%)**, **Akaun Sejahtera (15%)**, and **Akaun Fleksibel (10%)**.

| Part       | Eligible Employee Category                                                                                 | Age Bracket | Standard Statutory Rate Reference                                                  |
| :--------- | :--------------------------------------------------------------------------------------------------------- | :---------: | :--------------------------------------------------------------------------------- |
| **A**      | Malaysian Citizens, PRs, & non-citizens registered before 1 Aug 1998                                       |  Below 60   | **11%** employee; **13%** employer (≤ RM5,000) / **12%** employer (> RM5,000)      |
| **B**      | Non-citizens registered on/after 1 Aug 1998 (pre-Oct 2025 / voluntary legacy election)                     |  Below 60   | **11%** employee; **RM5.00** employer                                              |
| **C**      | PRs & non-citizens registered before 1 Aug 1998                                                            |  60 to 74   | **5.5%** employee; **6.5%** employer (≤ RM5,000) / **6.0%** employer (> RM5,000)   |
| **D**      | Non-citizens registered on/after 1 Aug 1998 (pre-Oct 2025 / voluntary legacy election)                     |  60 to 74   | **5.5%** employee; **RM5.00** employer                                             |
| **E**      | **Malaysian Citizens**                                                                                     |  60 to 74   | **0%** employee; **4.0%** employer                                                 |
| **F**      | **Non-Malaysian Citizens holding valid work passes** (excludes domestic servants) — _effective 1 Oct 2025_ |  Below 75   | **2.0%** employee + **2.0%** employer (fractions rounded up to next whole ringgit) |
| **Exempt** | Any employee who has attained **75 years of age** (_s.43(1)_)                                              |     75+     | **RM0** employee / **RM0** employer                                                |

- **Wage Base & Remittance:** Excludes overtime payments (`s.2`), gratuities, retirement/termination benefits, and travelling allowances (`payroll_engine.rs` computes `epf_wage` excluding `total_overtime` and claims, reduced for unpaid leave). Due on/before the **15th of the following month** via KWSP i-Akaun.
- **Codebase Enforcement:** `epf_service::derive_category_for` derives `A`–`F` / `EXEMPT`; `employees_epf_category_check` (`1029`) allows `A`–`F`, and `1033_audit_bugfixes.sql` removes the `'A'` default so omitted categories auto-derive cleanly; `preview_payroll` flags `epf_category_override` when an override differs from the derived part.

### 4. SOCSO & EIS (PERKESO) — _Act 4 & Act 800_

- **SOCSO:** _Employees' Social Security Act 1969 (Act 4)_ & _P.U. (A) 255/2024_. **EIS (SIP):** _Employment Insurance System Act 2017 (Act 800)_ & _P.U. (A) 256/2024_.
- **RM6,000 Ceiling (effective 1 Oct 2024):** `StatutoryTables::socso_band` and `eis_band` clamp insurable wages at `600_000` sen (including overtime under Act 4 s.2 / Act 800 s.2).
- **Foreign Workers & Age Gates:** Foreign workers under 60 fall under SOCSO First Category (from 1 July 2024; Second Category at 60+ or ≥55 first entry; _Lindung 24 Jam / SKBBK_ 0.75% non-employment coverage introduced June 2026) and are exempt from EIS (citizens/PRs 18–59 only). `socso_service.rs` and `eis_service.rs` fail closed on missing DOB, ages 55–59 (SOCSO), ages 57–59 (EIS), and foreign-worker PERKESO entries pending entry-age metadata. Remittance is due by the **15th of the following month** via PERKESO ASSIST.

### 5. Income Tax (PCB / MTD & Form EA) — _Income Tax Act 1967 (Act 53)_

- **Statute & Portal:** _Income Tax Act 1967_ **s.82** (7-year retention), **s.83(1)/83(1A)** (Form E/C.P.8D & Form EA/C.P.8A), **s.107C / MTD Rules 1994**, _LHDN Spesifikasi Kaedah Pengiraan Berkomputer PCB 2026_, and MyTax **e-PCB Plus** (replaces legacy e-PCB/e-Data PCB/e-CP39).
- **Resident Brackets (YA 2025 & YA 2026):** 0% (0–5k), 1% (5k–20k), 3% (20k–35k), 6% (35k–50k), 11% (50k–70k), 19% (70k–100k), 25% (100k–400k), 26% (400k–600k), 28% (600k–2M), 30% (>2M); **RM400 rebate** (`s.6A(2)`) at chargeable income ≤ RM35,000; ringgit-for-ringgit **Zakat rebate** (`s.6A(3)`).
- **Key Resident Tax Reliefs (YA 2025 & YA 2026):** Individual RM9,000; Non-working spouse RM4,000; Disabled individual RM7,000 / disabled spouse RM6,000; Child <18 RM2,000 / 18+ higher ed RM8,000 / disabled child RM8,000; EPF RM4,000 + Life Insurance/Takaful RM3,000 (RM7,000 group); SOCSO+EIS `s.49(1D)` RM350; Medical self/spouse/child RM10,000; Parents/grandparents medical & carer RM8,000; Education self RM7,000; Lifestyle RM2,500 + Sports RM1,000; First-home loan interest RM7,000/RM5,000 (SPA 2025–2027).
- **Deadlines & Codebase Posture:** Monthly PCB due **15th of following month** via e-PCB Plus; **Form EA** due **last day of February** (`ea_form_service.rs`); **Form E & C.P.8D** due **31 March** (electronic grace usually 30 April). `1033_audit_bugfixes.sql` adds `payroll_items.taxable_gross` so YTD PCB and EA forms exclude unpaid leave and non-taxable allowances. Automatic PCB stays **fail-closed** until official LHDN conformance vectors pass.

### 6. HRD Corp Levy — _PSMB Act 2001 (Act 612)_

Mandatory **1.0%** monthly levy on basic salary + fixed allowances for covered employers with ≥10 Malaysian citizen employees (optional **0.5%** for 5–9 Malaysian employees; foreign workers excluded). Due by the **15th of the following month** via HRD Corp e-TRiS. `company_settings` stores `statutory/hrdf_enabled` and `statutory/hrdf_rate`, though automated payroll run booking remains on the roadmap.

### 7. Personal Data Protection & Cybersecurity — _PDPA 2010 / 2024 & Act 854_

_PDPA 2010 (Act 709)_ as amended by _Act A1727_ completed phased enforcement in 2025 (Data Controller/Processor Security Principle liability, biometric sensitive data, Section 129 cross-border rules, penalties up to RM1,000,000 / 3 years, mandatory DPO `s.12A`, 72-hour DBN `s.12B`, and data portability `s.43A`), alongside the _Cyber Security Act 2024 (Act 854)_ for designated NCII entities. See [Part II](#part-ii-privacy-policy-and-pdpa-notice) for the full bilingual notice.

### 8. Official Malaysian Statutory Sources

- **KWSP / EPF:** <https://www.kwsp.gov.my/en/employer/responsibilities/mandatory-contribution>
- **PERKESO / SOCSO & EIS:** <https://www.perkeso.gov.my/en/rate-of-contribution.html>
- **PERKESO Foreign Workers:** <https://www.perkeso.gov.my/en/our-services/protection/foreign-worker>
- **LHDN / HASiL (e-PCB Plus & 2026 MTD Spec):** <https://www.hasil.gov.my/en/employers/> and <https://mytax.hasil.gov.my>
- **JTKSM & Minimum Wages:** <https://jtksm.mohr.gov.my> and <https://gajiminimum.mohr.gov.my>
- **HRD Corp & JPDP (PDPA):** <https://hrdcorp.gov.my> and <https://www.pdp.gov.my>
