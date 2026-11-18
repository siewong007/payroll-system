# Privacy Policy & PDPA Personal Data Protection Notice

# _Dasar Privasi & Notis Perlindungan Data Peribadi (PDPA)_

**Effective Date / _Tarikh Kuat Kuasa_:** 18 November 2026  
**Document Version / _Versi Dokumen_:** 2.0 (Aligned with _Personal Data Protection Act 2010 [Act 709]_ & _Personal Data Protection (Amendment) Act 2024 [Act A1727]_)

> **Statutory Bilingual Notice:** Pursuant to **Section 7(3) of the Malaysian Personal Data Protection Act 2010 (Act 709)**, this Personal Data Protection Notice is issued in both **English** (Part A) and the **National Language / Bahasa Malaysia** (Part B).

---

## Part A — Privacy Policy & Personal Data Protection Notice (English)

### 1. Introduction and Roles (Data Controller vs. Data Processor)

This Notice explains how personal data and sensitive personal data are collected, used, disclosed, retained, and protected within the **Payroll System** (**"Platform"**) in compliance with the **Personal Data Protection Act 2010 (Act 709)** as amended by the **Personal Data Protection (Amendment) Act 2024 (Act A1727)** (fully in force across its January, April, and 1 June 2025 commencement phases, collectively the **"PDPA"**).

- **Data Controller (_Pengawal Data_):** Your employing organization (`company`) is the **Data Controller** that determines the purposes and means of processing your employment, attendance, leave, claims, and payroll data.
- **Data Processor (_Pemproses Data_):** Where the Platform infrastructure is operated on behalf of the Data Controller, the hosting operator acts as a **Data Processor** and is directly bound by the **Security Principle under Section 9 of the PDPA**.

### 2. Categories of Personal Data Processed

Depending on your role and enabled workflows, the Platform processes:

1. **Identity & Demographic Data:** Full name, employee number, NRIC number or passport/work-pass number, date of birth, gender, nationality, race (optional statistical reporting), residency status (`citizen`, `permanent_resident`, `foreigner`), marital status, spouse working status, and number of eligible children.
2. **Contact Data:** Work/personal email address, telephone number, and residential address.
3. **Employment & Organizational Data:** Department, designation, branch, cost centre, employment type, join/probation/confirmation/resignation dates, work schedule, team membership, and HR letters/documents.
4. **Compensation, Banking & Statutory Data:** Basic salary, salary change history, allowances, overtime records, expense claims, bank name, bank account number and type, Income Tax TIN, EPF (KWSP) number and Third Schedule category, SOCSO (PERKESO) number, EIS number, Form TP3 prior-employment figures, Zakat, PTPTN, and Tabung Haji deduction instructions.
5. **Attendance, Location & Network Signals:** Check-in/check-out timestamps, QR scan records, GPS coordinates when geofence verification (`warn` or `enforce`) is enabled, and server-observed egress IP prefixes when office-network binding (`learn`, `warn`, or `enforce`) is active.
6. **Authentication & Security Data:** Bcrypt password hashes, AES-GCM encrypted TOTP 2FA secrets (under a dedicated `TOTP_ENCRYPTION_KEY`), backup recovery code hashes, WebAuthn public-key credentials, Google OAuth2 subject links, session device/IP metadata, Cloudflare Turnstile verification tokens, and immutable security/workflow `audit_logs`.
7. **Sensitive Personal Data (Section 4 PDPA):**
   - _Health / Medical Data:_ Outpatient sick leave and hospitalization details or medical certificates uploaded to support leave or medical claim approvals.
   - _Religious Indicators:_ Muslim/Zakat eligibility flags where required to process voluntary or state statutory Zakat salary deductions and tax rebates.
   - _Biometric Boundary Clarification:_ Under the PDPA (Amendment) Act 2024 (effective 1 April 2025), **biometric data** constitutes Sensitive Personal Data. When you use Passkeys or **"Face ID" Attendance** on the Platform, biometric unlocking occurs strictly inside your own device's hardware enclave via the W3C WebAuthn standard; **the Platform never collects, transmits, or stores facial images, fingerprints, or biometric templates**.

