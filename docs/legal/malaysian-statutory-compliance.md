# Malaysian Employment, Payroll, Tax & Data Protection Regulatory Guide (2026)

**Last Reviewed:** 18 November 2026  
**Scope:** Peninsular Malaysia, Federal Territories, Sabah & Sarawak

This reference documents the Malaysian statutes, subsidiary orders, agency specifications, contribution ceilings, and compliance rules applicable to payroll and HR workflows in **2025–2026**, and maps each requirement to how this repository implements, enforces, or fail-closes it.

> [!IMPORTANT]
> The repository's seeded `legacy-prototype-*` rows in `1001_data.sql` are academic fixtures (`status = 'prototype'`). Production payroll **fails closed** until an organization imports complete, official schedules from KWSP, PERKESO, and LHDN, verifies their SHA-256 provenance in `statutory_rule_sets`, and passes LHDN computerised-MTD conformance testing before enabling `pcb_calculator_status`.

---

## 1. Employment Legislation & Labour Standards

### Primary Statutes

- **Peninsular Malaysia & Labuan:** _Employment Act 1955 (Act 265)_, incorporating the _Employment (Amendment) Act 2022 (Act A1651)_ (in force 1 January 2023).
- **Sabah:** _Labour Ordinance of Sabah (Cap. 67)_, as amended by the _Labour Ordinance of Sabah (Amendment) Act 2025_ (**in force 1 May 2025**).
- **Sarawak:** _Labour Ordinance of Sarawak (Cap. 76)_, as amended by the _Labour Ordinance of Sarawak (Amendment) Act 2025_ (**in force 1 May 2025**).

> **2025 East Malaysia Harmonization:** Effective **1 May 2025**, Sabah and Sarawak removed their legacy RM2,500 coverage ceiling and aligned core labour protections—45-hour maximum work week, 98 days paid maternity leave, 7 days paid paternity leave, separate 60-day hospitalization leave, flexible working arrangement (FWA) procedures, and anti-discrimination/forced-labour protections—with Peninsular Malaysia's _Employment Act 1955_.

### Key Statutory Rules & Codebase Mapping

| Statutory Provision                                                                  | Requirement (2025–2026)                                                                                                                                                                                                                                                                                                                                                                                                                                               | Implementation in Payroll System                                                                                                                                                                                                                                               |
| :----------------------------------------------------------------------------------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **First Schedule Coverage**                                                          | Universal coverage of all employees under contract of service regardless of wage; statutory overtime & termination-benefit entitlements remain mandatory up to **RM4,000/month** wages (unless manual/specified categories apply)                                                                                                                                                                                                                                     | Supported for all employees; OT applications and attendance OT configurable per company                                                                                                                                                                                        |
| **Working Hours (s.60A(1))**                                                         | Maximum **45 working hours per week** (exclusive of unpaid meal/rest breaks)                                                                                                                                                                                                                                                                                                                                                                                          | Default company settings: `effective_hours_per_day = 8`, `rest_time_minutes = 60`, `unpaid_break_minutes = 60` (`1024_unpaid_break_minutes.sql`)                                                                                                                               |
| **Ordinary Rate of Pay (ORP) & OT Multipliers (s.60I, s.60A(3), s.60(3), s.60D(3))** | Monthly-rated ORP = $\text{Monthly Wages} \div 26$; HRP = $\text{ORP} \div \text{normal hours (8)}$. Multipliers: **1.5×** normal day, **2.0×** rest day, **3.0×** public holiday                                                                                                                                                                                                                                                                                     | `OvertimeSettings::rate_overtime` uses `working_days_per_month` (`26`), `effective_hours_per_day` (`8`), and `1.5` / `2.0` / `3.0` multipliers (`1022_overtime_divisor_setting_rename.sql`); hourly rate stays unrounded until final sen rounding                              |
| **Incomplete Month Proration (s.18A / s.18B)**                                       | Prorated wage = $\frac{\text{Monthly Wages}}{\text{Number of calendar days in the wage period}} \times \text{Eligible calendar days}$                                                                                                                                                                                                                                                                                                                                 | `payroll_engine::compute_payslip` prorates both `basic_salary` and overlapping recurring allowances by exact **calendar days** (`period_days`)                                                                                                                                 |
| **Payment of Wages (s.19 & s.25A)**                                                  | Wages (less lawful deductions) must be paid **not later than the 7th day** after the last day of the wage period, via bank account payment                                                                                                                                                                                                                                                                                                                            | Default `payment_day = 28` (cutoff `25`); `missing_bank_account` warning surfaced in `preview_payroll`; bank CSV export in `payroll_service`                                                                                                                                   |
| **Statutory Paid Leave Minimums**                                                    | **Annual Leave (s.60E):** 8 days (<2 yrs), 12 days (2–5 yrs), 16 days (>5 yrs)<br>**Sick Leave (s.60F):** 14 days (<2 yrs), 18 days (2–5 yrs), 22 days (>5 yrs) + **60 days** hospitalization<br>**Maternity Leave (s.37):** **98 consecutive days**<br>**Paternity Leave (s.60FA):** **7 consecutive days** (married male, ≥12 months service, up to 5 confinements)<br>**Public Holidays (s.60D):** Minimum **11 gazetted days** (incl. 5 compulsory national days) | Provisioned in `provision_company_defaults` (`Maternity Leave = 98`, `Paternity Leave = 7`, `Hospitalisation Leave = 60`, `Sick Leave = 14`, `Annual Leave = 14`); balance reservation enforced in `leave_rules.rs`; holiday calendar + `.ics` import in `calendar_service.rs` |
| **Register & Payslip Retention (s.61)**                                              | Employers must furnish itemized wage statements and retain employee/wage registers for **at least 6 years**                                                                                                                                                                                                                                                                                                                                                           | Itemized `payroll_item_details` persisted per run; PDF payslips in `payslip_pdf_service.rs`; 7-year (2,555-day) retention floor in `main.rs`                                                                                                                                   |

