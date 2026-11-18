# Statutory rules

EPF (KWSP), SOCSO (PERKESO), EIS (SIP) and PCB/MTD are table-driven,
effective-dated and **versioned** — never hardcoded into UI components or
scattered functions. For the complete statute-by-statute reference, see
[../legal/malaysian-statutory-compliance.md](../legal/malaysian-statutory-compliance.md).

## Rule sets

`statutory_rule_sets` carries a named, dated, source-linked dataset
(`source_version`, `source_sha256`, effective window). A payroll run snapshots
the rule-set ids it used (`calculation_snapshot`), so later rule versions never
silently rewrite history.

## Fail-closed posture

The fixtures in `1001_data.sql` are unverified academic data (`status = 'prototype'`).
Production processing refuses to run until a source-linked verified rule set
exists for every domain (`epf`, `socso`, `eis`, `pcb`), and automatic PCB stays
disabled (`platform_settings.pcb_calculator_status <> 'enabled'`) until the
calculator passes **LHDN computerised-MTD conformance** (_Spesifikasi Kaedah
Pengiraan Berkomputer PCB 2026_). Wrong statutory figures are worse than a
refused run.

## Services & 2025–2026 statutory coverage

| Service          | Governing statute                                                                                                 | Responsibility                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ---------------- | ----------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `epf_service`    | _EPF Act 1991 (Act 452)_ & _EPF (Amendment) Act 2025_                                                             | Derives Third Schedule Parts **A** (citizen/PR <60), **B** (foreigner <60 pre-Oct 2025), **C** (PR 60–74), **D** (foreigner 60–74 pre-Oct 2025), **E** (citizen 60–74: 0% employee / 4% employer), **F** (non-citizen valid work-pass holder <75: mandatory **2% + 2%** from **1 October 2025**), and statutory **EXEMPT** at age **75+** (_s.43(1)_). Excludes overtime from contributable wages (_s.2_). Honours explicit `employees.epf_category` (`A`–`F`) overrides while surfacing `epf_category_override` in preview. |
| `socso_service`  | _Employees' Social Security Act 1969 (Act 4)_ & _P.U. (A) 255/2024_                                               | First Category (<55) vs Second Category (≥60, employer-only), clamped at the schedule's ceiling (**RM6,000/month** from 1 Oct 2024). Fails closed for ages 55–59 and foreign-worker schemes until entry-age/prior-contribution fields are recorded.                                                                                                                                                                                                                                                                          |
| `eis_service`    | _Employment Insurance System Act 2017 (Act 800)_ & _P.U. (A) 256/2024_                                            | Citizens and PRs aged 18–56 (0.2% + 0.2% up to **RM6,000/month** ceiling); exempts foreigners and age ≥60; fails closed for ages 57–59 pending prior-contribution data.                                                                                                                                                                                                                                                                                                                                                      |
| `pcb_calculator` | _Income Tax Act 1967 (Act 53)_ & _MTD Rules 1994_                                                                 | Progressive resident brackets (0%–30%), RM400 rebate (≤ RM35,000 chargeable income), Schedule 2 additional-remuneration (bonus/commission) differential, ringgit-for-ringgit Zakat offset, YTD + TP3 accumulation. Gated in production behind `pcb_calculator_status = 'enabled'`.                                                                                                                                                                                                                                           |
| `payroll_engine` | _Employment Act 1955 (Act 265)_, _Sabah/Sarawak 2025 Amendments_ & _Minimum Wages Order 2024 [P.U. (A) 376/2024]_ | Calendar-day proration (_s.18B_), ORP/OT rating (`26` days, `8` hours, `1.5×`/`2.0×`/`3.0×` under _s.60I_), unpaid-leave statutory base reduction, and `below_minimum_wage` advisory warning when a full-time basic salary is below **RM1,700/month** (effective 1 Feb 2025 / universal 1 Aug 2025).                                                                                                                                                                                                                         |

## Verification duty

Any rate or relief change lands as a **new rule-set row** (never an in-place
edit) with its official source URL, version, and SHA-256 digest. Before
production use, validate against official KWSP, PERKESO, and LHDN (MyTax e-PCB
Plus) publications — the test suite pins expected values against prototype
fixtures, which are not authoritative statutory schedules.