### 3. Purposes and Lawful Basis of Processing

Your personal data is processed strictly for:

- Administering your contract of service, calculating monthly wages, overtime, proration (**EA 1955 s.18B**), and reimbursements;
- Complying with Malaysian statutory obligations under the **Employment Act 1955**, **Minimum Wages Order 2024**, **EPF Act 1991** (including Part F foreign-worker contributions), **Employees' Social Security Act 1969**, **EIS Act 2017**, **Income Tax Act 1967** (PCB/MTD & Form EA), and **PSMB Act 2001 (HRD Corp)**;
- Verifying workplace attendance, administering leave entitlements and approvals, and maintaining occupational/audit records;
- Enforcing authentication, fraud/bot prevention, tenant isolation, and information security; and
- Generating statutory exports, bank payment files, management reports, and audit evidence.

Explicit consent is obtained where sensitive personal data is processed outside direct statutory obligations (Section 40 PDPA). Failure to supply mandatory identity, banking, date-of-birth, or statutory registration data may prevent your employer from onboarding you, rating age-based SOCSO/EIS/EPF contributions, disbursing salary by bank transfer, or fulfilling statutory filings.

### 4. Disclosure and Third-Party Recipients

Personal data is never sold. Within your employer's tenant scope and strictly as necessary, data may be disclosed to:

- **Malaysian Statutory Authorities:** KWSP (EPF), PERKESO (SOCSO & EIS), LHDN (e-PCB Plus / MyTax / CP39 / Form EA), Jabatan Tenaga Kerja (JTK), and HRD Corp;
- **Financial Institutions:** Licensed banks processing approved payroll disbursement files and statutory Zakat/PTPTN/Tabung Haji bodies;
- **Authorized Infrastructure & Security Sub-Processors:**
  - _Cloudflare, Inc._ — edge reverse proxy and Turnstile bot protection on unauthenticated sign-in/recovery endpoints;
  - _Google LLC_ — solely if your organization enables optional Google OAuth2 sign-in/account linking;
  - _Configured SMTP Provider_ — for password reset links, welcome notices (with credentials redacted from stored logs), HR letters, and payslip notifications.

### 5. Cross-Border Data Transfers (Section 129 PDPA)

Following the **1 April 2025** amendment to **Section 129 of the PDPA**, personal data may only be transferred outside Malaysia if the receiving jurisdiction has substantially similar data protection laws or provides an adequate level of protection, or where the transfer falls within an applicable statutory condition (such as contractual necessity, explicit consent, or binding technical and contractual safeguards with sub-processors).

### 6. Security Controls & Privacy-by-Design Safeguards

In accordance with **Section 9 of the PDPA**, both Data Controllers and Data Processors implement technical and organizational safeguards including:

- TLS transport encryption, `httpOnly` rotated refresh cookies, in-memory-only JWT access tokens, and AES-encrypted TOTP secrets;
- Strict multi-tenant `company_id` database predicates, composite foreign keys, and capability-based permission guards (`exec` role denied all payroll figures; four-eyes payroll approval);
- Per-user reverse-reference authorization on uploaded receipts and medical attachments (`GET /api/uploads/{filename}` returns uniform `404` on unauthorized access), magic-byte validation, and canonical path containment;
- Privacy-preserving office-network learning: `attendance_network_observations` aggregates candidate prefixes without exposing any `employee_id → IP address` mapping through API endpoints; and
- Redaction of initial/temporary passwords from stored `email_logs`.

### 7. Data Retention Schedule (Section 10 PDPA)

Personal data is not kept longer than necessary for its lawful or statutory purpose. The Platform enforces automated daily retention purges alongside statutory archival horizons:

