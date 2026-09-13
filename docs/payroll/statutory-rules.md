# Statutory rules

EPF (KWSP), SOCSO (PERKESO), EIS and PCB/MTD are table-driven, effective-dated
and **versioned** — never hardcoded into components or scattered functions.

## Rule sets

`statutory_rule_sets` carries a named, dated, source-linked dataset
(`source_version`, `source_sha256`, effective window). A payroll run snapshots
the rule-set ids it used (`calculation_snapshot`), so later rule versions never
silently rewrite history.

## Fail-closed posture

The fixtures in `1001_data.sql` are unverified academic data. Production
processing refuses to run until a source-linked verified rule set exists, and
automatic PCB stays disabled until the calculator passes LHDN computerised-MTD
conformance. This is deliberate: wrong statutory figures are worse than no
run.

## Services

| Service | Responsibility |
| --- | --- |
| `epf_service` | Category derivation (age/residency), table lookup, caps |
| `socso_service` | First/second category by age, wage ceiling |
| `eis_service` | Contribution history gate, wage ceiling |
| `pcb_calculator` | Bracketed MTD incl. additional-remuneration (bonus) handling, zakat offset, YTD |
| `tp3` records | Previous-employer YTD feeds the annualisation |

## Verification duty

Any rate change lands as a **new rule-set row** (never an in-place edit) with
its official source. Before production use, validate against KWSP/PERKESO/LHDN
publications — the test suite pins expected values, but the shipped fixtures
are not authoritative.
