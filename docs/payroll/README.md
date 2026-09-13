# Payroll module

This directory documents the payroll subsystem as implemented — the run
lifecycle, the calculation engine, statutory rule versioning, permissions,
the operational overview, and the payment/accounting outputs added in
migration 1025.

Read these together with `../features.md` (capability status) and
`../database.md` (schema reference).

| Document | Covers |
| --- | --- |
| [architecture.md](architecture.md) | Module boundaries, request flow, layering |
| [calculation-engine.md](calculation-engine.md) | Money handling, engine structure, explainability |
| [workflows.md](workflows.md) | Run lifecycle incl. cancellation, adjustments, audit |
| [statutory-rules.md](statutory-rules.md) | EPF/SOCSO/EIS/PCB rule sets, verification gate |
| [permissions.md](permissions.md) | Permission matrix, separation of duties, scoping |
| [database.md](database.md) | Payroll tables, constraints, indexes, migration notes |
| [testing.md](testing.md) | Test inventory and how to extend it |