| Record Category                                       | Retention Period                               | Statutory / Operational Basis                                                                               |
| :---------------------------------------------------- | :--------------------------------------------- | :---------------------------------------------------------------------------------------------------------- |
| Payroll runs, payslips, EA records, statutory exports | **7 years** minimum                            | _Income Tax Act 1967 s.82_ (7 years) & _Employment Act 1955 s.61_ (6 years)                                 |
| Security & workflow `audit_logs`                      | **2,555 days (~7 years)**                      | Automated daily purge in `main.rs`; `user_id` set to `NULL` on account deletion to preserve audit integrity |
| Attendance network observations                       | **`OBSERVATION_RETENTION_DAYS`**               | Short rolling window purged daily; PDPA minimization for home/mobile egress prefixes                        |
| Email delivery logs (`email_logs`)                    | **180 days**                                   | Operational delivery verification; sensitive body secrets redacted                                          |
| User sessions (`user_sessions`)                       | **30 days** (revoked) / **90 days** (inactive) | Automated daily purge                                                                                       |
| Expired/revoked `refresh_tokens`                      | **30 days** after expiry/revocation            | Automated daily purge                                                                                       |
| Unreferenced expired QR tokens                        | **7 days**                                     | Automated daily purge; tokens linked to check-ins retained with attendance record                           |
| Portal notifications                                  | **90 days** (read) / **365 days** (unread)     | Automated daily purge                                                                                       |
| Bulk import validation sessions                       | **Upon session expiry**                        | Automated daily purge of staged CSV/XLSX JSON payloads                                                      |

### 8. Governance: Data Protection Officer (DPO) & 72-Hour Breach Notification

Pursuant to the **1 June 2025** PDPA governance provisions:

- **Data Protection Officer (Section 12A):** Employers meeting the prescribed threshold (processing personal data of >20,000 data subjects, sensitive personal data of >10,000 data subjects, or regular and systematic monitoring) must appoint a qualified, Malaysia-resident **Data Protection Officer (DPO)** registered with the Personal Data Protection Commissioner and publish business contact details to employees.
- **Mandatory Data Breach Notification (Section 12B):** Where a personal data breach occurs that causes or is likely to cause **significant harm**, the Data Controller must notify the **Personal Data Protection Commissioner (`dbnpdp@pdp.gov.my` / official portal) as soon as practicable and within 72 hours**, and notify affected individuals **without unnecessary delay** (within 7 days of notifying the Commissioner). Data Processors must notify the Data Controller immediately upon discovering any security incident.

### 9. Your Rights as a Data Subject

Under **Sections 30–43A of the PDPA**, you have the right to:

1. **Access (s.30):** View your personal profile, attendance history, leave balances, claims, and payslips directly via the Employee Portal or request a copy of your personal data;
2. **Correction (s.34):** Request correction of inaccurate, incomplete, misleading, or outdated personal data through HR/Payroll administration;
3. **Withdraw Consent (s.38):** Withdraw consent for optional processing by written notice (subject to statutory employment/tax obligations that must continue by law);
4. **Prevent Processing Causing Damage or Distress (s.42):** Object to unwarranted processing causing substantial damage or distress; and
5. **Data Portability (s.43A — effective 1 June 2025):** Request transmission of your personal data to another Data Controller in a structured, commonly used machine-readable format where technically feasible.

**Contact & Inquiries:** Please direct all PDPA access, correction, or portability requests to your employing organization's **HR Department or appointed Data Protection Officer (DPO)** as listed in your Company Profile (`Settings → Company`). DataController responses are processed within **21 days** in accordance with the PDPA.

---

## Bahagian B — Notis Perlindungan Data Peribadi (Bahasa Malaysia)

### 1. Pengenalan dan Peranan (_Pengawal Data_ & _Pemproses Data_)