---

## 2. National Minimum Wage — _Minimum Wages Order 2024_

- **Governing Law:** _National Wages Consultative Council Act 2011 (Act 732)_ & _Minimum Wages Order 2024 [P.U. (A) 376/2024]_.
- **Effective Timeline:**
  - **1 February 2025:** Mandatory **RM1,700.00/month** for employers with **5 or more employees** and all employers conducting professional activities classified under **MASCO** (regardless of headcount).
  - **1 August 2025:** Mandatory **RM1,700.00/month** across **all remaining employers nationwide** (including micro-enterprises with fewer than 5 employees).
  - **2026 Status:** Uniform **RM1,700.00/month** national floor across Peninsular Malaysia, Sabah, Sarawak, and Labuan.
- **Statutory Equivalent Rates:**
  - **Hourly rate:** **RM8.72/hour**
  - **Daily rate:** **RM78.46/day** (5-day work week), **RM65.38/day** (6-day work week), **RM98.08/day** (4-day work week).
  - ** Statutory Exclusions:** Domestic employees (_pekerja domestik_) and statutory apprenticeship contracts (_kontrak perantisan_ under EA 1955 s.2).
- **Codebase Enforcement:**
  - `payroll_engine::preview_payroll` evaluates `statutory_minimum_wage_sen(period.effective_date)` (`170_000` sen from `2025-02-01`; `150_000` sen prior) and raises a `below_minimum_wage` advisory diagnostic for full-time non-intern employees whose contracted `basic_salary` falls below the statutory floor.
  - `company_settings` seeds `statutory/minimum_wage_monthly = "1700"` (`1028_epf_part_f_and_statutory_defaults.sql`).

---

## 3. EPF / KWSP — _Employees Provident Fund Act 1991 (Act 452)_

### Primary Statute & 2025–2026 Amendments

- _Employees Provident Fund Act 1991 (Act 452)_, **Section 2** (definition of wages), **Section 43(1)** (cessation at age 75), and **Third Schedule (Parts A–F)**.
- _Employees Provident Fund (Amendment) Act 2025_ (**effective 1 October 2025 salary month**, first remittance due **15 November 2025**): introduced **Part F** mandating EPF contributions for non-Malaysian citizen employees holding valid work passes.
- _Mid-2026 Status Note:_ Although the Cabinet announced a policy review of foreign-worker EPF implementation in July 2026, **Part F (2% + 2%) remains legally active and enforceable statutory law** unless and until a gazetted amendment takes effect.
- _Member Account Structure (effective 11 May 2024):_ Contributions for members under age 55 are credited across **Akaun Persaraan (75%)**, **Akaun Sejahtera (15%)**, and **Akaun Fleksibel (10%)** by KWSP upon remittance.

### Third Schedule Categories (Parts A – F)