Notis ini dikeluarkan selaras dengan **Akta Perlindungan Data Peribadi 2010 [Akta 709]** sebagaimana yang dipinda oleh **Akta Perlindungan Data Peribadi (Pindaan) 2024 [Akta A1727]** (**"PDPA"**) untuk menerangkan bagaimana data peribadi dan data peribadi sensitif anda dikumpulkan, diproses, dizahirkan, disimpan, dan dilindungi dalam **Payroll System** (**"Platform"**).

- **Pengawal Data (_Data Controller_):** Majikan anda (`company`) ialah **Pengawal Data** yang menentukan tujuan dan cara pemprosesan data pekerjaan, kehadiran, cuti, tuntutan, dan penggajian anda.
- **Pemproses Data (_Data Processor_):** Pengendali infrastruktur Platform bertindak sebagai **Pemproses Data** dan tertakluk secara langsung kepada **Prinsip Keselamatan di bawah Seksyen 9 PDPA**.

### 2. Kategori Data Peribadi Yang Diproses

1. **Data Pengenalan & Demografi:** Nama penuh, nombor pekerja, nombor MyKad (NRIC) atau pasport/permit kerja, tarikh lahir, jantina, kewarganegaraan, bangsa (pilihan), status pemastautin (`citizen`, `permanent_resident`, `foreigner`), status perkahwinan, status pekerjaan pasangan, dan bilangan anak layak.
2. **Data Perhubungan:** Alamat e-mel, nombor telefon, dan alamat kediaman.
3. **Data Pekerjaan & Organisasi:** Jabatan, jawatan, cawangan, pusat kos, jenis pekerjaan, tarikh mula kerja/percubaan/pengesahan/peletakan jawatan, jadual kerja, dan dokumen HR.
4. **Data Gaji, Perbankan & Berkanun:** Gaji pokok, sejarah pelarasan gaji, elaun, kerja lebih masa (OT), tuntutan perbelanjaan, nama bank dan nombor akaun, TIN cukai pendapatan, nombor dan kategori Jadual Ketiga KWSP (EPF), nombor PERKESO (SOCSO), nombor SIP (EIS), rekod Borang TP3, Zakat, PTPTN, dan Tabung Haji.
5. **Data Kehadiran, Lokasi & Rangkaian:** Masa daftar masuk/keluar, imbasan QR, koordinat GPS (apabila mod geofence `warn`/`enforce` diaktifkan), dan awalan IP rangkaian pejabat (apabila mod pengikatan rangkaian diaktifkan).
6. **Data Peribadi Sensitif (Seksyen 4 PDPA):** Sijil cuti sakit/penghospitalan atau resit perubatan yang dimuat naik bagi tujuan kelulusan cuti/tuntutan, serta penanda kelayakan Zakat. **Data biometrik** (seperti imbasan wajah/cap jari pada peranti anda melalui WebAuthn / "Face ID") diproses sepenuhnya di dalam peranti anda sendiri dan **tidak pernah dihantar atau disimpan di dalam pelayan Platform**.

### 3. Tujuan Pemprosesan Data Peribadi

Data peribadi anda diproses bagi tujuan:

- Pentadbiran kontrak perkhidmatan, pengiraan gaji bulanan, kerja lebih masa, prorata gaji (**Seksyen 18B Akta Kerja 1955**), dan pembayaran balik tuntutan;
- Pematuhan kewajipan undang-undang di bawah **Akta Kerja 1955** (serta Ordinan Buruh Sabah & Sarawak Pindaan 2025), **Perintah Gaji Minimum 2024**, **Akta KWSP 1991** (termasuk caruman mandatori pekerja asing Bahagian F), **Akta Keselamatan Sosial Pekerja 1969**, **Akta SIP 2017**, **Akta Cukai Pendapatan 1967** (PCB/MTD & Borang EA), dan **Akta PSMB 2001 (HRD Corp)**;
- Pengesahan kehadiran, pengurusan cuti dan kelulusan, kawalan keselamatan pengesahan dua faktor (2FA/Passkey), serta jejak audit pematuhan.