|    Part    | Eligible Employee Category                                                                                                              | Age Bracket | Standard Statutory Rate Reference                                                                                  |
| :--------: | :-------------------------------------------------------------------------------------------------------------------------------------- | :---------: | :----------------------------------------------------------------------------------------------------------------- |
|   **A**    | Malaysian Citizens, Permanent Residents (PR), & non-citizens registered before 1 Aug 1998                                               |  Below 60   | **11%** employee; **13%** employer (wages ≤ RM5,000) / **12%** employer (wages > RM5,000) per Third Schedule bands |
|   **B**    | Non-Malaysian citizens registered on/after 1 Aug 1998 (pre-Oct 2025 / voluntary legacy election)                                        |  Below 60   | **11%** employee; **RM5.00** employer                                                                              |
|   **C**    | Permanent Residents (PR) & non-citizens registered before 1 Aug 1998                                                                    |  60 to 74   | **5.5%** employee; **6.5%** employer (≤ RM5,000) / **6.0%** employer (> RM5,000)                                   |
|   **D**    | Non-Malaysian citizens registered on/after 1 Aug 1998 (pre-Oct 2025 / voluntary legacy election)                                        |  60 to 74   | **5.5%** employee; **RM5.00** employer                                                                             |
|   **E**    | **Malaysian Citizens**                                                                                                                  |  60 to 74   | **0%** employee; **4.0%** employer                                                                                 |
|   **F**    | **Non-Malaysian Citizens holding valid work passes** (Employment Pass, PLKS, etc.; excludes domestic servants) — _effective 1 Oct 2025_ |  Below 75   | **2.0%** employee + **2.0%** employer (fractions of a ringgit rounded up to the next whole ringgit)                |
| **Exempt** | Any employee who has attained **75 years of age** (_EPF Act 1991 s.43(1)_)                                                              |     75+     | **RM0** employee / **RM0** employer                                                                                |

### Contributable Wage Base & Remittance

- **Excluded from EPF "Wages" (s.2):** Overtime payments, service charges, gratuities, retirement benefits, retrenchment/termination benefits, payment in lieu of notice, and travelling allowances. (`payroll_engine.rs` computes `epf_wage` **excluding** `total_overtime` and claims reimbursements, and reduces for unpaid leave).
- **Included in EPF "Wages":** Basic salary, fixed/variable allowances (unless excluded under s.2), bonuses, and commissions.
- **Monthly Remittance Deadline:** On or before the **15th day of the following month** via KWSP i-Akaun.
- **Codebase Enforcement:** `epf_service::derive_category_for` derives Parts `A`, `B`, `C`, `D`, `E`, `F`, and `EXEMPT` (age 75+) from `(age, residency_status, effective_date)`; `employees_epf_category_check` (`1028`) validates explicit overrides `A`–`F`; `preview_payroll` warns (`epf_category_override`) whenever a stored override differs from the derived Third Schedule part.

---

## 4. SOCSO & EIS (PERKESO) — _Act 4 & Act 800_

### Primary Statutes & 2024–2026 Reforms

- **SOCSO:** _Employees' Social Security Act 1969 (Act 4)_ & _P.U. (A) 255/2024_.
- **EIS (SIP):** _Employment Insurance System Act 2017 (Act 800)_ & _P.U. (A) 256/2024_.
- **RM6,000 Monthly Wage Ceiling (effective 1 October 2024):** Both SOCSO and EIS raised their monthly insurable wage ceiling from RM5,000 to **RM6,000/month** (`600_000` sen). Wages exceeding RM6,000 contribute at the RM6,000 ceiling band (`StatutoryTables::socso_band` and `eis_band` clamp via `.min(ceiling)`).
- **Foreign Worker PERKESO Updates (2024–2026):**
  - From **1 July 2024**, foreign workers with valid passes under age 60 were extended into SOCSO **First Category** (Employment Injury + Invalidity Scheme; Second Category applies at 60+ or age ≥55 upon first entry).
  - **EIS (Act 800)** remains strictly applicable to **Malaysian citizens and Permanent Residents aged 18–59**; foreign workers and employees aged 60+ are exempt (`eis_service.rs` returns `0`/`0`).
  - **Lindung 24 Jam / SKBBK (June 2026):** Round-the-clock non-employment injury protection introduced for foreign workers (0.75% employee-borne up to the RM6,000 ceiling).

### Age & Eligibility Gates in Code

- **Wage Base (Act 4 s.2 / Act 800 s.2):** Includes basic salary, overtime, commissions, paid leave, and allowances (excludes claims reimbursements, annual bonuses in specific statutory contexts, gratuities, and EPF employer shares). `payroll_engine.rs` includes `total_overtime` in `statutory_socso_eis_wage`.
- **Fail-Closed Age/History Guards:**
  - Missing `date_of_birth` blocks payroll calculation (`missing_date_of_birth`).
  - `socso_service.rs` fails closed for ages **55–59** (where First vs. Second Category depends on whether the employee first entered PERKESO before age 55) and for foreign workers until PERKESO entry-age/scheme metadata is recorded on the employee model.
  - `eis_service.rs` fails closed for ages **57–59** (where contribution liability depends on prior contributions before age 57 under Act 800).