Kegagalan membekalkan data mandatori (seperti tarikh lahir, nombor pengenalan, maklumat akaun bank, atau nombor berkanun) boleh menjejaskan pengiraan caruman berkanun berasaskan umur, pembayaran gaji melalui pindahan bank, atau pemfailan berkanun majikan.

### 4. Penzahiran Data Peribadi & Pemindahan Rentas Sempadan

Data peribadi anda tidak akan dijual dan hanya dizahirkan mengikut keperluan undang-undang serta operasi kepada:

- **Agensi Berkanun Malaysia:** KWSP, PERKESO, LHDN (e-PCB Plus / MyTax), Jabatan Tenaga Kerja (JTK), dan HRD Corp;
- **Institusi Kewangan:** Bank berlesen bagi pembayaran gaji serta badan berkanun Zakat, PTPTN, dan Tabung Haji;
- **Sub-Pemproses Infrastruktur & Keselamatan:** Cloudflare (perlindungan bot Turnstile dan proksi), Google OAuth2 (jika diaktifkan), dan penyedia SMTP e-mel yang ditetapkan oleh majikan. Sebarang pemindahan data ke luar Malaysia mematuhi syarat perlindungan setara di bawah **Seksyen 129 PDPA (berkuat kuasa 1 April 2025)**.

### 5. Keselamatan, Penyimpanan Data & Pemberitahuan Pelanggaran Data (72 Jam)

- **Keselamatan & Penyimpanan (Seksyen 9 & 10):** Data gaji, cukai, dan log audit disimpan selama **7 tahun (2,555 hari)** selaras dengan **Seksyen 82 Akta Cukai Pendapatan 1967** dan **Seksyen 61 Akta Kerja 1955**, manakala data operasi sementara (seperti pemerhatian rangkaian kehadiran, sesi tamat tempoh, dan log e-mel) dipadamkan secara automatik mengikut jadual pengekalan harian.
- **Pegawai Perlindungan Data (DPO — Seksyen 12A) & Pemberitahuan Pelanggaran Data (DBN — Seksyen 12B):** Selaras dengan peruntukan PDPA berkuat kuasa **1 Jun 2025**, majikan yang memenuhi nilai ambang ditetapkan hendaklah melantik **Pegawai Perlindungan Data (DPO)** dan memaklumkan **Pesuruhjaya Perlindungan Data Peribadi dalam tempoh 72 jam** sekiranya berlaku pelanggaran data peribadi yang boleh mendatangkan kemudaratan ketara.

### 6. Hak Subjek Data & Cara Menghubungi Kami

Di bawah **Seksyen 30 hingga 43A PDPA**, anda berhak untuk:

1. Mengakses dan menyemak data peribadi serta penyata gaji anda melalui Portal Pekerja (**Seksyen 30**);
2. Memohon pembetulan terhadap data peribadi yang tidak tepat, tidak lengkap, atau tidak terkini (**Seksyen 34**);
3. Menarik balik persetujuan pemprosesan pilihan melalui notis bertulis (**Seksyen 38**);
4. Menghalang pemprosesan yang mungkin menyebabkan kerosakan atau tekanan yang tidak wajar (**Seksyen 42**); dan
5. Memohon **kemudahalihan data (_data portability_)** dalam format berstruktur yang boleh dibaca mesin tertakluk kepada kebolehlaksanaan teknikal (**Seksyen 43A**, berkuat kuasa 1 Jun 2025).

Sebarang pertanyaan, permohonan akses, atau pembetulan data peribadi hendaklah dikemukakan kepada **Jabatan Sumber Manusia (HR) atau Pegawai Perlindungan Data (DPO)** majikan anda seperti yang dinyatakan pada halaman Profil Syarikat.