- **Monthly Remittance Deadline:** On or before the **15th day of the following month** via PERKESO ASSIST Portal.

---

## 5. Income Tax (PCB / MTD & Form EA) — _Income Tax Act 1967 (Act 53)_

### Primary Statute & LHDN Specifications

- _Income Tax Act 1967 (Act 53)_, **Section 82** (7-year record keeping), **Section 83(1) & 83(1A)** (Form E / C.P.8D & Form EA / C.P.8A), and **Section 107C / Income Tax (Deduction from Remuneration) Rules 1994**.
- _LHDN Computerised Calculation Method Specification 2026_ (_Spesifikasi Kaedah Pengiraan Berkomputer PCB 2026_).
- **e-PCB Plus Consolidation (MyTax Portal):** LHDN replaced the legacy standalone _e-PCB_, _e-Data PCB_, and _e-CP39_ portals with **e-PCB Plus** on [MyTax](https://mytax.hasil.gov.my), unifying employee TIN validation, monthly PCB submission, and FPX remittance.

### Resident Individual Tax Brackets (YA 2025 & YA 2026)

| Chargeable Income Band (RM) | Rate (%) | Cumulative Tax at Band Floor (RM) |
| :-------------------------- | :------: | :-------------------------------: |
| 0 – 5,000                   |    0%    |                 0                 |
| 5,001 – 20,000              |    1%    |                 0                 |
| 20,001 – 35,000             |    3%    |                150                |
| 35,001 – 50,000             |    6%    |                600                |
| 50,001 – 70,000             |   11%    |               1,500               |
| 70,001 – 100,000            |   19%    |               3,700               |
| 100,001 – 400,000           |   25%    |               9,400               |
| 400,001 – 600,000           |   26%    |              84,400               |
| 600,001 – 2,000,000         |   28%    |              136,400              |
| Exceeding 2,000,000         |   30%    |              528,400              |

- **Individual Tax Rebate (s.6A(2)):** **RM400** where chargeable income ≤ **RM35,000** (plus RM400 spouse rebate where non-working spouse relief is claimed).
- **Zakat / Fitrah Rebate (s.6A(3)):** Ringgit-for-ringgit offset against tax payable (cannot reduce tax below RM0).

### Key Resident Tax Reliefs Reference (YA 2025 & YA 2026 — Budget 2025 / 2026 Updates)

| Relief Category                                                                            |     Cap (RM)      | Notes                                                                                          |
| :----------------------------------------------------------------------------------------- | :---------------: | :--------------------------------------------------------------------------------------------- |
| Individual & dependent relatives                                                           |     **9,000**     | Mandatory baseline relief                                                                      |
| Spouse (non-working / alimony)                                                             |     **4,000**     | Where spouse has no income or elects combined assessment                                       |
| Disabled individual (self)                                                                 |     **7,000**     | Increased from RM6,000 from YA 2025                                                            |
| Disabled spouse (additional)                                                               |     **6,000**     | Increased from RM5,000 from YA 2025                                                            |
| Unmarried child under 18                                                                   |     **2,000**     | Per qualifying child                                                                           |
| Unmarried child 18+ in higher education                                                    |     **8,000**     | Diploma+ in Malaysia or degree+ abroad                                                         |
| Disabled unmarried child                                                                   |     **8,000**     | Increased from RM6,000 from YA 2025 (+RM8,000 if 18+ in higher education)                      |
| Mandatory / voluntary EPF (KWSP)                                                           |     **4,000**     | Sub-cap within the RM7,000 EPF + Life Insurance group                                          |
| Life insurance / Family Takaful                                                            |     **3,000**     | Sub-cap within the RM7,000 group                                                               |
| SOCSO & EIS employee contributions (s.49(1D))                                              |      **350**      | Combined annual cap across PERKESO Act 4 + Act 800 employee deductions                         |
| Medical expenses (self/spouse/child, incl. learning disability intervention up to RM6,000) |    **10,000**     | Subject to LHDN sub-caps (vaccination RM1,000, dental RM1,000, check-up/mental health RM1,000) |
| Medical, special needs & carer expenses for parents / grandparents                         |     **8,000**     | Expanded in YA 2025 to include grandparents                                                    |
| Education fees (self, incl. upskilling up to RM2,000)                                      |     **7,000**     | Accredited institutions / recognized upskilling                                                |
| Lifestyle (books, PC/smartphone, internet, skill courses) + Sports                         | **2,500 + 1,000** | Separate RM1,000 sports equipment/activity/gym relief                                          |
| First-home housing loan interest (SPA 1 Jan 2025 – 31 Dec 2027)                            | **7,000 / 5,000** | Up to RM7,000 (property ≤ RM500k) or RM5,000 (RM500k–RM750k)                                   |

### Employer Filing Deadlines & Codebase Posture

- **Monthly PCB/MTD Remittance:** On or before the **15th day of the following month** via LHDN e-PCB Plus (MyTax).
- **Form EA (Borang C.P.8A) to Employees (s.83(1A)):** Must be rendered to every employee on or before the **last day of February** following the assessment year (`ea_form_service.rs`).
- **Form E & C.P.8D Return to LHDN (s.83(1)):** Due **31 March** (electronic filing via MyTax typically granted grace to 30 April by LHDN operational programme).
- **Form TP3:** Prior-employer YTD remuneration, EPF, SOCSO, Zakat, and PCB captured in `tp3_records` and folded into YTD figures.
- **Production Gate (`pcb_calculator_status`):** Automatic PCB stays **fail-closed** in production until official LHDN computerised-MTD test vectors replace `src/tests/fixtures/pcb_regression_vectors.json` (including full TP1 monthly relief claims and combined s.49(1D) relief alignment) and pass LHDN verification.

---

## 6. HRD Corp Levy — _Pembangunan Sumber Manusia Berhad Act 2001 (Act 612)_

- **Mandatory Registration & Levy (1.0%):** Employers in covered sectors with **10 or more Malaysian citizen employees** must contribute **1.0%** of monthly wages (basic salary + fixed allowances) per Malaysian employee.
- **Optional Registration (0.5%):** Employers with **5 to 9 Malaysian citizen employees** may opt in at **0.5%**.
- **Foreign Workers:** Non-Malaysian citizens are excluded from the HRD Corp headcount and levy base.
- **Monthly Remittance Deadline:** On or before the **15th day of the following month** via HRD Corp e-TRiS.
- **Codebase Configuration:** `company_settings` tracks `statutory/hrdf_enabled` and `statutory/hrdf_rate` (`"1"`). Note that HRD Corp levy is not yet automatically booked as a payroll run line; employers subject to Act 612 must compute and remit via e-TRiS.

---

## 7. Personal Data Protection & Cybersecurity — _PDPA 2010 / 2024 & Act 854_

- **Primary Law:** _Personal Data Protection Act 2010 (Act 709)_ as amended by the _Personal Data Protection (Amendment) Act 2024 (Act A1727)_.
- **Phased Enforcement Completed in 2025:**
  1. **1 January 2025:** Administrative and electronic-notice provisions.
  2. **1 April 2025:** "Data Controller" terminology; direct **Section 9 Security Principle** liability on **Data Processors**; **biometric data** classified as Sensitive Personal Data; revised **Section 129** cross-border transfer framework; maximum penalties raised to **RM1,000,000 fine and/or 3 years imprisonment**.
  3. **1 June 2025:** Mandatory **Data Protection Officer (DPO)** appointment & registration (**s.12A**); mandatory **72-hour Data Breach Notification (DBN)** to the PDP Commissioner and affected individuals (**s.12B**); **Data Portability** right (**s.43A**).
- **Related Legislation:** _Cyber Security Act 2024 (Act 854)_ (applicable where a deployment serves designated National Critical Information Infrastructure [NCII] entities).
- **Full Notice & Retention Matrix:** See [Privacy Policy & Bilingual PDPA Notice](privacy-and-pdpa-notice.md).

---

## 8. Official Malaysian Statutory Sources

- **KWSP / EPF:** <https://www.kwsp.gov.my/en/employer/responsibilities/mandatory-contribution>
- **PERKESO / SOCSO & EIS:** <https://www.perkeso.gov.my/en/rate-of-contribution.html>
- **LHDN / HASiL (e-PCB Plus & MTD Specifications):** <https://www.hasil.gov.my/en/employers/> and <https://mytax.hasil.gov.my>
- **Jabatan Tenaga Kerja Semenanjung Malaysia (JTKSM):** <https://jtksm.mohr.gov.my>
- **Minimum Wages Portal (NWCC / Kementerian Sumber Manusia):** <https://gajiminimum.mohr.gov.my>
- **HRD Corp:** <https://hrdcorp.gov.my>
- **Jabatan Perlindungan Data Peribadi (JPDP / PDP Commissioner):** <https://www.pdp.gov.my>
